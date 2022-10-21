use std::future::Future;

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

    pub fn enter<'borrow, 'scope, Output: 'borrow, G>(&'borrow mut self, f: G) -> Output
    where
        'scope: 'borrow,
        G: FnOnce(&'borrow mut <T as Family<'scope>>::Family) -> Output + 'borrow,
    {
        // SAFETY: FIXME
        unsafe { Scope::enter(self.0, f) }
    }
}
