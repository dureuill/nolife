//! Declares `NoFuture`, a handrolled type-erased `Future` that uses similar tricks as `anyhow::Error`.

use std::{
    future::Future,
    mem::ManuallyDrop,
    ptr::NonNull,
    task::{Context, Poll},
};

use crate::Never;

// SAFETY: repr C to ensure that field layout stays as declared.
#[repr(C)]
pub struct NoFuture<F = Erased> {
    vtable: &'static FutureVTable,
    // SAFETY:
    // - last item of the struct so that vtable doesn't move when cast
    // - Manually dropped to make sure we don't drop the _object twice if the outer struct is dropped before being erased.
    _object: ManuallyDrop<F>,
}

struct FutureVTable {
    object_drop: unsafe fn(NonNull<NoFuture>),
    future_poll: for<'a, 'b> unsafe fn(NonNull<NoFuture>, &'a mut Context<'b>) -> Poll<Never>,
}

impl<ErasedOrF> NoFuture<ErasedOrF> {
    /// Erase the future.
    ///
    /// This boils down to a pointer cast, that is always safe to do.
    ///
    /// For the result of the cast to actually be soundly dereferenceable, however, the usual
    /// conditions apply
    pub fn erase(this: NonNull<NoFuture<ErasedOrF>>) -> NonNull<NoFuture<Erased>> {
        this.cast()
    }
}

impl<F> NoFuture<F>
where
    // SAFETY: the lifetime must be static otherwise it is going to be erased.
    // Alternatively, we could put a lifetime parameter 'a on NoFuture and save the lifetime by
    // requesting F: 'a.
    // This doesn't seem super useful because most future erasures are necessary in contexts where there are
    // no lifetimes.
    F: Future<Output = Never> + 'static,
{
    /// Builds a not-yet erased but eraseable future from a future.
    // SAFETY: the only way to build a NoFuture is to start from a future.
    pub fn new(future: F) -> NoFuture<F> {
        let vtable = &FutureVTable {
            object_drop: object_drop::<F>,
            future_poll: future_poll::<F>,
        };
        NoFuture {
            vtable,
            _object: ManuallyDrop::new(future),
        }
    }

    fn unerase(this: NonNull<NoFuture>) -> NonNull<NoFuture<F>> {
        this.cast()
    }
}

impl Future for NoFuture<Erased> {
    type Output = Never;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let future_poll = self.vtable.future_poll;
        let this = NonNull::from(self.get_mut());
        unsafe { future_poll(this, cx) }
    }
}

// We really would prefer to implement drop only on NoFuture<Erased>, but Rust won't let us "specialize" Drop.
impl<ErasedOrF> Drop for NoFuture<ErasedOrF> {
    fn drop(&mut self) {
        let object_drop = self.vtable.object_drop;

        let this = NonNull::from(self);

        let definitely_erased = NoFuture::erase(this);

        // SAFETY:
        // 1. Ensured by the module don't letting you building erased futures from scratch.
        // 2. Only called in Drop implementation, then the object is forgotten.
        //    The future was marked as `ManuallyDrop` in the case `ErasedOrF == F`.
        unsafe { object_drop(definitely_erased) };
    }
}

/// # Safety
///
/// 1. f was obtained by calling `NoFuture::<F>::erase(g)`
/// 2. as this function is dropping the object, it should not be called twice on the same object
unsafe fn object_drop<F: Future<Output = Never> + 'static>(f: NonNull<NoFuture>) {
    // Cast back to NoFuture<F> so that the allocator receives the correct
    // Layout to deallocate the Box's memory.
    let mut f = NoFuture::<F>::unerase(f);
    // SAFETY: per precondition (1)
    let f = unsafe { f.as_mut() };

    // SAFETY: per preconditions
    ManuallyDrop::drop(&mut f._object)
}

/// # Safety
///
/// 1. f was obtained by calling `NoFuture::<F>::erase(g)`
/// 2. f verifies the same guarantees as if it was pinned
unsafe fn future_poll<F: Future<Output = Never> + 'static>(
    f: NonNull<NoFuture>,
    cx: &mut Context,
) -> Poll<Never> {
    let mut f = NoFuture::<F>::unerase(f);
    // SAFETY: per precondition (1)
    let f = unsafe { f.as_mut() };

    let f = &mut *f._object;
    // SAFETY: per precondition (2)
    let f = std::pin::Pin::new_unchecked(f);
    f.poll(cx)
}

pub struct Erased;
