use alloc::boxed::Box;
use core::{
    future::Future,
    mem::{self, MaybeUninit},
    ptr::NonNull,
};

use crate::{raw_scope::RawScope, Family, Never, TopScope};

/// A dynamic scope tied to a Box.
///
/// This kind of scopes uses a dynamic allocation.
/// In exchange, it is fully `'static` and can be moved after creation.
#[repr(transparent)]
pub struct BoxScope<T, F: ?Sized = dyn Future<Output = Never> + 'static>(
    core::ptr::NonNull<RawScope<T, F>>,
)
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>;

impl<T, F: ?Sized> Drop for BoxScope<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>,
{
    fn drop(&mut self) {
        // SAFETY: this `Box::from_raw` pairs with a `Box::into_raw`
        // in the `new` constructor. The type `F` is not the same,
        // but `MaybeUninit<F>` and `F` are repr(transparent)-compatible
        // and RawScope is repr(C), so the Box frees the same memory.
        // Furthermore, the `new` constructor ensured that F is properly
        // initialized so it may be dropped.
        //
        // Finally, the drop order of implicitly first dropping self.0.state
        // and THEN self.0.active_fut goes a bit against the typical self-referencing
        // structs assumptions, however self.0.state is a pointer and has no drop glue.
        drop(unsafe { Box::from_raw(self.0.as_ptr()) })
    }
}

impl<T> BoxScope<T>
where
    T: for<'a> Family<'a>,
{
    /// Ties the passed scope to the heap.
    ///
    /// This function erased the `Future` generic type of the [`TopScope`], at the cost
    /// of using a dynamic function call to poll the future.
    ///
    /// If the `Future` generic type can be inferred, it can be more efficient to use [`BoxScope::new`].
    ///
    /// # Panics
    ///
    /// - If `scope` panics.
    pub fn new_dyn<S: TopScope<Family = T>>(scope: S) -> Self
    where
        S::Future: 'static,
    {
        let this = mem::ManuallyDrop::new(BoxScope::new(scope));
        Self(this.0)
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
    /// To store the [`BoxScope`] in a struct, it can be easier to use [`BoxScope::new_dyn`].
    ///
    /// # Panics
    ///
    /// - If `scope` panics.
    pub fn new<S: TopScope<Family = T, Future = F>>(scope: S) -> BoxScope<T, F>
    where
        S: TopScope<Family = T>,
    {
        let raw_scope = Box::new(RawScope::<T, F>::new_uninit());
        let raw_scope: *mut RawScope<T, MaybeUninit<F>> = Box::into_raw(raw_scope);
        struct Guard<Sc> {
            raw_scope: *mut Sc,
        }
        // guard ensures Box is freed on panic (i.e. if scope.run panics)
        let panic_guard = Guard { raw_scope };
        impl<Sc> Drop for Guard<Sc> {
            fn drop(&mut self) {
                // SAFETY: defuse below makes sure this only happens on panic,
                // in this case, self.raw_scope is still in the same uninitialized state
                // and not otherwise being cleaned up, so this `Box::from_raw` pairs with
                // `Box::into_raw` above
                drop(unsafe { Box::from_raw(self.raw_scope) })
            }
        }

        let raw_scope: *mut RawScope<T, F> = raw_scope.cast();

        // SAFETY:
        // 1. `raw_scope` allocated by the `Box` so is valid memory, although the future is not yet initialized
        // 2. `raw_scope` was created from a valid `RawScope::<T, MaybeUninit<F>>`, so `state` is fully initialized.
        //
        // Note: as a post-condition of `RawScope`, `raw_scope` is fully initialized.
        unsafe {
            RawScope::open(raw_scope, scope);
        }

        mem::forget(panic_guard); // defuse guard
                                  // (guard field has no drop glue, so this does not leak anything, it just skips the above `Drop` impl)

        // SAFETY: `raw_scope` allocated by the `Box` so is non-null.
        BoxScope(unsafe { NonNull::new_unchecked(raw_scope) })
    }
}

impl<T, F: ?Sized> BoxScope<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>,
{
    /// Enters the scope, making it possible to access the data frozen inside of the scope.
    ///
    /// # Panics
    ///
    /// - If the passed function panics.
    /// - If the underlying future panics.
    /// - If the underlying future awaits for a future other than the [`crate::FrozenFuture`].
    pub fn enter<'borrow, Output, G>(&'borrow mut self, f: G) -> Output
    where
        G: for<'a> FnOnce(&'borrow mut <T as Family<'a>>::Family) -> Output,
    {
        // SAFETY:
        // 1. `self.0` is valid as a post-condition of `new`.
        // 2. The object pointed to by `self.0` did not move and won't before deallocation.
        // 3. `BoxScope::enter` takes an exclusive reference and the reference passed to `f` cannot escape `f`.
        unsafe { RawScope::enter(self.0, f) }
    }
}
