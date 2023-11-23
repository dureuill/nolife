use crate::{waker, Family, Never, State, TimeCapsule};
use std::{
    cell::RefCell, future::Future, marker::PhantomData, mem::ManuallyDrop, ops::DerefMut, pin::Pin,
    task::Poll,
};

/// Underlying representation of a scope.
pub(crate) struct Scope<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>,
{
    pub(crate) active_fut: RefCell<Option<ManuallyDrop<F>>>,
    pub(crate) phantom: PhantomData<*const fn(TimeCapsule<T>) -> F>,
    pub(crate) state: State<T>,
}

impl<T, F> Scope<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>,
{
    /// Creates a new closed scope.
    pub fn new() -> Self {
        Self {
            active_fut: RefCell::new(None),
            phantom: PhantomData,
            state: Default::default(),
        }
    }

    /// # Safety
    ///
    /// The `this` parameter is *dereference-able*.
    #[allow(unused_unsafe)]
    pub(crate) unsafe fn open<P>(this: std::ptr::NonNull<Self>, producer: P)
    where
        P: FnOnce(TimeCapsule<T>) -> F,
    {
        // SAFETY: `this` is dereference-able as per precondition.
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

    /// # Safety
    ///
    /// The `this` parameter is *dereference-able*.
    #[allow(unused_unsafe)]
    pub(crate) unsafe fn enter<'borrow, Output: 'borrow, G>(
        this: std::ptr::NonNull<Self>,
        f: G,
    ) -> Output
    where
        G: for<'a> FnOnce(&'a mut <T as Family<'a>>::Family) -> Output,
    {
        // SAFETY: `this` is dereference-able as per precondition.
        let this = unsafe { this.as_ref() };

        let mut fut = this.active_fut.borrow_mut();
        let fut = fut.as_mut().unwrap().deref_mut();
        // SAFETY: self.active_fut is never moved by self after the first call to produce completes.
        //         self itself is pinned.
        let fut = unsafe { Pin::new_unchecked(fut) };
        // SAFETY: we didn't do anything particular here before calling `poll`, which may panic, so
        // we have nothing to handle.
        match fut.poll(&mut std::task::Context::from_waker(&waker::create())) {
            Poll::Ready(_) => unreachable!(),
            Poll::Pending => {}
        }
        let state = this.state.0.get();
        // SAFETY: cast the lifetime of the Family to `'borrow`.
        // This is safe to do
        let state: *mut <T as Family>::Family = state.cast();
        let output;
        {
            // SAFETY: The `state` variable has been set to a dereference-able value by the future,
            // or kept its NULL value.
            let state = unsafe {
                state
                    .as_mut()
                    .expect("The scope's future did not fill the value")
            };
            // SAFETY: we're already in a clean state here even if `f` panics.
            // (not doing anything afterwards beside returning `output`)
            output = f(state);
        }
        output
    }
}
