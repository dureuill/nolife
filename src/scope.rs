//! Defines a generic `Scope` as a trait that can be instantiated as a [`crate::BoxScope`].
use std::{future::Future, marker::PhantomData};

use crate::{Family, Never, TimeCapsule};

/// Trait sealed for safety.
///
/// The trait is only implemented on [`crate::scope::Wrapper`]
pub(crate) trait Sealed {}

impl<P, Family, Future, Output> Sealed for Wrapper<P, Family, Future, Output>
where
    P: FnOnce(super::TimeCapsule<Family>) -> Future,
    Family: for<'a> crate::Family<'a>,
    Future: std::future::Future<Output = Output>,
{
}

/// A scope that can be frozen in time.
///
/// To get a `Scope`, use the [`crate::scope!`] macro.
#[allow(private_bounds)]
pub trait Scope: Sealed {
    /// The helper struct that serves to define the reference type.
    type Family: for<'a> Family<'a>;
    /// The output type of this scope.
    type Output;
    /// The underlying future that serves as a coroutine to freeze the scope.
    type Future: Future<Output = Self::Output>;

    /// Runs a scope by injecting a [`TimeCapsule`].
    ///
    /// # Safety
    ///
    /// - This function is only safe if the produced future is awaited immediately.
    ///
    /// Using the `sub_scope` macro inside a [`crate::scope!`] always verifies this condition and is therefore always safe.
    unsafe fn run(self, time_capsule: TimeCapsule<Self::Family>) -> Self::Future;
}

/// A top-level [`Scope`], always returning [`crate::Never`].
///
/// Create one using the [`crate::scope!`] macro.
pub trait TopScope: Scope<Output = Never> {}

impl<S> TopScope for S where S: Scope<Output = Never> {}

/// A wrapper for a producer.
///
/// See [`Scope`] for more information.
struct Wrapper<P, Family, Future, Output>(P, PhantomData<*const Family>)
where
    P: FnOnce(TimeCapsule<Family>) -> Future,
    Family: for<'a> crate::Family<'a>,
    Future: std::future::Future<Output = Output>;

impl<P, Family, Future, Output> Scope for Wrapper<P, Family, Future, Output>
where
    P: FnOnce(TimeCapsule<Family>) -> Future,
    Family: for<'a> crate::Family<'a>,
    Future: std::future::Future<Output = Output>,
{
    type Family = Family;
    type Output = Output;
    type Future = Future;

    unsafe fn run(self, time_capsule: TimeCapsule<Self::Family>) -> Self::Future {
        (self.0)(time_capsule)
    }
}

#[doc(hidden)]
/// Constructs a new scope from a producer
///
/// # Safety
///
/// - This function is only safe if the producer guarantees that any call to `crate::TimeCapsule::freeze` or
///   `crate::TimeCapsule::freeze_forever` happens at the top level of the producer,
///   and that the resulting future is awaited immediately.
///
/// Using the [`crate::scope!`] macro always verifies this condition and is therefere always safe.
pub unsafe fn new_scope<P, Family, Future, Output>(
    producer: P,
) -> impl Scope<Family = Family, Output = Output, Future = Future>
where
    P: FnOnce(TimeCapsule<Family>) -> Future,
    Family: for<'a> crate::Family<'a>,
    Future: std::future::Future<Output = Output>,
{
    Wrapper(producer, PhantomData)
}

/// A macro to open a scope that can be frozen in time.
///
/// You can write code like you normally would in that scope, but you get 3 additional superpowers:
///
/// 1. `freeze!(&mut x)`: interrupts execution of the scope until the next call to [`crate::BoxScope::enter`],
///   that will resume execution. The passed `&mut x` will be available to the next call to [`crate::BoxScope::enter`].
/// 2. `freeze_forever!(&mut x)`: interrupts execution of the scope forever.
///    All future calls to [`crate::BoxScope::enter`] will have access to the passed `&mut x`.
/// 3. `subscope!(some_subscope(...))`: execute an expression that can be another function returning a `scope!` itself.
///    This is meant to be able to structure your code in functions.
///
/// A `scope!` invocation returns some type that `impl Scope` or `impl TopScope` (when the scope never returns).
/// The `Family` type of the `Scope` typically needs to be annotated, whereas the `Future` and `Producer`
/// types should not be.
///
/// TODO: example
///
/// # Panics
///
/// The block passed to `scope` is technically an `async` block, but trying to `await` a future in this block
/// will always result in a panic.
#[macro_export]
macro_rules! scope {
    ($b:block) => {
        match move |#[allow(unused_variables, unused_mut)] mut time_capsule| async move {
            'check_top: {
                #[allow(unreachable_code)]
                if false {
                    break 'check_top (loop {});
                }
                /// `freeze!(&mut x)` interrupts execution of the scope, making `&mut x` available to the next call
                /// to [`nolife::BoxScope::enter`].
                ///
                /// Execution will resume after a call to [`nolife::BoxScope::enter`].
                #[allow(unused_macros)]
                macro_rules! freeze {
                    ($e:expr) => {
                        #[allow(unreachable_code)]
                        if false {
                            break 'check_top (loop {});
                        }
                        $crate::TimeCapsule::freeze(&mut time_capsule, $e).await
                    }
                }
                /// `freeze_forever!(&mut x)` stops execution of the scope forever, making `&mut x` available to all future calls
                /// to [`$crate::BoxScope::enter`].
                ///
                /// Execution will never resume.
                #[allow(unused_macros)]
                macro_rules! freeze_forever {
                    ($e:expr) => {{
                        #[allow(unreachable_code)]
                        if false {
                            break 'check_top (loop {});
                        }
                        $crate::TimeCapsule::freeze_forever(&mut time_capsule, $e).await}
                    }
                }
                /// `sub_scope(some_scope)` runs the sub-scope `some_scope` to completion before continuing execution of the current scope,
                /// yielding the output value of the sub-scope.
                ///
                /// `some_scope` is typically an expression that is itself a `scope!`.
                ///
                /// This macro is meant to allow you to structure your code in multiple functions.
                #[allow(unused_macros)]
                macro_rules! sub_scope {
                    ($e:expr) => {{
                        #[allow(unreachable_code)]
                        if false {
                            break 'check_top (loop {});
                        }
                        match $e { e => unsafe { $crate::scope::Scope::run(e, time_capsule).await } }
                    }}
                }
                $b
            }
        } { scope => unsafe { $crate::scope::new_scope(scope) } }
    };
}
