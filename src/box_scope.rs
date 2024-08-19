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
pub struct BoxScope<T, F: ?Sized = dyn Future<Output = Never> + Send + 'static>(
    core::ptr::NonNull<RawScope<T, F>>,
)
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>;

/// A dynamic scope tied to a Box that is guaranteed to be open.
///
/// Being open, a value can be accessed by calling [`Self::get`] or [`Self::get_mut`].
///
/// To move to the next value, call [`Self::advance`].
///
/// Lastly, [`Self::advance`] and [`Self::get_mut`] can be combined as [`Self::next`].
///
/// This kind of scopes uses a dynamic allocation.
/// In exchange, it is fully `'static` and can be moved after creation.
pub struct OpenBoxScope<T, F: ?Sized = dyn Future<Output = Never> + Send + 'static>(BoxScope<T, F>)
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>;

pub struct IterMut<'borrow, T, F, G, Output>(&'borrow mut BoxScope<T, F>, G)
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never> + ?Sized,
    G: for<'a> FnMut(&'borrow mut <T as Family<'a>>::Family) -> Output;

impl<'borrow, T, F, G, Output> Iterator for IterMut<'borrow, T, F, G, Output>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never> + ?Sized,
    G: for<'a, 'b> FnMut(&'b mut <T as Family<'a>>::Family) -> Output,
{
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.0.enter(&mut self.1))
    }
}

pub struct IntoIter<T, F, G, Output>(BoxScope<T, F>, G)
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never> + ?Sized,
    G: for<'a, 'b> FnMut(&'b mut <T as Family<'a>>::Family) -> Output;

impl<T, F, G, Output> Iterator for IntoIter<T, F, G, Output>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never> + ?Sized,
    G: for<'a, 'b> FnMut(&'b mut <T as Family<'a>>::Family) -> Output,
{
    type Item = Output;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.0.enter(&mut self.1))
    }
}

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
    /// This function requires that the passed `Future` is [`Send`] and [`Sync`]. If it is not the case,
    /// use [`BoxScope::new_local_dyn`].
    ///
    /// # Panics
    ///
    /// - If `scope` panics.
    pub fn new_dyn<S: TopScope<Family = T>>(scope: S) -> Self
    where
        S::Future: Send + 'static,
    {
        let this = mem::ManuallyDrop::new(BoxScope::new(scope));
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
    /// Unless your `Future` is not [`Send`] or not [`Sync`], prefer [`BoxScope::new_dyn`].
    ///
    /// The `BoxScope` resulting from calling this function will also not be [`Send`] or [`Sync`].
    ///
    /// # Panics
    ///
    /// - If `scope` panics.
    pub fn new_local_dyn<S: TopScope<Family = T>>(scope: S) -> Self
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
    pub fn new<S: TopScope<Family = T, Future = F>>(scope: S) -> BoxScope<T, F> {
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
    #[doc(alias = "next")]
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
    /// The returned [`OpenBoxScope`] guarantees that a value is available inside of the scope.
    ///
    /// # Panics
    ///
    /// - If the underlying future panics.
    /// - If the underlying future awaits for a future other than the [`crate::FrozenFuture`].
    pub fn open(mut self) -> OpenBoxScope<T, F> {
        if !self.is_open() {
            self.advance();
        }
        OpenBoxScope(self)
    }

    /// If the scope is open, accesses its frozen data.
    ///
    /// This method can be called multiple times without modifying the frozen data.
    /// However, it requires that the underlying data and future be `Sync`.
    ///
    /// If the scope is closed, then `f` will not be executed and `None` will be returned.
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
    pub fn get_mut_if_open<'borrow, Output, G>(&'borrow mut self, f: G) -> Option<Output>
    where
        G: for<'a> FnOnce(&'borrow mut <T as Family<'a>>::Family) -> Output,
    {
        unsafe { RawScope::get_mut(self.0, f) }
    }

    /// Returns an infinite iterator that accesses the successive values inside of the scope through the passed closure.
    pub fn iter_mut<'borrow, Output, G>(
        &'borrow mut self,
        f: G,
    ) -> IterMut<'borrow, T, F, G, Output>
    where
        G: for<'a, 'b> FnMut(&'b mut <T as Family<'a>>::Family) -> Output,
    {
        if !self.is_open() {
            self.advance();
        }
        IterMut(self, f)
    }

    /// Returns an infinite iterator that accesses the successive values inside of the scope through the passed closure.
    pub fn into_iter<Output, G>(mut self, f: G) -> IntoIter<T, F, G, Output>
    where
        G: for<'a, 'borrow> FnMut(&'borrow mut <T as Family<'a>>::Family) -> Output,
    {
        if !self.is_open() {
            self.advance();
        }
        IntoIter(self, f)
    }

    /// Advances in the scope until a new value is produced.
    pub fn advance(&mut self) {
        unsafe { RawScope::advance(self.0) }
    }
}

impl<T, F: ?Sized> OpenBoxScope<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>,
{
    /// Accesses the frozen data inside of the scope.
    ///
    /// This method can be called multiple times without modifying the frozen data.
    ///
    /// However, it requires that the underlying data and future be `Sync`.
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
    pub fn get_mut<'borrow, Output, G>(&'borrow mut self, f: G) -> Output
    where
        G: for<'a> FnOnce(&'borrow mut <T as Family<'a>>::Family) -> Output,
    {
        self.0
            .get_mut_if_open(f)
            .expect("scope open by construction")
    }

    /// Advance in the scope until a new value is produced.
    pub fn advance(&mut self) {
        self.0.advance()
    }

    /// Advances in the scope until a new value is produced, and provide exclusive access to it.
    ///
    /// This method combines [`Self::advance`] and [`Self::get_mut`].
    #[doc(alias = "enter")]
    pub fn next<'borrow, Output, G>(&'borrow mut self, f: G) -> Output
    where
        G: for<'a> FnOnce(&'borrow mut <T as Family<'a>>::Family) -> Output,
    {
        self.advance();
        self.get_mut(f)
    }

    /// Returns an infinite iterator that accesses the successive values inside of the scope through the passed closure.
    pub fn iter_mut<'borrow, Output, G>(
        &'borrow mut self,
        f: G,
    ) -> IterMut<'borrow, T, F, G, Output>
    where
        G: for<'a, 'b> FnMut(&'b mut <T as Family<'a>>::Family) -> Output,
    {
        self.0.iter_mut(f)
    }

    /// Returns an infinite iterator that accesses the successive values inside of the scope through the passed closure.
    pub fn into_iter<Output, G>(self, f: G) -> IntoIter<T, F, G, Output>
    where
        G: for<'a, 'borrow> FnMut(&'borrow mut <T as Family<'a>>::Family) -> Output,
    {
        self.0.into_iter(f)
    }

    /// Erases the information that the scope was opened, returning to a [`BoxScope`].
    ///
    /// This method does not reset the frozen data inside of the scope. The scope remains open, but this information
    /// is lost to the type system.
    ///
    /// Use this method is you need to store a `BoxScope`, rather than an [`OpenBoxScope`], in a struct.
    pub fn into_inner(self) -> BoxScope<T, F> {
        self.0
    }
}

// SAFETY:
//
// - No operation can be performed on a `&BoxScope`, so it is trivially `Sync`
// - Operations that require a `&BoxScope` may require that the Family or Future be Sync as well.
unsafe impl<T, F> Sync for BoxScope<T, F>
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
unsafe impl<T, F> Send for BoxScope<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never> + ?Sized + Send,
{
}
