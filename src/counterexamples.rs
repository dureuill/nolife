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
//!         let scope = BoxScope::new();
//!         let mut scope = scope.open(
//!             |mut time_capsule: TimeCapsule<CovariantFamily>| async move {
//!                 let mut f = Covariant { x: "bbb" };
//!                 loop {
//!                     time_capsule.freeze(&mut f).await;
//!                     println!("Called {}", f.x)
//!                 }
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
//!         let scope = BoxScope::new();
//!         let mut scope = scope.open(
//!             |mut time_capsule: TimeCapsule<CovariantFamily>| async move {
//!                 let mut f = Covariant { x: "bbb" };
//!                 loop {
//!                     time_capsule.freeze(&mut f).await;
//!                     println!("Called {}", f.x)
//!                 }
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
//!                 loop {
//!                     time_capsule.freeze(&mut f).await;
//!                     println!("Called {}", f.x)
//!                 }
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
//! ```compile_fail,E0597
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
//!                 loop {
//!                     time_capsule.freeze(&mut f).await;
//!                     println!("Called {}", f.x)
//!                 }
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
//!                 loop {
//!                     println!("Called {}", f.x);
//!                     time_capsule.freeze(&mut f).await;
//!                 }
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
//!         let mut scope = BoxScope::new(
//!             |mut time_capsule: nolife::TimeCapsule<ContravariantFamily>| async move {
//!                 loop {
//!                     let mut x = String::from("inner");
//!
//!                     let mut f = Contravariant {
//!                         f: Box::new(|_| {}),
//!                     };
//!                     time_capsule.freeze(&mut f).await;
//!                     (f.f)(&mut x);
//!                 }
//!             },
//!         );
//!
//!         scope.enter(|f| {
//!             f.f = Box::new(|inner| outer.set(inner));
//!         });
//!     }
//!     println!("{}", outer.get());
//! }
//! ```
//!
//! ## Holding a reference
//!
//! ```compile_fail,E0597
//! use nolife::{Family, BoxScope, TimeCapsule, SingleFamily};
//!
//! fn hold_reference() {
//!     let mut scope = BoxScope::new(
//!         |mut time_capsule: TimeCapsule<SingleFamily<u32>>| async move {
//!             let mut x = 0u32;
//!             loop {
//!                 time_capsule.freeze(&mut x).await;
//!                 x += 1;
//!             }
//!         },
//!     );
//!     let x = scope.enter(|x| x);
//!     *x = 0;
//!     scope.enter(|x| *x += 1);
//!     scope.enter(|x| assert_eq!(*x, 3))
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
//!                 loop {
//!                     time_capsule.freeze(&mut f).await;
//!                     println!("Called {}", f.x)
//!                 }
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
