#![warn(rustdoc::broken_intra_doc_links)]
#![warn(missing_docs)]

//! Open a scope and then freeze it in time for future access.
//!
//! This crate allows constructing structs that contain references and keeping them alive alongside the data they reference,
//! without a lifetime.
//!
//! This is especially useful for zero-copy parsers that construct elaborate (and possibly costly) representations that borrow
//! the source data.
//!
//! In this regard, this crate has similar use cases as [`yoke`].
//!
//! Unlike [`yoke`], this crate achieves that by leveraging `async` functions. At their core, `async` functions are self-referential
//! structs. this crate simply provides a way to ex-filtrate references outside of the async function, in a controlled manner.
//!
//!
//! # Soundness note
//!
//! While the code of this crate was reviewed carefully and tested using `miri`, this crate is using **extremely unsafe** code that is
//! *easy to get wrong*.
//!
//! To emphasize what is already written in the license, **use at your own risk**.
//!
//! # Using this crate
//!
//! After you identified the data and its borrowed representation that you'd like to access without a lifetime,
//! using this crate will typically encompass a few steps:
//!
//! 1. Define a helper type that will express where the lifetimes of the borrowed representation live.
//!    Given the following types:
//!
//!     ```
//!     struct MyData(Vec<u8>);
//!     struct MyParsedData<'a>(&'a mut MyData, /* ... */);
//!     ```
//!
//!    we want to define an helper type that will implement the [`Family`] trait and tie its lifetime to `MyParsedData`'s lifetime.
//!
//!     ```rust
//!     # struct MyData(Vec<u8>);
//!     # struct MyParsedData<'a>(&'a mut MyData, /* ... */);
//!     struct MyParsedDataFamily; // empty type, no lifetime.
//!     impl<'a> nolife::Family<'a> for MyParsedDataFamily {
//!         type Family = MyParsedData<'a>; // Indicates how the type is tied to the trait's lifetime.
//!         // you generally want to replace all lifetimes in the struct with the one of the trait.
//!     }
//!     ```
//!
//!     2. Define an async function that setups the data and its borrowed representation:
//!
//!      ```
//!      # struct MyData(Vec<u8>);
//!      # struct MyParsedData<'a>(&'a mut MyData, /* ... */);
//!      # struct MyParsedDataFamily; // empty type, no lifetime.
//!      # impl<'a> nolife::Family<'a> for MyParsedDataFamily {
//!      #     type Family = MyParsedData<'a>; // Indicates how the type is tied to the trait's lifetime.
//!      #     // you generally want to replace all lifetimes in the struct with the one of the trait.
//!      # }
//!      async fn my_scope(mut time_capsule: nolife::TimeCapsule<MyParsedDataFamily /* 👈 use the helper type we declared */>,
//!                        data_source: Vec<u8> /* 👈 all parameters that allow to build a `MyData` */)
//!      -> nolife::Never /* 👈 will be returned from loop */ {
//!          let mut data = MyData(data_source);
//!          let mut parsed_data = MyParsedData(&mut data); // imagine that this step is costly...
//!          loop /* 👈 will be coerced to a `Never` */ {
//!              time_capsule.freeze(&mut parsed_data).await; // gives access to the parsed data to the outside.
//!                                /* 👆 reference to the borrowed data */
//!          }
//!      }
//!      ```
//!
//!      3. Open a scope, either on the [stack](`StackScope`) or in a [box](`BoxScope`),
//!      using the previously written async function:
//!
//!      ```
//!      # struct MyData(Vec<u8>);
//!      # struct MyParsedData<'a>(&'a mut MyData, /* ... */);
//!      # struct MyParsedDataFamily; // empty type, no lifetime.
//!      # impl<'a> nolife::Family<'a> for MyParsedDataFamily {
//!      #     type Family = MyParsedData<'a>; // Indicates how the type is tied to the trait's lifetime.
//!      #     // you generally want to replace all lifetimes in the struct with the one of the trait.
//!      # }
//!      # async fn my_scope(mut time_capsule: nolife::TimeCapsule<MyParsedDataFamily /* 👈 use the helper type we declared */>,
//!      #                   data_source: Vec<u8> /* 👈 all parameters that allow to build a `MyData` */)
//!      # -> nolife::Never /* 👈 will be returned from loop */ {
//!      #     let mut data = MyData(data_source);
//!      #     let mut parsed_data = MyParsedData(&mut data); // imagine that this step is costly...
//!      #     loop /* 👈 will be coerced to a `Never` */ {
//!      #         time_capsule.freeze(&mut parsed_data).await; // gives access to the parsed data to the outside.
//!      #                           /* 👆 reference to the borrowed data */
//!      #     }
//!      # }
//!      let mut scope = nolife::BoxScope::new();
//!      scope.open(|time_capsule| my_scope(time_capsule, vec![0, 1, 2]));
//!      // You can now store the open scope anywhere you want.
//!      ```
//!
//!      4. Lastly, enter the scope to retrieve access to the referenced value.
//!      ```
//!      # struct MyData(Vec<u8>);
//!      # struct MyParsedData<'a>(&'a mut MyData, /* ... */);
//!      # struct MyParsedDataFamily; // empty type, no lifetime.
//!      # impl<'a> nolife::Family<'a> for MyParsedDataFamily {
//!      #     type Family = MyParsedData<'a>; // Indicates how the type is tied to the trait's lifetime.
//!      #     // you generally want to replace all lifetimes in the struct with the one of the trait.
//!      # }
//!      # async fn my_scope(mut time_capsule: nolife::TimeCapsule<MyParsedDataFamily /* 👈 use the helper type we declared */>,
//!      #                   data_source: Vec<u8> /* 👈 all parameters that allow to build a `MyData` */)
//!      # -> nolife::Never /* 👈 will be returned from loop */ {
//!      #     let mut data = MyData(data_source);
//!      #     let mut parsed_data = MyParsedData(&mut data); // imagine that this step is costly...
//!      #     loop /* 👈 will be coerced to a `Never` */ {
//!      #         time_capsule.freeze(&mut parsed_data).await; // gives access to the parsed data to the outside.
//!      #                           /* 👆 reference to the borrowed data */
//!      #     }
//!      # }
//!      # let mut scope = nolife::BoxScope::new();
//!      # scope.open(|time_capsule| my_scope(time_capsule, vec![0, 1, 2]));
//!      scope.enter(|parsed_data| { /* do what you need with the parsed data */ });
//!      ```
//!
//! # Kinds of scopes
//!
//! This crate provides two kinds of scopes, at the moment, with varying properties:
//!
//! |Scope|Allocations|Moveable after opening|Thread-safe|
//! |-----|-----------|----------------------|-----------|
//! |[`BoxScope`]|1 (size of the contained Future + 1 pointer to the reference type)|Yes|No|
//! |[`StackScope`]|0|No|No|
//!
//! An `RcScope` and a `MutexScope` could
//!
//! # Inner async support
//!
//! At the moment, although the functions passed to [`BoxScope::open`] are asynchronous, they should not `await` futures
//! other than the [`FrozenFuture`]. Attempting to do so **will result in a panic** if the future does not resolve immediately.
//!
//! Future versions of this crate could provide async version of [`BoxScope::enter`] to handle the asynchronous use case.
//!
//! [`yoke`]: https://crates.io/crates/yoke

