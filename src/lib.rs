mod box_scope;
pub mod counterexamples;
mod stack_scope;
/// From <https://blog.aloni.org/posts/a-stack-less-rust-coroutine-100-loc/>, originally from
/// [genawaiter](https://lib.rs/crates/genawaiter).
mod waker;

pub use box_scope::BoxScope;
pub use stack_scope::StackScope;

use std::{
    cell::{Cell, RefCell},
    future::Future,
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::DerefMut,
    pin::Pin,
    task::Poll,
};

pub enum Never {}

pub trait Family<'a> {
    type Family: 'a;
}

pub struct Scope<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>,
{
    active_fut: RefCell<Option<ManuallyDrop<F>>>,
    phantom: PhantomData<*const fn(TimeCapsule<T>) -> F>,
    state: State<T>,
}

impl<T, F> Scope<T, F>
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

    #[allow(unused_unsafe)]
    unsafe fn open<P>(this: std::ptr::NonNull<Self>, producer: P)
    where
        P: FnOnce(TimeCapsule<T>) -> F,
    {
        let this = unsafe { this.as_ref() };
        let mut active_fut = this.active_fut.borrow_mut();
        if active_fut.is_some() {
            panic!("Multiple calls to open")
        }
        let state: *const State<T> = &this.state;
        let time_capsule = TimeCapsule { state };
        let fut = producer(time_capsule);
        *active_fut = Some(ManuallyDrop::new(fut));
    }

    #[allow(unused_unsafe)]
    unsafe fn enter<'borrow, 'scope, Output: 'borrow, G>(
        this: std::ptr::NonNull<Self>,
        f: G,
    ) -> Output
    where
        'scope: 'borrow,
        G: FnOnce(&'borrow mut <T as Family<'scope>>::Family) -> Output + 'borrow,
    {
        // SAFETY: FIXME
        let this = unsafe { this.as_ref() };

        let mut fut = this.active_fut.borrow_mut();
        let fut = fut.as_mut().unwrap().deref_mut();
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

pub struct FrozenFuture<'a, 'b, T>
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
    ) -> FrozenFuture<'a, 'b, T>
    where
        'b: 'a,
    {
        FrozenFuture {
            mut_ref: Cell::new(Some(t)),
            state: self.state,
        }
    }
}

impl<'a, 'b, T> Future for FrozenFuture<'a, 'b, T>
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
        let mut scope = Scope::new();
        let mut scope = unsafe { StackScope::new_unchecked(&mut scope) };

        scope.open(
            |mut time_capsule: TimeCapsule<SingleFamily<u32>>| async move {
                let mut x = 0u32;
                loop {
                    time_capsule.freeze(&mut x).await;
                    x += 1;
                }
            },
        );
        println!("{}", scope.enter(|x| *x + 42));
        println!("{}", scope.enter(|x| *x + 42));
        scope.enter(|x| *x += 100);
        println!("{}", scope.enter(|x| *x + 42));
    }

    #[test]
    fn hold_reference() {
        let mut scope = Scope::new();

        let mut scope = unsafe { StackScope::new_unchecked(&mut scope) };
        scope.open(
            |mut time_capsule: TimeCapsule<SingleFamily<u32>>| async move {
                let mut x = 0u32;
                loop {
                    time_capsule.freeze(&mut x).await;
                    x += 1;
                }
            },
        );

        let x = scope.enter(|x| x);
        *x = 0;

        scope.enter(|x| *x += 1);
        scope.enter(|x| println!("{x}"))
    }
}
