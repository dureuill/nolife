use std::{future::Future, mem::ManuallyDrop};

use crate::{raw_scope::RawScope, Family, Never, TopScope};

/// A dynamic scope tied to a Box.
///
/// This kind of scopes uses a dynamic allocation.
/// In exchange, it is fully `'static` and can be moved after creation.
#[repr(transparent)]
pub struct BoxScope<T, F>(std::ptr::NonNull<RawScope<T, F>>)
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>;

/// An unopened, dynamic scope tied to a Box.
///
/// This kind of scopes uses a dynamic allocation.
/// In exchange, it is fully `'static` and can be moved after creation.
struct ClosedBoxScope<T, F>(std::ptr::NonNull<RawScope<T, F>>)
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>;

impl<T, F> Drop for ClosedBoxScope<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>,
{
    fn drop(&mut self) {
        unsafe { drop(Box::from_raw(self.0.as_ptr())) }
    }
}

impl<T, F> ClosedBoxScope<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>,
{
    /// Creates a new unopened scope.
    fn new() -> Self {
        let b = Box::new(RawScope::new());
        let b = Box::leak(b);
        Self(b.into())
    }

    /// Opens this scope, making it possible to call [`BoxScope::enter`] on the scope.
    ///
    /// # Panics
    ///
    /// - If `scope` panics.
    fn open<S: TopScope<Family = T, Future = F>>(self, scope: S) -> BoxScope<T, F> {
        // SAFETY: `self.0` is dereference-able due to coming from a `Box`.
        unsafe { RawScope::open(self.0, scope) }

        let open_scope = BoxScope(self.0);

        // SAFETY: don't call drop on self to avoid double-free since the resource of self was moved to `open_scope`
        std::mem::forget(self);

        open_scope
    }
}

impl<T, F> Drop for BoxScope<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>,
{
    fn drop(&mut self) {
        // SAFETY: created from a Box in the constructor, so dereference-able.
        let this = unsafe { self.0.as_ref() };
        // SAFETY: we MUST release the `RefMut` before calling drop on the `Box` otherwise we'll call its
        // destructor after releasing its backing memory, causing uaf
        {
            let mut fut = this.active_fut.borrow_mut();
            // unwrap: fut was set in open
            let fut = fut.as_mut().unwrap();
            unsafe { ManuallyDrop::drop(fut) };
        }
        unsafe { drop(Box::from_raw(self.0.as_ptr())) }
    }
}

impl<T, F> BoxScope<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>,
{
    /// Creates a new scope from a producer.
    ///
    /// # Panics
    ///
    /// - If `producer` panics.
    pub fn new<S: TopScope<Family = T, Future = F>>(scope: S) -> BoxScope<T, F>
    where
        S: TopScope<Family = T>,
    {
        let closed_scope = ClosedBoxScope::new();
        closed_scope.open(scope)
    }

    /// Enters the scope, making it possible to access the data frozen inside of the scope.
    ///
    /// # Panics
    ///
    /// - If the passed function panics.
    /// - If the underlying future panics.
    /// - If the underlying future awaits for a future other than the [`crate::FrozenFuture`].
    pub fn enter<'borrow, Output: 'borrow, G>(&'borrow mut self, f: G) -> Output
    where
        G: for<'a> FnOnce(&'a mut <T as Family<'a>>::Family) -> Output,
    {
        // SAFETY: `self.0` is dereference-able due to coming from a `Box`.
        unsafe { RawScope::enter(self.0, f) }
    }
}
