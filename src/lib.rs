#![warn(rustdoc::broken_intra_doc_links)]
#![warn(missing_docs)]
#![doc = include_str!("../README.md")]

mod box_scope;
pub mod counterexamples;
/// From <https://blog.aloni.org/posts/a-stack-less-rust-coroutine-100-loc/>, originally from
/// [genawaiter](https://lib.rs/crates/genawaiter).
mod waker;

pub use box_scope::{BoxScope, ClosedBoxScope};

use std::{
    cell::{Cell, RefCell},
    future::Future,
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::DerefMut,
    pin::Pin,
    task::Poll,
};

/// A type for functions that never return.
///
/// Since this enum has no variant, a value of this type can never actually exist.
/// This type is similar to [`std::convert::Infallible`] and used as a technicality to ensure that
/// functions passed to [`ClosedBoxScope::open`] never return.
///
/// ## Future compatibility
///
/// Should the [the `!` “never” type][never] ever be stabilized, this type would become a type alias and
/// eventually be deprecated. See [the relevant section](std::convert::Infallible#future-compatibility)
/// for more information.
pub enum Never {}

/// Describes a family of types containing a lifetime.
///
/// This type is typically implemented on a helper type to describe the lifetime of the borrowed data we want to freeze in time.
/// See [the module documentation](self) for more information.
pub trait Family<'a> {
    /// An instance with lifetime `'a` of the borrowed data.
    type Family: 'a;
}

/// Underlying representation of a scope.
struct Scope<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>,
{
    active_fut: RefCell<Option<ManuallyDrop<F>>>,
    phantom: PhantomData<*const fn(TimeCapsule<T>) -> F>,
    state: State<T>,
}

impl<T, F> Scope<T, F>
where
    T: for<'a> Family<'a>,
    F: Future<Output = Never>,
{
    /// Creates a new closed scope.
    pub fn new() -> Self {
        Self {
            active_fut: RefCell::new(None),
            phantom: PhantomData,
            state: Default::default(),
        }
    }

    /// # Safety
    ///
    /// The `this` parameter is *dereference-able*.
    #[allow(unused_unsafe)]
    unsafe fn open<P>(this: std::ptr::NonNull<Self>, producer: P)
    where
        P: FnOnce(TimeCapsule<T>) -> F,
    {
        // SAFETY: `this` is dereference-able as per precondition.
        let this = unsafe { this.as_ref() };
        let mut active_fut = this.active_fut.borrow_mut();
        if active_fut.is_some() {
            panic!("Multiple calls to open")
        }
        let state: *const State<T> = &this.state;
        let time_capsule = TimeCapsule { state };
        let fut = producer(time_capsule);
        *active_fut = Some(ManuallyDrop::new(fut));
    }

    /// # Safety
    ///
    /// The `this` parameter is *dereference-able*.
    #[allow(unused_unsafe)]
    unsafe fn enter<'borrow, Output: 'borrow, G>(this: std::ptr::NonNull<Self>, f: G) -> Output
    where
        G: for<'a> FnOnce(&'a mut <T as Family<'a>>::Family) -> Output,
    {
        // SAFETY: `this` is dereference-able as per precondition.
        let this = unsafe { this.as_ref() };

        let mut fut = this.active_fut.borrow_mut();
        let fut = fut.as_mut().unwrap().deref_mut();
        // SAFETY: self.active_fut is never moved by self after the first call to produce completes.
        //         self itself is pinned.
        let fut = unsafe { Pin::new_unchecked(fut) };
        // SAFETY: we didn't do anything particular here before calling `poll`, which may panic, so
        // we have nothing to handle.
        match fut.poll(&mut std::task::Context::from_waker(&waker::create())) {
            Poll::Ready(_) => unreachable!(),
            Poll::Pending => {}
        }
        let state = this.state.0.get();
        // SAFETY: cast the lifetime of the Family to `'borrow`.
        // This is safe to do
        let state: *mut <T as Family>::Family = state.cast();
        let output;
        {
            // SAFETY: The `state` variable has been set to a dereference-able value by the future,
            // or kept its NULL value.
            let state = unsafe {
                state
                    .as_mut()
                    .expect("The scope's future did not fill the value")
            };
            // SAFETY: we're already in a clean state here even if `f` panics.
            // (not doing anything afterwards beside returning `output`)
            output = f(state);
        }
        output
    }
}

/// The future resulting from using a time capsule to freeze some scope.
pub struct FrozenFuture<'a, 'b, T>
where
    T: for<'c> Family<'c>,
    'b: 'a,
{
    mut_ref: Cell<Option<&'a mut <T as Family<'b>>::Family>>,
    state: *const State<T>,
}

