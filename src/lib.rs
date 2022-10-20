use std::{
    cell::{Cell, RefCell},
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::Poll,
};

pub enum Never {}

pub trait Family<'a> {
    type Family: 'a;
}

pub struct Nolife<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>,
{
    active_fut: RefCell<Option<F>>,
    phantom: PhantomData<*const fn(TimeCapsule<T>) -> F>,
    state: State<T>,
}

impl<T, F> Nolife<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>,
{
    pub fn new() -> Self {
        Self {
            active_fut: RefCell::new(None),
            phantom: PhantomData,
            state: Default::default(),
        }
    }

    pub fn produce<P>(self: &mut Pin<&mut Self>, producer: P)
    where
        P: FnOnce(TimeCapsule<T>) -> F,
    {
        let this = self.as_ref();
        let mut active_fut = this.active_fut.borrow_mut();
        if active_fut.is_some() {
            panic!("Multiple calls to produce")
        }
        let state: *const State<T> = &this.state;
        let time_capsule = TimeCapsule { state };
        let fut = producer(time_capsule);
        *active_fut = Some(fut);
    }

    pub fn call<'borrow, 'pin, 'scope, Output: 'borrow, G>(
        self: &'borrow mut Pin<&'pin mut Self>,
        f: G,
    ) -> Output
    where
        'scope: 'borrow,
        // FIXME: we only accept a Fn while we should accept a FnOnce
        G: Fn(&'borrow mut <T as Family<'scope>>::Family) -> Output + 'borrow,
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
        let state = this.state.0.get();
        // SAFETY: papering over the lifetime requirements here!!!
        let state: *mut <T as Family>::Family = state.cast();
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

pub struct NolifeFuture<'a, 'b, T>
where
    T: for<'c> Family<'c>,
    'b: 'a,
{
    mut_ref: Cell<Option<&'a mut <T as Family<'b>>::Family>>,
    state: *const State<T>,
}

struct State<T>(Cell<*mut <T as Family<'static>>::Family>)
where
    T: for<'a> Family<'a>;

impl<T> Default for State<T>
where
    T: for<'a> Family<'a>,
{
    fn default() -> Self {
        Self(Cell::new(std::ptr::null_mut()))
    }
}

pub struct TimeCapsule<T>
where
    T: for<'a> Family<'a>,
{
    state: *const State<T>,
}

impl<T> TimeCapsule<T>
where
    T: for<'a> Family<'a>,
{
    pub fn freeze<'a, 'b>(
        &'a mut self,
        t: &'a mut <T as Family<'b>>::Family,
    ) -> NolifeFuture<'a, 'b, T>
    where
        'b: 'a,
    {
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

impl<'a, 'b, T> Future for NolifeFuture<'a, 'b, T>
where
    T: for<'c> Family<'c>,
{
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Self::Output> {
        // FIXME: Safety
        let state = unsafe { self.state.as_ref().unwrap() };
        if state.0.get().is_null() {
            // FIXME: poll called several times on the same future
            let mut_ref = self.mut_ref.take().unwrap();
            let mut_ref: *mut <T as Family>::Family = mut_ref;
            // FIXME: SAFETY!!!
            let mut_ref: *mut <T as Family<'static>>::Family = mut_ref.cast();

            state.0.set(mut_ref);
            Poll::Pending
        } else {
            state.0.set(std::ptr::null_mut());
            Poll::Ready(())
        }
    }
}

pub struct SingleFamily<T: 'static>(PhantomData<T>);
impl<'a, T: 'static> Family<'a> for SingleFamily<T> {
    type Family = T;
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn produce_output() {
        let mut nolife = Nolife::new();

        let mut nolife = unsafe { Pin::new_unchecked(&mut nolife) };
        nolife.produce(
            |mut time_capsule: TimeCapsule<SingleFamily<u32>>| async move {
                let mut x = 0u32;
                loop {
                    time_capsule.freeze(&mut x).await;
                    x += 1;
                }
            },
        );
        println!("{}", nolife.call(|x| *x + 42));
        println!("{}", nolife.call(|x| *x + 42));
        nolife.call(|x| *x += 100);
        println!("{}", nolife.call(|x| *x + 42));
    }

    #[test]
    fn hold_reference() {
        let mut nolife = Nolife::new();

        let mut nolife = unsafe { Pin::new_unchecked(&mut nolife) };
        nolife.produce(
            |mut time_capsule: TimeCapsule<SingleFamily<u32>>| async move {
                let mut x = 0u32;
                loop {
                    time_capsule.freeze(&mut x).await;
                    x += 1;
                }
            },
        );

        let x = nolife.call(|x| x);
        *x = 0;

        nolife.call(|x| *x += 1);
        nolife.call(|x| println!("{x}"))
    }

    struct Contravariant<'a> {
        f: Box<dyn Fn(&'a u32) -> u32>,
    }

    impl<'a> Family<'a> for Contravariant<'a> {
        type Family = Self;
    }

    #[test]
    fn contravariant() {}
}
