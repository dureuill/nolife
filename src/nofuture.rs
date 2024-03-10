//! Declares `NoFuture`, a handrolled type-erased `Future` that uses similar tricks as `anyhow::Error`.

use std::{
    alloc::Layout,
    future::Future,
    mem::ManuallyDrop,
    ptr::NonNull,
    task::{Context, Poll},
};

use crate::Never;

// SAFETY: repr C to ensure that field layout stays as declared.
#[repr(C)]
pub struct NoFuture<F = Erased> {
    // SAFETY:
    // - must be the first item of the struct so we can read a pointer to the struct as a pointer to the vtable.
    metadata: Metadata,

    marker: std::marker::PhantomPinned,
    // SAFETY:
    // - last item of the struct so that vtable doesn't move when cast
    // - Manually dropped to make sure we don't drop the _object twice if the outer struct is dropped before being erased.
    _object: ManuallyDrop<F>,
}

#[derive(Clone, Copy)]
struct Metadata {
    vtable: &'static FutureVTable,
    outer_layout: Layout,
}

struct FutureVTable {
    object_drop: unsafe fn(NonNull<NoFuture>),
    future_poll: for<'a, 'b> unsafe fn(NonNull<NoFuture>, &'a mut Context<'b>) -> Poll<Never>,
}

impl<F> NoFuture<F>
where
    F: Future,
{
    /// Erase the future.
    ///
    /// This boils down to a pointer cast, that is always safe to do.
    ///
    /// For the result of the cast to actually be soundly dereferenceable, however, the usual
    /// conditions apply
    pub fn erase(this: NonNull<NoFuture<F>>) -> NonNull<NoFuture<Erased>> {
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
    pub fn new(future: F, outer_layout: Layout) -> NoFuture<F> {
        let vtable = &FutureVTable {
            object_drop: object_drop::<F>,
            future_poll: future_poll::<F>,
        };
        NoFuture {
            metadata: Metadata {
                vtable,
                outer_layout,
            },
            marker: std::marker::PhantomPinned,
            _object: ManuallyDrop::new(future),
        }
    }

    fn unerase(this: NonNull<NoFuture>) -> NonNull<NoFuture<F>> {
        this.cast()
    }
}

// Reads the vtable out of `p`. This is the same as `p.as_ref().vtable`, but
// avoids converting `p` into a reference.
unsafe fn vtable(p: NonNull<NoFuture<Erased>>) -> &'static FutureVTable {
    // NOTE: This assumes that `FutureVTable` is the first field of NoFuture.
    let metadata: Metadata = unsafe { *(p.as_ptr() as *const Metadata) };
    metadata.vtable
}

// Reads the vtable out of `p`. This is the same as `p.as_ref().vtable`, but
// avoids converting `p` into a reference.
unsafe fn outer_layout(p: NonNull<NoFuture<Erased>>) -> Layout {
    // NOTE: This assumes that `FutureVTable` is the first field of NoFuture.
    let metadata: Metadata = unsafe { *(p.as_ptr() as *const Metadata) };
    metadata.outer_layout
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

pub trait RawFuture {
    type Output;

    unsafe fn poll(this: NonNull<Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;

    unsafe fn drop_future(this: NonNull<Self>);

    unsafe fn dealloc_outer<T>(this: NonNull<Self>, outer: *mut T);
}

impl RawFuture for NoFuture<Erased> {
    type Output = Never;

    unsafe fn poll(this: NonNull<Self>, cx: &mut Context<'_>) -> Poll<Never> {
        let vtable = vtable(this);
        (vtable.future_poll)(this, cx)
    }

    unsafe fn drop_future(this: NonNull<Self>) {
        let vtable = vtable(this);
        (vtable.object_drop)(this)
    }

    unsafe fn dealloc_outer<T>(this: NonNull<Self>, outer: *mut T) {
        let outer_layout = outer_layout(this);
        std::alloc::dealloc(outer as *mut _, outer_layout)
    }
}

impl<F> RawFuture for F
where
    F: Future,
{
    type Output = F::Output;

    unsafe fn poll(mut this: NonNull<Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = this.as_mut();
        let pinned = std::pin::Pin::new_unchecked(this);
        pinned.poll(cx)
    }

    unsafe fn drop_future(this: NonNull<Self>) {
        std::ptr::drop_in_place(this.as_ptr())
    }

    unsafe fn dealloc_outer<T>(_this: NonNull<Self>, outer: *mut T) {
        drop(Box::from_raw(outer))
    }
}
