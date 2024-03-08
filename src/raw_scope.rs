use crate::{nofuture::NoFuture, waker, Family, Never, TopScope};
use std::{
    cell::{Cell, UnsafeCell},
    future::Future,
    marker::PhantomData,
    mem::MaybeUninit,
    pin::Pin,
    ptr::NonNull,
    task::Poll,
};

/// The future resulting from using a time capsule to freeze some scope.
pub struct FrozenFuture<'a, 'b, T>
where
    T: for<'c> Family<'c>,
    'b: 'a,
{
    mut_ref: Option<&'a mut <T as Family<'b>>::Family>,
    state: *const State<T>,
}

/// Passed to the closures of a scope so that they can freeze the scope.
pub struct TimeCapsule<T>
where
    T: for<'a> Family<'a>,
{
    state: *const State<T>,
}

impl<T> Clone for TimeCapsule<T>
where
    T: for<'a> Family<'a>,
{
    fn clone(&self) -> Self {
        Self { state: self.state }
    }
}

impl<T> Copy for TimeCapsule<T> where T: for<'a> Family<'a> {}

impl<T> TimeCapsule<T>
where
    T: for<'a> Family<'a>,
{
    /// Freeze a scope, making the data it has borrowed available to the outside.
    ///
    /// Once a scope is frozen, its borrowed data can be accessed through [`crate::BoxScope::enter`].
    ///
    /// For simple cases where you don't need to execute code in the scope between two calls to `enter`,
    /// use [`Self::freeze_forever`].
    pub fn freeze<'a, 'b>(
        &'a mut self,
        t: &'a mut <T as Family<'b>>::Family,
    ) -> FrozenFuture<'a, 'b, T>
    where
        'b: 'a,
    {
        FrozenFuture {
            mut_ref: Some(t),
            state: self.state,
        }
    }

    /// Freeze a scope forever, making the data it has borrowed available to the outside.
    ///
    /// Once a scope is frozen, its borrowed data can be accessed through [`crate::BoxScope::enter`].
    ///
    /// If you need to execute code between two calls to [`crate::BoxScope::enter`], use [`Self::freeze`].
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
// SAFETY: repr C to ensure that the layout of the struct stays as declared.
#[repr(C)]
pub(crate) struct RawScope<T, F>
where
    T: for<'a> Family<'a>,
{
    phantom: PhantomData<*const fn(TimeCapsule<T>) -> F>,
    state: State<T>,
    // SAFETY:
    // 1. must be the last item of the struct so that the state is still accessible after casting
    // 2. unsafe cell allows in place modifications, and is repr[transparent]
    // 3. maybeuninit allows the future to be setup only once the outer struct has been pinned, and is transparent.
    pub(crate) active_fut: UnsafeCell<MaybeUninit<F>>,
}

impl<T, F> RawScope<T, F>
where
    T: for<'a> Family<'a>,
{
    /// Creates a new closed scope.
    pub fn new() -> Self {
        Self {
            active_fut: UnsafeCell::new(MaybeUninit::uninit()),
            phantom: PhantomData,
            state: Default::default(),
        }
    }
}

impl<T, F> RawScope<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>,
{
    /// # Safety
    ///
    /// 1. The `this` parameter is *dereference-able*.
    ///
    /// # Warning
    ///
    /// Calling this function multiple time will cause the previous future to be dropped.
    #[allow(unused_unsafe)]
    pub(crate) unsafe fn open<S: TopScope<Family = T, Future = F>>(this: NonNull<Self>, scope: S) {
        // SAFETY: `this` is dereference-able as per precondition (1)
        let this = unsafe { this.as_ref() };
        // SAFETY: the mut reference is exclusive because:
        // - the scope is !Sync
        // - it is released by the end of the function
        let active_fut = unsafe { this.active_fut.get().as_mut() }.unwrap();

        let state: *const State<S::Family> = &this.state;
        let time_capsule = TimeCapsule { state };
        // SAFETY: called run from the executor
        let fut = unsafe { scope.run(time_capsule) };
        active_fut.write(fut);
    }

    /// # Safety
    ///
    /// 1. The `this` parameter is *dereference-able*.
    /// 2. `open` was called on `this`
    /// 3. The `this` parameter verifies the pin guarantees
    #[allow(unused_unsafe)]
    pub(crate) unsafe fn enter<'borrow, Output: 'borrow, G>(this: NonNull<Self>, f: G) -> Output
    where
        G: for<'a> FnOnce(&'a mut <T as Family<'a>>::Family) -> Output,
    {
        // SAFETY: `this` is dereference-able as per precondition (1)
        let this = unsafe { this.as_ref() };

        // SAFETY: RawScope is !Sync + the reference is released by the end of the function.
        let fut = this.active_fut.get().as_mut().unwrap();
        // SAFETY: per precondition (2)
        let fut = fut.assume_init_mut();
        // SAFETY: per precondition (3)
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

impl<T, F> RawScope<T, NoFuture<F>>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>,
{
    /// # SAFETY
    ///
    /// - This function must be called **after** calling `open(this)`
    pub(crate) unsafe fn erase(this: NonNull<Self>) -> NonNull<RawScope<T, NoFuture>> {
        this.cast()
    }
}

impl<T, F> RawScope<T, NoFuture<F>>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never> + 'static,
{
    pub(crate) unsafe fn open_erased<S: TopScope<Family = T, Future = F>>(
        this: NonNull<Self>,
        scope: S,
    ) {
        // SAFETY: `this` is dereference-able as per precondition (1)
        let this = unsafe { this.as_ref() };
        // SAFETY: the mut reference is exclusive because:
        // - the scope is !Sync
        // - it is released by the end of the function
        let active_fut = unsafe { this.active_fut.get().as_mut() }.unwrap();

        let state: *const State<S::Family> = &this.state;
        let time_capsule = TimeCapsule { state };
        // SAFETY: called run from the executor
        let fut = unsafe { scope.run(time_capsule) };
        active_fut.write(NoFuture::new(fut));
    }
}

impl<'a, 'b, T> Future for FrozenFuture<'a, 'b, T>
where
    T: for<'c> Family<'c>,
{
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
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
