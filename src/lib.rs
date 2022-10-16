use std::{
    any::Any,
    cell::{Cell, RefCell},
    future::Future,
    marker::PhantomData,
    panic::{catch_unwind, resume_unwind, AssertUnwindSafe},
    pin::Pin,
    task::Poll,
};

pub enum Never {}

pub struct Nolife<T, P, F>
where
    P: FnOnce(TimeCapsule<T>) -> F,
    F: Future<Output = Never>,
{
    producer: Cell<Option<P>>,
    active_fut: RefCell<Option<F>>,
    phantom: PhantomData<*const fn(TimeCapsule<T>) -> F>,
    state: State<T>,
}

impl<T, P, F> Nolife<T, P, F>
where
    P: FnOnce(TimeCapsule<T>) -> F,
    F: Future<Output = Never>,
{
    pub fn new(producer: P) -> Self {
        Self {
            producer: Cell::new(Some(producer)),
            active_fut: RefCell::new(None),
            phantom: PhantomData,
            state: State {
                order: Cell::new(None),
            },
        }
    }

    pub fn produce(self: &mut Pin<&mut Self>) {
        let this = self.as_ref();
        let producer = this.producer.take().unwrap();
        let state: *const State<T> = &this.state;
        let time_capsule = TimeCapsule { state };
        let fut = producer(time_capsule);
        *this.active_fut.borrow_mut() = Some(fut);
    }

    pub fn call<'borrow, 'pin, Output: 'borrow, G>(
        self: &'borrow mut Pin<&'pin mut Self>,
        f: G,
    ) -> Output
    where
        // FIXME: we only accept a Fn while we should accept a FnOnce
        G: Fn(&'borrow mut T) -> Output + 'borrow,
    {
        let output: Cell<Result<Output, Option<Box<dyn Any + Send + 'static>>>> =
            Cell::new(Err(None));
        let output_ptr: *const Cell<Result<Output, Option<Box<dyn Any + Send + 'static>>>> =
            &output;
        let f_ptr: *const dyn Fn(&'borrow mut T) -> Output = &f;
        // SAFETY: FIXME
        let f_ptr: *const dyn Fn(&mut T) -> Output = unsafe { std::mem::transmute(f_ptr) };
        let g = move |t: &mut T| {
            // SAFETY: FIXME
            unsafe {
                let output = output_ptr.as_ref().unwrap();
                let f = f_ptr.as_ref().unwrap();
                match catch_unwind(AssertUnwindSafe(|| output.set(Ok(f(t))))) {
                    Ok(()) => {}
                    Err(err) => output.set(Err(Some(err))),
                }
            }
        };
        let g: *const dyn for<'a> Fn(&'a mut T) = &g;
        // transmute so that we can shove a closure that has a local lifetime in the `state` object, that expects
        // a `'static` closure because we are inserting it in the communication channel with the future and can't
        // statically bound its lifetime.
        //
        // SAFETY: the state is dropped by the end of this function, constraining the lifetime to 'pin, the duration
        // of the input borrow.
        let g: *const _ = unsafe { std::mem::transmute(g) };
        let this = self.as_ref();
        let order = Some(g);
        this.state.order.set(order);

        let mut f = this.active_fut.borrow_mut();
        let f = f.as_mut().unwrap();
        // SAFETY: self.active_fut is never moved by self after the first call to produce completes.
        //         self itself is pinned.
        let f = unsafe { Pin::new_unchecked(f) };
        match f.poll(&mut std::task::Context::from_waker(&waker::create())) {
            Poll::Ready(_) => unreachable!(),
            Poll::Pending => {}
        }

        // important for safety.
        this.state.order.set(None);

        match output.into_inner() {
            Ok(output) => output,
            Err(Some(panic_payload)) => resume_unwind(panic_payload),
            Err(None) => panic!("Function was not called by the future."),
        }
    }
}

pub struct NolifeFuture<'a, T> {
    mut_ref: &'a mut T,
    state: *const State<T>,
}

struct State<T> {
    order: Cell<Option<*const dyn for<'a> Fn(&'a mut T)>>,
}

pub struct TimeCapsule<T> {
    state: *const State<T>,
}

impl<T> TimeCapsule<T> {
    pub fn into_future<'a>(&'a mut self, t: &'a mut T) -> NolifeFuture<'a, T> {
        NolifeFuture {
            mut_ref: t,
            state: self.state,
        }
    }
}

/// From <https://blog.aloni.org/posts/a-stack-less-rust-coroutine-100-loc/>, originally from
/// [genawaiter](https://lib.rs/crates/genawaiter).
mod waker {
    use std::task::{RawWaker, RawWakerVTable, Waker};

    pub fn create() -> Waker {
        // Safety: The waker points to a vtable with functions that do nothing. Doing
        // nothing is memory-safe.
        unsafe { Waker::from_raw(RAW_WAKER) }
    }

    const RAW_WAKER: RawWaker = RawWaker::new(std::ptr::null(), &VTABLE);
    const VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

    unsafe fn clone(_: *const ()) -> RawWaker {
        RAW_WAKER
    }
    unsafe fn wake(_: *const ()) {}
    unsafe fn wake_by_ref(_: *const ()) {}
    unsafe fn drop(_: *const ()) {}
}

impl<'a, T> Future for NolifeFuture<'a, T> {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Self::Output> {
        let order = unsafe { self.as_ref().state.as_ref().unwrap().order.take() };
        if let Some(order) = order {
            let f = unsafe { order.as_ref().unwrap() };
            let mut_ref = &mut self.get_mut().mut_ref;
            f(mut_ref);
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn produce_output() {
        let mut nolife = Nolife::new(|mut time_capsule| async move {
            let mut x = 0u32;
            loop {
                time_capsule.into_future(&mut x).await;
                x += 1;
            }
        });

        let mut nolife = unsafe { Pin::new_unchecked(&mut nolife) };
        nolife.produce();
        println!("{}", nolife.call(|x| *x + 42));
        println!("{}", nolife.call(|x| *x + 42));
        nolife.call(|x| *x += 100);
        println!("{}", nolife.call(|x| *x + 42));
    }

    #[test]
    fn hold_reference() {
        let mut nolife = Nolife::new(|mut time_capsule| async move {
            let mut x = 0u32;
            loop {
                time_capsule.into_future(&mut x).await;
                x += 1;
            }
        });

        let mut nolife = unsafe { Pin::new_unchecked(&mut nolife) };
        nolife.produce();

        let x = nolife.call(|x| x);
        *x = 0;

        nolife.call(|x| *x += 1);
        nolife.call(|x| println!("{x}"))
    }
}