mod box_scope;
pub mod counterexamples;
mod stack_scope;
/// From <https://blog.aloni.org/posts/a-stack-less-rust-coroutine-100-loc/>, originally from
/// [genawaiter](https://lib.rs/crates/genawaiter).
mod waker;

pub use box_scope::BoxScope;
pub use stack_scope::StackScope;

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
/// functions passed to [`BoxScope::open`] and [`StackScope::open`] never return.
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
///
/// Used as a parameter to [`StackScope::new_unchecked`].
pub struct Scope<T, F>
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

    #[allow(unused_unsafe)]
    unsafe fn open<P>(this: std::ptr::NonNull<Self>, producer: P)
    where
        P: FnOnce(TimeCapsule<T>) -> F,
    {
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

    #[allow(unused_unsafe)]
    unsafe fn enter<'borrow, 'scope, Output: 'borrow, G>(
        this: std::ptr::NonNull<Self>,
        f: G,
    ) -> Output
    where
        'scope: 'borrow,
        G: FnOnce(&'borrow mut <T as Family<'scope>>::Family) -> Output + 'borrow,
    {
        // SAFETY: FIXME
        let this = unsafe { this.as_ref() };

        let mut fut = this.active_fut.borrow_mut();
        let fut = fut.as_mut().unwrap().deref_mut();
        // SAFETY: self.active_fut is never moved by self after the first call to produce completes.
        //         self itself is pinned.
        let fut = unsafe { Pin::new_unchecked(fut) };
        match fut.poll(&mut std::task::Context::from_waker(&waker::create())) {
            Poll::Ready(_) => unreachable!(),
            Poll::Pending => {}
        }
        let state = this.state.0.get();
        // SAFETY: papering over the lifetime requirements here!!!
        let state: *mut <T as Family>::Family = state.cast();
        let output;
        {
            // SAFETY: NULL or set by
            // FIXME if f panics, set back to NULL
            // PANICS: future did not fill the value
            let state = unsafe { state.as_mut().unwrap() };
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
    /// Once a scope is frozen, its borrowed data can be accessed through [`BoxScope::enter`] and [`StackScope::enter`]
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
        // FIXME: Safety
        let state = unsafe { self.state.as_ref().unwrap() };
        if state.0.get().is_null() {
            // FIXME: poll called several times on the same future
            let mut_ref = self.mut_ref.take().unwrap();
            let mut_ref: *mut <T as Family>::Family = mut_ref;
            // FIXME: SAFETY!!!
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
pub struct SingleFamily<T: 'static>(PhantomData<T>);
impl<'a, T: 'static> Family<'a> for SingleFamily<T> {
    type Family = T;
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn produce_output() {
        open_stack_scope!(
            scope = |mut time_capsule: TimeCapsule<SingleFamily<u32>>| async move {
                let mut x = 0u32;
                loop {
                    time_capsule.freeze(&mut x).await;
                    x += 1;
                }
            }
        );

        assert_eq!(scope.enter(|x| *x + 42), 42);
        assert_eq!(scope.enter(|x| *x + 42), 43);
        scope.enter(|x| *x += 100);
        assert_eq!(scope.enter(|x| *x + 42), 145);
    }

    #[test]
    fn hold_reference() {
        open_stack_scope! { scope = |mut time_capsule: TimeCapsule<SingleFamily<u32>>| async move {
            let mut x = 0u32;
            loop {
                time_capsule.freeze(&mut x).await;
                x += 1;
            }
        } }

        let x = scope.enter(|x| x);
        *x = 0;

        scope.enter(|x| *x += 1);
        scope.enter(|x| assert_eq!(*x, 3))
    }
}
