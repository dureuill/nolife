use std::{
    cell::{Cell, RefCell},
    future::Future,
    marker::PhantomData,
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
                order: Cell::new(std::ptr::null_mut()),
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
        let this = self.as_ref();

        let mut fut = this.active_fut.borrow_mut();
        let fut = fut.as_mut().unwrap();
        // SAFETY: self.active_fut is never moved by self after the first call to produce completes.
        //         self itself is pinned.
        let fut = unsafe { Pin::new_unchecked(fut) };
        match fut.poll(&mut std::task::Context::from_waker(&waker::create())) {
            Poll::Ready(_) => unreachable!(),
            Poll::Pending => {}
        }
        let state = this.state.order.get();
        let output;
        {
            // SAFETY: NULL or set by
            // FIXME if f panics, set back to NULL
            // PANICS: future did not fill the value
            let state = unsafe { state.as_mut().unwrap() };
            output = f(state);
        }
        output
    }
}

pub struct NolifeFuture<'a, T> {
    mut_ref: Cell<Option<&'a mut T>>,
    state: *const State<T>,
}

struct State<T> {
    order: Cell<*mut T>,
}

pub struct TimeCapsule<T> {
    state: *const State<T>,
}

impl<T> TimeCapsule<T> {
    pub fn freeze<'a>(&'a mut self, t: &'a mut T) -> NolifeFuture<'a, T> {
        NolifeFuture {
            mut_ref: Cell::new(Some(t)),
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
        // FIXME: Safety
        let state = unsafe { self.state.as_ref().unwrap() };
        if state.order.get().is_null() {
            // FIXME: poll called several times on the same future
            let mut_ref = self.mut_ref.take().unwrap();
            state.order.set(mut_ref);
            Poll::Pending
        } else {
            state.order.set(std::ptr::null_mut());
            Poll::Ready(())
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
                time_capsule.freeze(&mut x).await;
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
                time_capsule.freeze(&mut x).await;
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
