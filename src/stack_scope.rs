use std::{future::Future, marker::PhantomData, mem::ManuallyDrop};

use crate::{Family, Never, Scope, TimeCapsule};

/// A scope that is tied to a syntactic scope.
///
/// Spawning such a scope is `unsafe`, as it requires the underlying [`Scope`] object to remain pinned for the entirety
/// of its lifetime after being passed to a [`StackScope`] (see [`StackScope::new_unchecked`] for more information).
#[repr(transparent)]
pub struct StackScope<'a, T, F>(
    std::ptr::NonNull<Scope<T, F>>,
    PhantomData<&'a mut dyn Fn(&'a mut F)>,
)
where
    T: for<'b> Family<'b>,
    F: Future<Output = Never>;

impl<'a, T, F> StackScope<'a, T, F>
where
    T: for<'b> Family<'b>,
    F: Future<Output = Never>,
{
    /// Create a new unopened scope from borrowing a low-level [`Scope`] object.
    ///
    /// ## Safety
    ///
    /// - Although this crate does not use `pin`, the passed scope **must** provide the same guarantees as if it had been pinned.
    /// - As an additional soundness condition, the passed scope **shall not** be reused for another call to `new_unchecked`.
    ///
    /// The [`crate::stack_scope!`] and [`crate::open_stack_scope!`] macros provides a safe way of spawning a [`StackScope`].
    pub unsafe fn new_unchecked(scope: &'a mut Scope<T, F>) -> Self {
        Self(scope.into(), PhantomData)
    }

    /// Opens this scope, making it possible to call [`Self::enter`] on the scope.
    ///
    /// # Panics
    ///
    /// - If this method is called on an already opened scope.
    pub fn open<P>(&mut self, producer: P)
    where
        P: FnOnce(TimeCapsule<T>) -> F,
    {
        // SAFETY: FIXME
        unsafe { Scope::open(self.0, producer) }
    }

    /// Enters the scope, making it possible to access the data frozen inside of the scope.
    ///
    /// # Panics
    ///
    /// - If this method is called on an unopened scope.
    /// - If the passed function panics.
    /// - If the underlying future panics.
    /// - If the underlying future awaits for a future other than the [`crate::FrozenFuture`].
    pub fn enter<'borrow, Output: 'borrow, G>(&'borrow mut self, f: G) -> Output
    where
        G: FnOnce(&'borrow mut <T as Family<'borrow>>::Family) -> Output + 'a,
    {
        // SAFETY: FIXME
        unsafe { Scope::enter(self.0, f) }
    }
}

/// Safely creates a [`StackScope`].
///
/// # Example
///
/// ```
/// use nolife::{SingleFamily, TimeCapsule, stack_scope};
///
/// stack_scope!(scope);
/// scope.open(|mut time_capsule: TimeCapsule<SingleFamily<u32>>| async move {
///         let mut x = 0u32;
///         loop {
///             time_capsule.freeze(&mut x).await;
///             x += 1;
///         }
/// });
///
/// assert_eq!(scope.enter(|x| *x + 42), 42);
/// assert_eq!(scope.enter(|x| *x + 42), 43);
/// scope.enter(|x| *x += 100);
/// assert_eq!(scope.enter(|x| *x + 42), 145);
/// ```
#[macro_export]
macro_rules! stack_scope {
    ($id:ident) => {
        let mut $id = $crate::Scope::new();
        // SAFETY: the original identifier is shadowed, ensuring it is never reused.
        let mut $id = unsafe { $crate::StackScope::new_unchecked(&mut $id) };
    };
}

/// Convenience macro to safely create a [`StackScope`] and open it on the spot.
///
/// # Example
///
/// ```
/// use nolife::{SingleFamily, TimeCapsule, open_stack_scope};
///
/// open_stack_scope!(
///     scope = |mut time_capsule: TimeCapsule<SingleFamily<u32>>| async move {
///         let mut x = 0u32;
///         loop {
///             time_capsule.freeze(&mut x).await;
///             x += 1;
///         }
///     }
/// );
///
/// assert_eq!(scope.enter(|x| *x + 42), 42);
/// assert_eq!(scope.enter(|x| *x + 42), 43);
/// scope.enter(|x| *x += 100);
/// assert_eq!(scope.enter(|x| *x + 42), 145);
/// ```
#[macro_export]
macro_rules! open_stack_scope {
    ($id: ident = $async_func: expr) => {
        $crate::stack_scope!($id);
        $id.open($async_func);
    };
}

impl<'a, T, F> Drop for StackScope<'a, T, F>
where
    T: for<'b> Family<'b>,
    F: Future<Output = Never>,
{
    fn drop(&mut self) {
        // FIXME: SAFETY
        let this = unsafe { self.0.as_ref() };
        let mut fut = this.active_fut.borrow_mut();
        let fut = fut.as_mut().unwrap();
        unsafe { ManuallyDrop::drop(fut) };
    }
}
