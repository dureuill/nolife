use std::{alloc::Layout, cell::UnsafeCell, future::Future, ptr::NonNull};

use crate::{
    nofuture::{NoFuture, RawFuture},
    raw_scope::RawScope,
    Family, Never, TopScope,
};

/// A dynamic scope tied to a Box.
///
/// This kind of scopes uses a dynamic allocation.
/// In exchange, it is fully `'static` and can be moved after creation.
#[repr(transparent)]
pub struct BoxScope<T, F = NoFuture>(std::ptr::NonNull<RawScope<T, F>>)
where
    T: for<'a> Family<'a>,
    F: RawFuture<Output = Never>;

impl<T, F> Drop for BoxScope<T, F>
where
    T: for<'a> Family<'a>,
    F: RawFuture<Output = Never>,
{
    fn drop(&mut self) {
        // SAFETY: created from a Box in the constructor, so dereference-able.
        let this = self.0.as_ptr();
        // SAFETY: we MUST release the future before calling drop on the `Box` otherwise we'll call its
        // destructor after releasing its backing memory, causing uaf
        {
            let fut = unsafe { std::ptr::addr_of!((*this).active_fut) };
            let fut = UnsafeCell::raw_get(fut);
            let fut: *mut F = fut.cast();
            let fut = NonNull::new(fut).unwrap();
            // SAFETY: a call to `RawScope::open` happened
            unsafe {
                RawFuture::drop_future(fut);
                let this = self.0.as_ptr();
                RawFuture::dealloc_outer(fut, this);
            }
        }
    }
}

impl<T> BoxScope<T, NoFuture>
where
    T: for<'a> Family<'a>,
{
    /// Ties the passed scope to the heap.
    ///
    /// This function erased the `Future` generic type of the [`TopScope`], at the cost
    /// of using a dynamic function call to poll the future.
    ///
    /// If the `Future` generic type can be inferred, it can be more efficient to use [`BoxScope::new_typed`].
    ///
    /// # Panics
    ///
    /// - If `scope` panics.
    pub fn new_erased<S: TopScope<Family = T>>(scope: S) -> BoxScope<T, NoFuture>
    where
        S::Future: 'static,
    {
        let outer_layout = Layout::new::<RawScope<T, NoFuture<S::Future>>>();
        let raw_scope = NonNull::new(
            unsafe { std::alloc::alloc(outer_layout) } as *mut RawScope<T, NoFuture<S::Future>>
        )
        .unwrap();

        unsafe { raw_scope.as_ptr().write(RawScope::new()) };

        // SAFETY: `self.0` is dereference-able due to coming from a `Box`.
        unsafe { RawScope::open_erased(raw_scope, outer_layout, scope) }

        // SAFETY: open was called as part of `BoxScope::new`
        let erased_raw_scope = unsafe { RawScope::erase(raw_scope) };
        BoxScope(erased_raw_scope)
    }
}

impl<T, F> BoxScope<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>,
{
    /// Ties the passed scope to the heap.
    ///
    /// This function retains the `Future` generic type from the [`TopScope`].
    /// To store the [`BoxScope`] in a struct, it can be easier to use [`BoxScope::new_erased`].
    ///
    /// # Panics
    ///
    /// - If `scope` panics.
    pub fn new_typed<S: TopScope<Family = T, Future = F>>(scope: S) -> BoxScope<T, F>
    where
        S: TopScope<Family = T>,
    {
        let raw_scope = Box::new(RawScope::new());
        let raw_scope = Box::leak(raw_scope).into();

        // SAFETY: `self.0` is dereference-able due to coming from a `Box`.
        unsafe { RawScope::open(raw_scope, scope) }

        BoxScope(raw_scope)
    }
}

impl<T, F> BoxScope<T, F>
where
    T: for<'a> Family<'a>,
    F: RawFuture<Output = Never>,
{
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
