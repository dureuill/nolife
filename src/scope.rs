use crate::{waker, Family, Never};
use std::{
    cell::{Cell, RefCell},
    future::Future,
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::DerefMut,
    pin::Pin,
    task::Poll,
};

/// The future resulting from using a time capsule to freeze some scope.
pub struct FrozenFuture<'a, 'b, T>
where
    T: for<'c> Family<'c>,
    'b: 'a,
{
    mut_ref: Cell<Option<&'a mut <T as Family<'b>>::Family>>,
    state: *const State<T>,
}

/// Passed to the closures of a scope so that they can freeze the scope.
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
    /// Freeze a scope, making the data it has borrowed available to the outside.
    ///
    /// Once a scope is frozen, its borrowed data can be accessed through [`crate::BoxScope::enter`].
    ///
    /// For simple cases where you don't need to execute code in the scope between two calls to `enter`,
    /// use [`freeze_forever`].
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

    /// Freeze a scope forever, making the data it has borrowed available to the outside.
    ///
    /// Once a scope is frozen, its borrowed data can be accessed through [`crate::BoxScope::enter`].
    ///
    /// If you need to execute code between two calls to [`crate::BoxScope::enter`], use [`freeze`].
    pub async fn freeze_forever<'a, 'b>(
        &'a mut self,
        t: &'a mut <T as Family<'b>>::Family,
    ) -> Never {
        loop {
            self.freeze(t).await
        }
    }
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

/// Underlying representation of a scope.
pub(crate) struct Scope<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>,
{
    pub(crate) active_fut: RefCell<Option<ManuallyDrop<F>>>,
    phantom: PhantomData<*const fn(TimeCapsule<T>) -> F>,
    state: State<T>,
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

impl<'a, 'b, T> Future for FrozenFuture<'a, 'b, T>
where
    T: for<'c> Family<'c>,
{
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Self::Output> {
        // SAFETY: `state` has been set in the future by the scope
        let state = unsafe { self.state.as_ref().unwrap() };
        if state.0.get().is_null() {
            let mut_ref = self
                .mut_ref
                .take()
                .expect("poll called several times on the same future");
            let mut_ref: *mut <T as Family>::Family = mut_ref;
            // SAFETY: Will be given back a reasonable lifetime in the `enter` method.
            let mut_ref: *mut <T as Family<'static>>::Family = mut_ref.cast();

            state.0.set(mut_ref);
            Poll::Pending
        } else {
            state.0.set(std::ptr::null_mut());
            Poll::Ready(())
        }
    }
}
