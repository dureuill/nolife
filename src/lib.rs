#![warn(rustdoc::broken_intra_doc_links)]
#![warn(missing_docs)]
#![doc = include_str!("../README.md")]
#![doc(
    html_favicon_url = "https://raw.githubusercontent.com/dureuill/nolife/main/assets/nolife-tr.png?raw=true"
)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/dureuill/nolife/main/assets/nolife-tr.png?raw=true"
)]

mod box_scope;
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
        let mut scope = BoxScope::new::<SingleFamily<u32>>(scope!({
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
    fn panicking_future() {
        let mut scope = BoxScope::new::<SingleFamily<u32>>(scope!({ panic!() }));

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
        let mut scope = BoxScope::new::<SingleFamily<u32>>(scope!({
            let mut x = 0u32;
            freeze!(&mut x);
            panic!()
        }));

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
        let mut scope = BoxScope::new::<SingleFamily<u32>>(scope!({
            let mut x = 0u32;
            loop {
                freeze!(&mut x);
                x += 1;
            }
        }));

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

    // TODO: add cursed swapped scopes test
}
