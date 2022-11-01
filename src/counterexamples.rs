//!
//!
//! # Non-compiling examples
//!
//! The following examples will not compile, preventing unsoundness.
//!
//! ## Covariant escape to inner
//!
//! ```compile_fail
//! use nolife::{Family, Scope, StackScope, TimeCapsule};
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
//!         let mut scope = Scope::new();
//!         let mut scope = unsafe { StackScope::new_unchecked(&mut scope) };
//!         scope.open(
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
//! ```compile_fail
//! use std::cell::Cell;
//! use nolife::{Family, Scope, StackScope, TimeCapsule};
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
//!         let mut scope = Scope::new();
//!         let mut scope = unsafe { StackScope::new_unchecked(&mut scope) };
//!         scope.open(
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
//! ## Covariant escapes to inner, boxed
//!
//! ```compile_fail
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
//!         let mut scope = BoxScope::new();
//!         scope.open(
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
//! ## Covariant escapes to outer, boxed
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
//!         let mut scope = BoxScope::new();
//!         scope.open(
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
//! ```compile_fail
//! use nolife::{Family, Scope, StackScope, TimeCapsule};
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
//!         let mut scope = Scope::new();
//!
//!         let mut scope = unsafe { StackScope::new_unchecked(&mut scope) };
//!         let outer = String::from("outer");
//!
//!         scope.open(
//!             |mut time_capsule: TimeCapsule<CovariantDropFamily>| async move {
//!                 let mut f = CovariantDrop { x: "inner" };
//!                 loop {
//!                     println!("Called {}", f.x);
//!                     time_capsule.freeze(&mut f).await;
//!                 }
//!             },
//!         );
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
//! ```compile_fail
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
//!     let mut scope = nolife::Scope::new();
//!     let mut scope = unsafe { nolife::StackScope::new_unchecked(&mut scope) };
//!
//!     {
//!         scope.open(
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
