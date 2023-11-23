use std::{future::Future, mem::ManuallyDrop};

use crate::{Family, Never, Scope, TimeCapsule};

/// A dynamic scope tied to a Box.
///
/// Contrary to [`crate::StackScope`], this kind of scopes uses a dynamic allocation.
/// In exchange, it is fully `'static` and can be moved after creation.
///
/// This scope was already opened from a [`ClosedBoxScope`] and can now be [`BoxScope::enter`]ed.
#[repr(transparent)]
pub struct BoxScope<T, F>(std::ptr::NonNull<Scope<T, F>>)
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>;

/// An unopened, dynamic scope tied to a Box.
///
/// Contrary to [`crate::StackScope`], this kind of scopes uses a dynamic allocation.
/// In exchange, it is fully `'static` and can be moved after creation.
///
/// Use [`ClosedBoxScope::open`] to open this scope and further use it.
pub struct ClosedBoxScope<T, F>(std::ptr::NonNull<Scope<T, F>>)
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
    pub fn new() -> Self {
        let b = Box::new(Scope::new());
        let b = Box::leak(b);
        Self(b.into())
    }

    /// Opens this scope, making it possible to call [`BoxScope::enter`] on the scope.
    ///
    /// # Panics
    ///
    /// - If `producer` panics.
    pub fn open<P>(self, producer: P) -> BoxScope<T, F>
    where
        P: FnOnce(TimeCapsule<T>) -> F,
    {
        // SAFETY: `self.0` is dereference-able due to coming from a `Box`.
        unsafe { Scope::open(self.0, producer) }

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
    /// Creates a new unopened scope.
    pub fn new() -> ClosedBoxScope<T, F> {
        ClosedBoxScope::new()
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
        unsafe { Scope::enter(self.0, f) }
    }
}
