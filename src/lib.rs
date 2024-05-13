#![warn(rustdoc::broken_intra_doc_links)]
#![warn(missing_docs)]
#![deny(elided_lifetimes_in_paths)]
#![deny(unsafe_op_in_unsafe_fn)]
#![doc = include_str!("../README.md")]
#![doc(
    html_favicon_url = "https://raw.githubusercontent.com/dureuill/nolife/main/assets/nolife-tr.png?raw=true"
)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/dureuill/nolife/main/assets/nolife-tr.png?raw=true"
)]
#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

mod box_scope;
#[cfg(not(miri))]
pub mod counterexamples;
mod raw_scope;
pub mod scope;
#[doc(hidden)]
pub use raw_scope::{FrozenFuture, TimeCapsule};
/// From <https://blog.aloni.org/posts/a-stack-less-rust-coroutine-100-loc/>, originally from
/// [genawaiter](https://lib.rs/crates/genawaiter).
mod waker;

pub use box_scope::BoxScope;
pub use scope::Scope;
pub use scope::TopScope;

use core::marker::PhantomData;

/// A type for functions that never return.
///
/// Since this enum has no variant, a value of this type can never actually exist.
/// This type is similar to [`std::convert::Infallible`] and used as a technicality to ensure that
/// functions passed to [`BoxScope::new_dyn`] never return.
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
/// for any `T : 'static` you can use this family.
pub struct SingleFamily<T: 'static>(PhantomData<T>);
impl<'a, T: 'static> Family<'a> for SingleFamily<T> {
    type Family = T;
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn produce_output() {
        let mut scope = BoxScope::<SingleFamily<u32>, _>::new(scope!({
            let mut x = 0u32;
            loop {
                freeze!(&mut x);
                x += 1;
            }
        }));

        assert_eq!(scope.enter(|x| *x + 42), 42);
        assert_eq!(scope.enter(|x| *x + 42), 43);
        scope.enter(|x| *x += 100);
        assert_eq!(scope.enter(|x| *x + 42), 145);
    }

    #[test]
    fn produce_output_erased() {
        let mut scope = BoxScope::<SingleFamily<u32>>::new_dyn(scope!({
            let mut x = 0u32;
            loop {
                freeze!(&mut x);
                x += 1;
            }
        }));

        assert_eq!(scope.enter(|x| *x + 42), 42);
        assert_eq!(scope.enter(|x| *x + 42), 43);
        scope.enter(|x| *x += 100);
        assert_eq!(scope.enter(|x| *x + 42), 145);
    }

    #[cfg(feature = "std")]
    fn must_panic<F, R>(f: F)
    where
        F: FnOnce() -> R,
    {
        assert!(matches!(
            std::panic::catch_unwind(core::panic::AssertUnwindSafe(f)),
            Err(_)
        ));
    }

    #[test]
    #[cfg(feature = "std")]
    fn panicking_producer() {
        must_panic(|| {
            BoxScope::<SingleFamily<u32>, _>::new(unsafe {
                crate::scope::new_scope(|_time_capsule| {
                    panic!("panicking producer");
                    #[allow(unreachable_code)]
                    async {
                        loop {}
                    }
                })
            })
        });
    }

    #[test]
    #[cfg(feature = "std")]
    fn panicking_future() {
        let mut scope = BoxScope::<SingleFamily<u32>, _>::new(scope!({ panic!() }));

        must_panic(|| scope.enter(|x| println!("{x}")));
        must_panic(|| scope.enter(|x| println!("{x}")));
    }

    #[test]
    #[cfg(feature = "std")]
    fn panicking_future_after_once() {
        let mut scope = BoxScope::<SingleFamily<u32>, _>::new(scope!({
            let mut x = 0u32;
            freeze!(&mut x);
            panic!()
        }));

        scope.enter(|x| println!("{x}"));

        must_panic(|| scope.enter(|x| println!("{x}")));
        must_panic(|| scope.enter(|x| println!("{x}")));
    }

    #[test]
    #[cfg(feature = "std")]
    fn panicking_enter() {
        let mut scope = BoxScope::<SingleFamily<u32>, _>::new(scope!({
            let mut x = 0u32;
            loop {
                freeze!(&mut x);
                x += 1;
            }
        }));

        scope.enter(|x| assert_eq!(*x, 0));

        must_panic(|| scope.enter(|_| panic!()));

        // '1' skipped due to panic
        scope.enter(|x| assert_eq!(*x, 2));
    }

    #[test]
    fn ref_scope() {
        use alloc::string::ToString;

        fn scope_with_ref<'scope, 'a: 'scope>(
            s: &'a str,
        ) -> impl TopScope<Family = SingleFamily<usize>> + 'scope {
            scope!({ freeze_forever!(&mut s.len()) })
        }
        let x = "Intel the Beagle".to_string();
        let mut scope = BoxScope::<SingleFamily<usize>, _>::new(scope_with_ref(&x));

        scope.enter(|x| assert_eq!(*x, 16));
    }

    #[test]
    fn awaiting_in_scope_ready() {
        let mut scope = BoxScope::<SingleFamily<u32>>::new_dyn(scope!({
            freeze!(&mut 40);
            core::future::ready(()).await;
            freeze_forever!(&mut 42)
        }));

        scope.enter(|x| assert_eq!(*x, 40));
        scope.enter(|x| assert_eq!(*x, 42));
    }

    #[test]
    #[cfg(feature = "std")]
    fn awaiting_in_scope_panics() {
        let mut scope = BoxScope::<SingleFamily<u32>>::new_dyn(scope!({
            freeze!(&mut 40);
            let () = core::future::pending().await;
            freeze_forever!(&mut 42)
        }));

        scope.enter(|x| assert_eq!(*x, 40));

        must_panic(|| scope.enter(|x| assert_eq!(*x, 42)));
    }

    #[test]
    #[cfg(feature = "std")]
    fn send_in_thread() {
        let mut scope = BoxScope::<SingleFamily<u32>, _>::new(scope!({
            let mut x = 0u32;
            loop {
                freeze!(&mut x);

                x += 1;
            }
        }));

        std::thread::scope(|t_scope| {
            t_scope.spawn(|| {
                assert_eq!(scope.enter(|x| *x + 42), 42);
                assert_eq!(scope.enter(|x| *x + 42), 43);
                scope.enter(|x| *x += 100);
                assert_eq!(scope.enter(|x| *x + 42), 145);
            });
        })
    }

    #[test]
    #[cfg(feature = "std")]
    fn sync_in_thread() {
        let scope = BoxScope::<SingleFamily<u32>, _>::new(scope!({
            let mut x = 0u32;
            loop {
                freeze!(&mut x);
                x += 1;
            }
        }));

        let scope_ref = &scope;

        std::thread::scope(|t_scope| {
            t_scope.spawn(|| scope_ref);
        })
    }
}
