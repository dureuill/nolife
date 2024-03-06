use std::mem::ManuallyDrop;

use crate::{raw_scope::RawScope, Family, TopScope};

/// A dynamic scope tied to a Box.
///
/// This kind of scopes uses a dynamic allocation.
/// In exchange, it is fully `'static` and can be moved after creation.
#[repr(transparent)]
pub struct BoxScope<S>(std::ptr::NonNull<RawScope<S>>)
where
    S: TopScope;

/// An unopened, dynamic scope tied to a Box.
///
/// This kind of scopes uses a dynamic allocation.
/// In exchange, it is fully `'static` and can be moved after creation.
struct ClosedBoxScope<S>(std::ptr::NonNull<RawScope<S>>)
where
    S: TopScope;

impl<S> Drop for ClosedBoxScope<S>
where
    S: TopScope,
{
    fn drop(&mut self) {
        unsafe { drop(Box::from_raw(self.0.as_ptr())) }
    }
}

impl<S> ClosedBoxScope<S>
where
    S: TopScope,
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
    fn open(self, scope: S) -> BoxScope<S> {
        // SAFETY: `self.0` is dereference-able due to coming from a `Box`.
        unsafe { RawScope::open(self.0, scope) }

        let open_scope = BoxScope(self.0);

        // SAFETY: don't call drop on self to avoid double-free since the resource of self was moved to `open_scope`
        std::mem::forget(self);

        open_scope
    }
}

impl<S> Drop for BoxScope<S>
where
    S: TopScope,
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

impl<S> BoxScope<S>
where
    S: TopScope,
{
    /// Creates a new scope from a producer.
    ///
    /// # Panics
    ///
    /// - If `producer` panics.
    pub fn new<F>(scope: S) -> BoxScope<S>
    where
        S: TopScope<Family = F>,
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
        G: for<'a> FnOnce(&'a mut <S::Family as Family<'a>>::Family) -> Output,
    {
        // SAFETY: `self.0` is dereference-able due to coming from a `Box`.
        unsafe { RawScope::enter(self.0, f) }
    }
}
