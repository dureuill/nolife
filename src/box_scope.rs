use alloc::boxed::Box;
use core::{
    future::Future,
    mem::{self, MaybeUninit},
    ptr::NonNull,
};

use crate::{raw_scope::RawScope, Family, Never, TopScope};

/// A dynamic scope tied to a Box.
///
/// This scope may or may not be open so a value can be accessed by calling [`Self::get_if_open`] or [`Self::get_mut_if_open`].
///
/// To move to the next value, call [`Self::advance`]. If the scope was closed, this will open it.
/// Open the scope and return a [`BoxScope`] with [`Self::open`].
///
/// This kind of scopes uses a dynamic allocation.
/// In exchange, it is fully `'static` and can be moved after creation.
#[repr(transparent)]
pub struct LazyBoxScope<T, F: ?Sized = dyn Future<Output = Never> + Send + 'static>(
    core::ptr::NonNull<RawScope<T, F>>,
)
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>;

/// A dynamic scope tied to a Box that is guaranteed to be open.
///
/// This scope is always open, so a value can be accessed by calling [`Self::get`] or [`Self::get_mut`].
///
/// To move to the next value, call [`Self::advance`].
///
/// Lastly, [`Self::advance`] and [`Self::get_mut`] can be combined as [`Self::enter`].
///
/// This kind of scopes uses a dynamic allocation.
/// In exchange, it is fully `'static` and can be moved after creation.
pub struct BoxScope<T, F: ?Sized = dyn Future<Output = Never> + Send + 'static>(LazyBoxScope<T, F>)
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>;

impl<T, F: ?Sized> Drop for LazyBoxScope<T, F>
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

impl<T> LazyBoxScope<T>
where
    T: for<'a> Family<'a>,
{
    /// Ties the passed scope to the heap.
    ///
    /// This function erased the `Future` generic type of the [`TopScope`], at the cost
    /// of using a dynamic function call to poll the future.
    ///
    /// If the `Future` generic type can be inferred, it can be more efficient to use [`LazyBoxScope::new`].
    ///
    /// This function requires that the passed `Future` is [`Send`]. If it is not the case,
    /// use [`LazyBoxScope::new_local_dyn`].
    ///
    /// # Panics
    ///
    /// - If `scope` panics.
    pub fn new_dyn<S: TopScope<Family = T>>(scope: S) -> Self
    where
        S::Future: Send + 'static,
    {
        let this = mem::ManuallyDrop::new(LazyBoxScope::new(scope));
        Self(this.0)
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
    /// This function requires that the passed `Future` is [`Send`]. If it is not the case,
    /// use [`BoxScope::new_local_dyn`].
    ///
    /// # Panics
    ///
    /// - If `scope` panics.
    pub fn new_dyn<S: TopScope<Family = T>>(scope: S) -> Self
    where
        S::Future: Send + 'static,
    {
        LazyBoxScope::new_dyn(scope).open()
    }
}

impl<T> LazyBoxScope<T, dyn Future<Output = Never>>
where
    T: for<'a> Family<'a>,
{
    /// Ties the passed scope to the heap.
    ///
    /// This function erased the `Future` generic type of the [`TopScope`], at the cost
    /// of using a dynamic function call to poll the future.
    ///
    /// If the `Future` generic type can be inferred, it can be more efficient to use [`LazyBoxScope::new`].
    ///
    /// Further, this function erased the thread-safety-ness of the underlying future type.
    /// Unless your `Future` is not [`Send`], prefer [`LazyBoxScope::new_dyn`].
    ///
    /// The scope resulting from calling this function will also not be [`Send`].
    ///
    /// # Panics
    ///
    /// - If `scope` panics.
    pub fn new_local_dyn<S: TopScope<Family = T>>(scope: S) -> Self
    where
        S::Future: 'static,
    {
        let this = mem::ManuallyDrop::new(LazyBoxScope::new(scope));
        Self(this.0)
    }
}

impl<T> BoxScope<T, dyn Future<Output = Never>>
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
    /// Further, this function erased the thread-safety-ness of the underlying future type.
    /// Unless your `Future` is not [`Send`], prefer [`BoxScope::new_dyn`].
    ///
    /// The `BoxScope` resulting from calling this function will also not be [`Send`].
    ///
    /// # Panics
    ///
    /// - If `scope` panics.
    pub fn new_local_dyn<S: TopScope<Family = T>>(scope: S) -> Self
    where
        S::Future: 'static,
    {
        LazyBoxScope::new_local_dyn(scope).open()
    }
}

impl<T, F> LazyBoxScope<T, F>
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
    pub fn new<S: TopScope<Family = T, Future = F>>(scope: S) -> LazyBoxScope<T, F> {
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
        LazyBoxScope(unsafe { NonNull::new_unchecked(raw_scope) })
    }
}