struct State<T>(Cell<*mut <T as Family<'static>>::Family>)
where
    T: for<'a> Family<'a>;

impl<T> Default for State<T>
where
    T: for<'a> Family<'a>,
{
    fn default() -> Self {
        Self(Cell::new(std::ptr::null_mut()))
    }
}

/// Passed to the closures of a scope so that they can freeze the scope.
pub struct TimeCapsule<T>
where
    T: for<'a> Family<'a>,
{
    state: *const State<T>,
}

impl<T> TimeCapsule<T>
where
    T: for<'a> Family<'a>,
{
    /// Freeze a scope, making the data it has borrowed available to the outside.
    ///
    /// Once a scope is frozen, its borrowed data can be accessed through [`BoxScope::enter`].
    pub fn freeze<'a, 'b>(
        &'a mut self,
        t: &'a mut <T as Family<'b>>::Family,
    ) -> FrozenFuture<'a, 'b, T>
    where
        'b: 'a,
    {
        FrozenFuture {
            mut_ref: Cell::new(Some(t)),
            state: self.state,
        }
    }
}

impl<'a, 'b, T> Future for FrozenFuture<'a, 'b, T>
where
    T: for<'c> Family<'c>,
{
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Self::Output> {
        // SAFETY: `state` has been set in the future by the scope
        let state = unsafe { self.state.as_ref().unwrap() };
        if state.0.get().is_null() {
            let mut_ref = self
                .mut_ref
                .take()
                .expect("poll called several times on the same future");
            let mut_ref: *mut <T as Family>::Family = mut_ref;
            // SAFETY: Will be given back a reasonable lifetime in the `enter` method.
            let mut_ref: *mut <T as Family<'static>>::Family = mut_ref.cast();

            state.0.set(mut_ref);
            Poll::Pending
        } else {
            state.0.set(std::ptr::null_mut());
            Poll::Ready(())
        }
    }
}

/// Helper type for static types.
///
/// Types that don't contain a lifetime are `'static`, and have one obvious family.
///
/// The usefulness of using `'static` types in the scopes of this crate is dubious, but should you want to do this,
/// for any `T : 'static` pass a `TimeCapsule<SingleFamily<T>>` to your async function.
struct SingleFamily<T: 'static>(PhantomData<T>);
impl<'a, T: 'static> Family<'a> for SingleFamily<T> {
    type Family = T;
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn produce_output() {
        let scope = BoxScope::new();
        let mut scope = scope.open(
            |mut time_capsule: TimeCapsule<SingleFamily<u32>>| async move {
                let mut x = 0u32;
                loop {
                    time_capsule.freeze(&mut x).await;
                    x += 1;
                }
            },
        );

        assert_eq!(scope.enter(|x| *x + 42), 42);
        assert_eq!(scope.enter(|x| *x + 42), 43);
        scope.enter(|x| *x += 100);
        assert_eq!(scope.enter(|x| *x + 42), 145);
    }

    #[test]
    fn panicking_future() {
        let scope = BoxScope::new();
        let mut scope = scope.open(|_: TimeCapsule<SingleFamily<u32>>| async move { panic!() });

        assert!(matches!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                scope.enter(|x| println!("{x}"))
            })),
            Err(_)
        ));

        assert!(matches!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                scope.enter(|x| println!("{x}"))
            })),
            Err(_)
        ));
    }

    #[test]
    fn panicking_future_after_once() {
        let scope = BoxScope::new();
        let mut scope = scope.open(
            |mut time_capsule: TimeCapsule<SingleFamily<u32>>| async move {
                let mut x = 0u32;
                time_capsule.freeze(&mut x).await;
                panic!()
            },
        );

        scope.enter(|x| println!("{x}"));

        assert!(matches!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                scope.enter(|x| println!("{x}"))
            })),
            Err(_)
        ));

        assert!(matches!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                scope.enter(|x| println!("{x}"))
            })),
            Err(_)
        ))
    }

    #[test]
    fn panicking_enter() {
        let scope = BoxScope::new();
        let mut scope = scope.open(
            |mut time_capsule: TimeCapsule<SingleFamily<u32>>| async move {
                let mut x = 0u32;
                loop {
                    time_capsule.freeze(&mut x).await;
                    x += 1;
                }
            },
        );

        scope.enter(|x| assert_eq!(*x, 0));

        assert!(matches!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                scope.enter(|_| panic!())
            })),
            Err(_)
        ));

        // '1' skipped due to panic
        scope.enter(|x| assert_eq!(*x, 2));
    }
}
