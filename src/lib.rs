#![warn(rustdoc::broken_intra_doc_links)]
#![warn(missing_docs)]
#![doc = include_str!("../README.md")]

mod box_scope;
pub mod counterexamples;
mod scope;
pub use scope::{FrozenFuture, TimeCapsule};
/// From <https://blog.aloni.org/posts/a-stack-less-rust-coroutine-100-loc/>, originally from
/// [genawaiter](https://lib.rs/crates/genawaiter).
mod waker;

pub use box_scope::BoxScope;

/// Convenient type alias for a [`BoxScope`] whose future is an erased boxed future.
pub type DynBoxScope<T> = BoxScope<T, std::pin::Pin<Box<dyn std::future::Future<Output = Never>>>>;

use std::marker::PhantomData;

/// A type for functions that never return.
///
/// Since this enum has no variant, a value of this type can never actually exist.
/// This type is similar to [`std::convert::Infallible`] and used as a technicality to ensure that
/// functions passed to [`BoxScope::new`] never return.
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
        let mut scope = BoxScope::new(
            |mut time_capsule: TimeCapsule<SingleFamily<u32>>| async move {
                let mut x = 0u32;
                time_capsule.freeze(&mut x).await
            },
        );

        assert_eq!(scope.enter(|x| *x + 42), 42);
        assert_eq!(scope.enter(|x| *x + 42), 42);
        scope.enter(|x| *x += 100);
        assert_eq!(scope.enter(|x| *x + 42), 142);
    }

    #[test]
    fn panicking_future() {
        let mut scope = BoxScope::new(|_: TimeCapsule<SingleFamily<u32>>| async move { panic!() });

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
        let mut scope = BoxScope::new(
            |mut time_capsule: TimeCapsule<SingleFamily<u32>>| async move {
                let mut x = 0u32;
                time_capsule.freeze(&mut x).await;
                panic!()
            },
        );

        scope.enter(|x| println!("{x}"));
        scope.enter(|x| println!("{x}"));
        scope.enter(|x| println!("{x}"));
    }

    #[test]
    fn panicking_enter() {
        let mut scope = BoxScope::new(
            |mut time_capsule: TimeCapsule<SingleFamily<u32>>| async move {
                let mut x = 0u32;
                time_capsule.freeze(&mut x).await
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
        scope.enter(|x| assert_eq!(*x, 0));
    }

    #[test]
    fn cursed_time_capsule_inception() {
        struct TimeCapsuleFamily;
        impl<'a> Family<'a> for TimeCapsuleFamily {
            // Yo dawg I heard you like time capsules, so I put time capsules in your time capsules
            type Family = TimeCapsule<TimeCapsuleFamily>;
        }

        // we'll use this to check we panicked at the correct location, RTTI-style
        struct ReachedTheEnd;

        let mut outer_scope = BoxScope::new(
            |mut time_capsule: TimeCapsule<TimeCapsuleFamily>| async move {
                let mut inner_scope = BoxScope::new(
                    |mut inner_time_capsule: TimeCapsule<TimeCapsuleFamily>| async move {
                            // very cursed
                            time_capsule.freeze(&mut inner_time_capsule).await
                    },
                );

                // we're expecting a panic here; let's catch it and check we're still safe
                assert!(matches!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        inner_scope.enter(|_exchanged_time_capsule| {});
                    })),
                    Err(_)
                ));

                // we can try again
                assert!(matches!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        inner_scope.enter(|_exchanged_time_capsule| {});
                    })),
                    Err(_)
                ));

                // we can't loop here because we relinquished our time capsule to the lambda
                std::panic::panic_any(ReachedTheEnd)
            },
        );

        // will panic with the panic at the end
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            outer_scope.enter(|_time_capsule| {});
        })) {
            Ok(_) => panic!("did not panic as expected"),
            Err(panic) => panic
                .downcast::<ReachedTheEnd>()
                .expect("panicked at the wrong location"),
        };
    }
}