impl<T, F: ?Sized> LazyBoxScope<T, F>
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
        unsafe {
            RawScope::advance(self.0);
            RawScope::get_mut(self.0, f).expect("The scope's future did not fill the value")
        }
    }

    /// Whether the scope is currently open or not.
    pub fn is_open(&self) -> bool {
        unsafe { RawScope::is_open(self.0) }
    }

    /// Opens the scope, advancing until a value is available inside of the scope.
    ///
    /// If the scope is already open, this method has no effect.
    ///
    /// The returned [`BoxScope`] guarantees that a value is available inside of the scope.
    ///
    /// # Panics
    ///
    /// - If the underlying future panics.
    /// - If the underlying future awaits for a future other than the [`crate::FrozenFuture`].
    pub fn open(mut self) -> BoxScope<T, F> {
        if !self.is_open() {
            self.advance();
        }
        BoxScope(self)
    }

    /// If the scope is open, accesses its frozen data.
    ///
    /// This method can be called multiple times without modifying the frozen data.
    /// However, it requires that the underlying data and future be `Sync`.
    ///
    /// If the scope is closed, then `f` will not be executed and `None` will be returned.
    ///
    /// # Panics
    ///
    /// - If the passed function panics.
    pub fn get_if_open<'borrow, Output, G>(&'borrow self, f: G) -> Option<Output>
    where
        G: for<'a> FnOnce(&'borrow <T as Family<'a>>::Family) -> Output,
        F: Sync,
        for<'a> <T as Family<'a>>::Family: Sync,
    {
        unsafe { RawScope::get(self.0, f) }
    }

    /// If the scope is open, exclusively accesses its frozen data, allowing for mutation of the data.
    ///
    /// The frozen data is not modified between two calls of this methods that are not separated by [`Self::advance`].
    ///
    /// If the scope is closed, then `f` will not be executed and `None` will be returned.
    ///
    /// # Panics
    ///
    /// - If the passed function panics.
    pub fn get_mut_if_open<'borrow, Output, G>(&'borrow mut self, f: G) -> Option<Output>
    where
        G: for<'a> FnOnce(&'borrow mut <T as Family<'a>>::Family) -> Output,
    {
        unsafe { RawScope::get_mut(self.0, f) }
    }

    /// Advances in the scope until a new value is produced.
    ///
    /// # Panics
    ///
    /// - If the underlying future panics.
    /// - If the underlying future awaits for a future other than the [`crate::FrozenFuture`].
    pub fn advance(&mut self) {
        unsafe { RawScope::advance(self.0) }
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
    pub fn new<S: TopScope<Family = T, Future = F>>(scope: S) -> BoxScope<T, F> {
        LazyBoxScope::new(scope).open()
    }
}

impl<T, F: ?Sized> BoxScope<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>,
{
    /// Accesses the frozen data inside of the scope.
    ///
    /// This method can be called multiple times without modifying the frozen data.
    ///
    /// However, it requires that the underlying data and future be `Sync`.
    ///
    /// # Panics
    ///
    /// - If the passed function panics.
    pub fn get<'borrow, Output, G>(&'borrow self, f: G) -> Output
    where
        G: for<'a> FnOnce(&'borrow <T as Family<'a>>::Family) -> Output,
        F: Sync,
        for<'a> <T as Family<'a>>::Family: Sync,
    {
        self.0.get_if_open(f).expect("scope open by construction")
    }

    /// Exclusively accesses the frozen data inside of the scope, allowing for mutation of the data.
    ///
    /// The frozen data is not modified between two calls of this methods that are not separated by [`Self::advance`].
    ///
    /// # Panics
    ///
    /// - If the passed function panics.
    pub fn get_mut<'borrow, Output, G>(&'borrow mut self, f: G) -> Output
    where
        G: for<'a> FnOnce(&'borrow mut <T as Family<'a>>::Family) -> Output,
    {
        self.0
            .get_mut_if_open(f)
            .expect("scope open by construction")
    }

    /// Advances in the scope until a new value is produced.
    ///
    /// # Panics
    ///
    /// - If the underlying future panics.
    /// - If the underlying future awaits for a future other than the [`crate::FrozenFuture`].
    pub fn advance(&mut self) {
        self.0.advance()
    }

    /// Advances in the scope until a new value is produced, and provide exclusive access to it.
    ///
    /// This method combines [`Self::advance`] and [`Self::get_mut`].
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
        self.advance();
        self.get_mut(f)
    }

    /// Erases the information that the scope was opened, returning to a [`LazyBoxScope`].
    ///
    /// This method does not reset the frozen data inside of the scope. The scope remains open, but this information
    /// is lost to the type system.
    ///
    /// Use this method is you need to store a [`LazyBoxScope`], rather than an [`BoxScope`], in a struct.
    pub fn into_inner(self) -> LazyBoxScope<T, F> {
        self.0
    }
}

// SAFETY:
//
// - No operation can be performed on a `&BoxScope`, so it is trivially `Sync`
// - Operations that require a `&BoxScope` may require that the Family or Future be Sync as well.
unsafe impl<T, F> Sync for LazyBoxScope<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never> + ?Sized,
{
}

// SAFETY:
//
// - `BoxScope` has owning semantic on its inner `RawScope`, so `BoxScope` is `Send`
// if and only if its inner `RawScope` is safe.
//
// Meanwhile `RawScope` is `Send` if its `F` is `Send`.
unsafe impl<T, F> Send for LazyBoxScope<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never> + ?Sized + Send,
{
}
