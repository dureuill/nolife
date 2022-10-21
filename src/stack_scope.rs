use std::{future::Future, marker::PhantomData};

use crate::{Family, Never, Scope, TimeCapsule};

#[repr(transparent)]
pub struct StackScope<'a, T, F>(std::ptr::NonNull<Scope<T, F>>, PhantomData<&'a Scope<T, F>>)
where
    T: for<'b> Family<'b>,
    F: Future<Output = Never>;

impl<'a, T, F> StackScope<'a, T, F>
where
    T: for<'b> Family<'b>,
    F: Future<Output = Never>,
{
    /// FIXME: SAFETY
    pub unsafe fn new_unchecked(scope: &'a mut Scope<T, F>) -> Self {
        Self(scope.into(), PhantomData)
    }

    pub fn open<P>(&mut self, producer: P)
    where
        P: FnOnce(TimeCapsule<T>) -> F,
    {
        // SAFETY: FIXME
        unsafe { Scope::open(self.0, producer) }
    }

    pub fn enter<'borrow, 'scope, Output: 'borrow, G>(&'borrow mut self, f: G) -> Output
    where
        'scope: 'borrow,
        G: FnOnce(&'borrow mut <T as Family<'scope>>::Family) -> Output + 'borrow,
    {
        // SAFETY: FIXME
        unsafe { Scope::enter(self.0, f) }
    }
}
