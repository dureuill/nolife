use crate::{waker, Family, Never, TopScope};
use std::{
    future::Future,
    marker::PhantomData,
    mem::MaybeUninit,
    pin::Pin,
    ptr::{addr_of_mut, NonNull},
    task::Poll,
};

/// The future resulting from using a time capsule to freeze some scope.
pub struct FrozenFuture<'a, 'b, T>
where
    T: for<'c> Family<'c>,
    'b: 'a,
{
    // Using a pointer here helps ensure that while RawScope<T, F> is dropped,
    // dropping of F can't assert unique access to the .state field by
    // operations that "touch" the FrozenFuture such moving it or passing it to a function.
    // (This probably wasn't exploitable with the scope! macro, but it still seems
    // more correct this way.)
    mut_ref: State<T>,
    state: *mut State<T>,
    marker: PhantomData<&'a mut <T as Family<'b>>::Family>,
}

/// Passed to the closures of a scope so that they can freeze the scope.
pub struct TimeCapsule<T>
where
    T: for<'a> Family<'a>,
{
    pub(crate) state: *mut State<T>,
}

impl<T> Clone for TimeCapsule<T>
where
    T: for<'a> Family<'a>,
{
    fn clone(&self) -> Self {
        *self
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
            mut_ref: Some(NonNull::from(t).cast()),
            state: self.state,
            marker: PhantomData,
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

// This type is a pointer-type and lifetime-erased equivalent of
// Option<&'a mut <T as Family<'b>>::Family>.
//
// NonNull differs in variance, which would typically be corrected
// with a `PhantomData` marker, however a projection like
// `<T as Family<'static>>::Family>` has T invariant already anyway.
pub(crate) type State<T> = Option<NonNull<<T as Family<'static>>::Family>>;

/// Underlying representation of a scope.
// SAFETY: repr C to ensure conversion between RawScope<T, MaybeUninit<F>> and RawScope<T, F>
// does not rely on unstable memory layout.
#[repr(C)]
pub(crate) struct RawScope<T, F: ?Sized>
where
    T: for<'a> Family<'a>,
{
    state: State<T>,
    active_fut: F,
}

impl<T, F> RawScope<T, F>
where
    T: for<'a> Family<'a>,
{
    /// Creates a new closed scope.
    pub fn new_uninit() -> RawScope<T, MaybeUninit<F>> {
        RawScope {
            state: None,
            active_fut: MaybeUninit::uninit(),
        }
    }
}

struct RawScopeFields<T, F: ?Sized>
where
    T: for<'a> Family<'a>,
{
    state: *mut State<T>,
    active_fut: *mut F,
}
impl<T, F: ?Sized> RawScope<T, F>
where
    T: for<'a> Family<'a>,
{
    unsafe fn fields(this: *mut Self) -> RawScopeFields<T, F> {
        RawScopeFields {
            state: unsafe { addr_of_mut!((*this).state) },
            active_fut: unsafe { addr_of_mut!((*this).active_fut) },
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
    /// TODO docs
    pub(crate) unsafe fn open<S: TopScope<Family = T, Future = F>>(this: *mut Self, scope: S)
    where
        T: for<'a> Family<'a>,
        F: Future<Output = Never>,
        S: TopScope<Family = T>,
    {
        let RawScopeFields { state, active_fut } = unsafe { Self::fields(this) };

        let time_capsule = TimeCapsule { state };

        unsafe {
            active_fut.write(scope.run(time_capsule));
        }
    }
}

impl<T, F: ?Sized> RawScope<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>,
{
    /// # Safety
    ///
    /// TODO docs
    #[allow(unused_unsafe)]
    pub(crate) unsafe fn enter<'borrow, Output: 'borrow, G>(this: NonNull<Self>, f: G) -> Output
    where
        G: for<'a> FnOnce(&'a mut <T as Family<'a>>::Family) -> Output,
    {
        let RawScopeFields { state, active_fut } = unsafe { Self::fields(this.as_ptr()) };

        let active_fut: Pin<&mut F> = unsafe { Pin::new_unchecked(&mut *active_fut) };

        match active_fut.poll(&mut std::task::Context::from_waker(&waker::create())) {
            Poll::Ready(never) => match never {},
            Poll::Pending => {}
        }

        let mut_ref = unsafe {
            state
                .read()
                .expect("The scope's future did not fill the value")
                .as_mut()
        };

        f(mut_ref)
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
        let state: &mut State<T> = unsafe { &mut *self.state };
        if state.is_none() {
            let mut_ref = self
                .mut_ref
                .take()
                .expect("poll called several times on the same future");

            *state = Some(mut_ref);
            Poll::Pending
        } else {
            *state = None;
            Poll::Ready(())
        }
    }
}
