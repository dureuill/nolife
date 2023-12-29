//!
//!
//! # Non-compiling examples
//!
//! The following examples will not compile, preventing unsoundness.
//!
//! ## Covariant escapes to inner
//!
//! ```compile_fail,E0597
//! use nolife::{Family, BoxScope, TimeCapsule};
//!
//! struct Covariant<'a> {
//!     x: &'a str,
//! }
//!
//! struct CovariantFamily;
//!
//! impl<'a> Family<'a> for CovariantFamily {
//!     type Family = Covariant<'a>;
//! }
//!
//! fn covariant_inner() {
//!     {
//!         let mut scope = BoxScope::new(
//!             |mut time_capsule: TimeCapsule<CovariantFamily>| async move {
//!                 let mut f = Covariant { x: "bbb" };
//!                 time_capsule.freeze(&mut f).await
//!             },
//!         );
//!
//!         {
//!             let s = String::from("foodog");
//!             scope.enter(|f| {
//!                 f.x = &s;
//!             });
//!         }
//!     }
//! }
//! ```
//!
//! ## Covariant escapes to outer
//!
//! ```compile_fail,E0521
//! use std::cell::Cell;
//! use nolife::{Family, BoxScope, TimeCapsule};
//!
//! struct Covariant<'a> {
//!     x: &'a str,
//! }
//!
//! struct CovariantFamily;
//!
//! impl<'a> Family<'a> for CovariantFamily {
//!     type Family = Covariant<'a>;
//! }
//!
//! fn covariant_outer() {
//!     let output = Cell::new("foo");
//!     {
//!         let mut scope = BoxScope::new(
//!             |mut time_capsule: TimeCapsule<CovariantFamily>| async move {
//!                 let mut f = Covariant { x: "bbb" };
//!                 time_capsule.freeze(&mut f).await
//!             },
//!         );
//!
//!         {
//!             scope.enter(|f| {
//!                 output.set(f.x);
//!             });
//!         }
//!     }
//!     println!("{}", output.get());
//! }
//! ```
//!
//! ## Covariant escapes to inner 2
//!
//! ```compile_fail,E0597
//! use nolife::{BoxScope, Family, TimeCapsule};
//!
//!
//! struct Covariant<'a> {
//!     x: &'a str,
//! }
//!
//! struct CovariantFamily;
//!
//! impl<'a> Family<'a> for CovariantFamily {
//!     type Family = Covariant<'a>;
//! }
//!
//! fn box_covariant_inner() {
//!     {
//!         let mut scope = BoxScope::new(
//!             |mut time_capsule: TimeCapsule<CovariantFamily>| async move {
//!                 let x = String::from("aaaaa");
//!                 let mut f = Covariant { x: &x };
//!                 time_capsule.freeze(&mut f).await
//!             },
//!         );
//!
//!         {
//!             let s = String::from("dangling");
//!             scope.enter(|f| f.x = &s);
//!         }
//!     };
//! }

//! ```
//!
//! ## Covariant escapes to outer 2
//!
//! ```compile_fail
//! use nolife::{BoxScope, Family, TimeCapsule};
//! use std::cell::Cell;
//!
//! struct Covariant<'a> {
//!     x: &'a str,
//! }
//!
//! struct CovariantFamily;
//!
//! impl<'a> Family<'a> for CovariantFamily {
//!     type Family = Covariant<'a>;
//! }
//!
//! fn box_covariant_outer() {
//!     let outer = Cell::new("foo");
//!     {
//!         let mut scope = BoxScope::new(
//!             |mut time_capsule: TimeCapsule<CovariantFamily>| async move {
//!                 let x = String::from("aaaaa");
//!                 let mut f = Covariant { x: &x };
//!                 time_capsule.freeze(&mut f).await
//!             },
//!         );
//!
//!         let inner = scope.enter(|f| f.x);
//!         outer.set(inner);
//!     };
//!     println!("{}", outer.get());
//! }
//! ```
//!
//! ## Covariant with `Drop`
//!
//! ```compile_fail,E0597
//! use nolife::{Family, BoxScope, TimeCapsule};
//!
//! struct CovariantDrop<'a> {
//!     x: &'a str,
//! }
//!
//! impl<'a> Drop for CovariantDrop<'a> {
//!     fn drop(&mut self) {
//!         println!("Dropping {}", self.x)
//!     }
//! }
//! struct CovariantDropFamily;
//!
//! impl<'a> Family<'a> for CovariantDropFamily {
//!     type Family = CovariantDrop<'a>;
//! }
//!
//! fn covariant_drop() {
//!     {
//!         let mut scope = BoxScope::new(
//!             |mut time_capsule: TimeCapsule<CovariantDropFamily>| async move {
//!                 let mut f = CovariantDrop { x: "inner" };
//!                 println!("Called {}", f.x);
//!                 time_capsule.freeze(&mut f).await
//!             },
//!         );
//!
//!         let outer = String::from("outer");
//!
//!
//!         {
//!             scope.enter(|f| {
//!                 f.x = &outer;
//!             });
//!         }
//!     }
//! }
//! ```
//!
//! ## Contravariant example
//!
//! ```compile_fail,E0597
//! use std::cell::Cell;
//!
//! struct Contravariant<'a> {
//!     f: Box<dyn FnMut(&'a mut str) + 'a>,
//!     x: &'a mut str,
//! }
//!
//! struct ContravariantFamily;
//!
//! impl<'a> nolife::Family<'a> for ContravariantFamily {
//!     type Family = Contravariant<'a>;
//! }
//!
//! fn contravariant() {
//!     let outer: Cell<&str> = Cell::new("toto");
//!
//!     {
//!         let mut scope = nolife::BoxScope::new(
//!             |mut time_capsule: nolife::TimeCapsule<ContravariantFamily>| async move {
//!                     let mut x = String::from("inner");
//!
//!                     let mut f = Contravariant {
//!                         f: Box::new(|_| {}),
//!                         x: &mut x,
//!                     };
//!                     time_capsule.freeze(&mut f).await
//!             },
//!         );
//!
//!         scope.enter(|f| {
//!             f.f = Box::new(|inner| outer.set(inner));
//!         });
//!         scope.enter(|f| {
//!             (f.f)(f.x);
//!         });
//!     }
//!     println!("{}", outer.get());
//! }
//! ```
//!
//! ## Covariant coming from a previous scope
//!
//! ```compile_fail,E0597
//! use nolife::{Family, BoxScope, TimeCapsule};
//!
//! struct Covariant<'a> {
//!     x: &'a str,
//! }
//!
//! struct CovariantFamily;
//!
//! impl<'a> Family<'a> for CovariantFamily {
//!     type Family = Covariant<'a>;
//! }
//!
//!
//! fn covariant_inner() {
//!     {
//!         let mut scope = BoxScope::new(
//!             |mut time_capsule: TimeCapsule<CovariantFamily>| async move {
//!                 let mut f = Covariant { x: "bbb" };
//!                 time_capsule.freeze(&mut f).await
//!             },
//!         );
//!         {
//!             let s = String::from("foodog");
//!
//!             {
//!                 scope.enter(|f| {
//!                     f.x = &s;
//!                 });
//!             }
//!         }
//!         scope.enter(|f| ());
//!     }
//! }
//! ```
