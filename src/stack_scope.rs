use std::{future::Future, marker::PhantomData, mem::ManuallyDrop};

use crate::{Family, Never, Scope, TimeCapsule};

#[repr(transparent)]
pub struct StackScope<'a, T, F>(
    std::ptr::NonNull<Scope<T, F>>,
    PhantomData<&'a mut dyn Fn(&'a mut F)>,
    //PhantomData<&'a mut dyn Fn(&'a mut Scope<T, F>)>,
    //PhantomData<<T as Family<'a>>::Family>,
    //PhantomData<dyn Fn(<T as Family<'a>>::Family)>,
)
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
        Self(
            scope.into(),
            PhantomData,
            //            PhantomData,
            //            PhantomData,
            //            PhantomData,
        )
    }

    pub fn open<P>(&mut self, producer: P)
    where
        P: FnOnce(TimeCapsule<T>) -> F,
    {
        // SAFETY: FIXME
        unsafe { Scope::open(self.0, producer) }
    }

    pub fn enter<'borrow, Output: 'borrow, G>(&'borrow mut self, f: G) -> Output
    where
        G: FnOnce(&'borrow mut <T as Family<'borrow>>::Family) -> Output + 'a,
    {
        // SAFETY: FIXME
        unsafe { Scope::enter(self.0, f) }
    }
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
