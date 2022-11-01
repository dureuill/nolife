use std::{future::Future, mem::ManuallyDrop};

use crate::{Family, Never, Scope, TimeCapsule};

#[repr(transparent)]
pub struct BoxScope<T, F>(std::ptr::NonNull<Scope<T, F>>)
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>;

impl<T, F> Drop for BoxScope<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>,
{
    fn drop(&mut self) {
        // FIXME: SAFETY
        let this = unsafe { self.0.as_ref() };
        // SAFETY: we MUST release the `RefMut` before calling drop on the `Box` otherwise we'll call its
        // destructor after releasing its backing memory, causing uaf
        {
            let mut fut = this.active_fut.borrow_mut();
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
    pub fn new() -> Self {
        let b = Box::new(Scope::new());
        let b = Box::leak(b);
        Self(b.into())
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
        G: FnOnce(&'borrow mut <T as Family<'borrow>>::Family) -> Output + 'static,
    {
        // SAFETY: FIXME
        unsafe { Scope::enter(self.0, f) }
    }
}
