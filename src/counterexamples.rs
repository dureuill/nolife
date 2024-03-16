//!
//!
//! # Non-compiling examples
//!
//! The following examples will not compile, preventing unsoundness.
//!
//! ## Covariant escapes to inner
//!
//! ```compile_fail,E0597
//! use nolife::{scope, BoxScope, Family};
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
//!         let mut scope = BoxScope::<CovariantFamily, _>::new_typed(scope!({
//!             let mut f = Covariant { x: "bbb" };
//!             loop {
//!                 freeze!(&mut f);
//!                 println!("Called {}", f.x)
//!             }
//!         }));
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
//! use nolife::{scope, BoxScope, Family};
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
//! fn covariant_outer() {
//!     let output = Cell::new("foo");
//!     {
//!         let mut scope = BoxScope::<CovariantFamily>::new_erased(scope!({
//!             let mut f = Covariant { x: "bbb" };
//!             loop {
//!                 freeze!(&mut f);
//!                 println!("Called {}", f.x)
//!             }
//!         }));
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
//! use nolife::{scope, BoxScope, Family};
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
//!         let mut scope = BoxScope::<CovariantFamily, _>::new_typed(scope!({
//!             let x = String::from("aaaaa");
//!             let mut f = Covariant { x: &x };
//!             loop {
//!                 freeze!(&mut f);
//!                 println!("Called {}", f.x)
//!             }
//!         }));
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
//! use nolife::{scope, BoxScope, Family};
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
//!         let mut scope = BoxScope::<CovariantFamily, _>::new_typed(scope!({
//!             let x = String::from("aaaaa");
//!             let mut f = Covariant { x: &x };
//!             loop {
//!                 freeze!(&mut f);
//!                 println!("Called {}", f.x)
//!             }
//!         }));
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
//! use nolife::{scope, BoxScope, Family};
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
//!         let mut scope = BoxScope::<CovariantDropFamily, _>::new_typed(scope!({
//!             let mut f = CovariantDrop { x: "inner" };
//!             loop {
//!                 println!("Called {}", f.x);
//!                 freeze!(&mut f);
//!             }
//!         }));
//!
//!         let outer = String::from("outer");
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
//!         let mut scope = nolife::BoxScope::<ContravariantFamily, _>::new_typed(nolife::scope!({
//!             loop {
//!                 let mut x = String::from("inner");
//!
//!                 let mut f = Contravariant {
//!                     f: Box::new(|_| {}),
//!                 };
//!                 freeze!(&mut f);
//!                 (f.f)(&mut x);
//!             }
//!         }));
//!
//!         scope.enter(|f| {
//!             f.f = Box::new(|inner| outer.set(inner));
//!         });
//!     }
//!     println!("{}", outer.get());
//! }
//! ```
//!
//! ## Covariant coming from a previous scope
//!
//! ```compile_fail,E0597
//! use nolife::{scope, BoxScope, Family};
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
//!         let mut scope = BoxScope::<CovariantFamily>::new_erased(scope!({
//!             let mut f = Covariant { x: "bbb" };
//!             loop {
//!                 freeze!(&mut f);
//!                 println!("Called {}", f.x)
//!             }
//!         }));
//!         {
//!             let s = String::from("foodog");
//!
//!             {
//!                 scope.enter(|f| {
//!                     f.x = &s;
//!                 });
//!             }
//!         }
//!         scope.enter(|_f| ());
//!     }
//! }
//! ```
//!
//! # Recursion is not allowed
//!
//! ```compile_fail,E0733
//! use nolife::{scope, SingleFamily, TopScope};
//!
//! fn recursive_sub_scope() {
//!     fn some_scope(x: u32) -> impl TopScope<Family = SingleFamily<u32>> {
//!         scope!({
//!             if x == 0 {
//!                 freeze_forever!(&mut 0)
//!             } else {
//!                 sub_scope!(some_scope(x - 1));
//!                 freeze_forever!(&mut 0)
//!             }
//!         })
//!     }
//! }
//! ```
//!
//! # Attempting to save the frozen future in an async block
//!
//! ```compile_fail,E0767,E0267
//! use nolife::{scope, SingleFamily, TopScope};
//! fn forcing_inner_async() {
//!     fn some_scope(x: u32) -> impl TopScope<Family = SingleFamily<u32>> {
//!         scope!({
//!             let fut = async {
//!                 freeze!(&mut 0);
//!             };
//!             // poll future
//!             // bang!
//!             panic!()
//!         })
//!     }
//! }
//! ```
//!
//! # Dropping a borrowed input to a scope.
//!
//! ```compile_fail,E505
//! use nolife::{scope, BoxScope, SingleFamily, TopScope};
//!
//! fn ref_scope() {
//!     fn scope_with_ref<'scope, 'a: 'scope>(
//!         s: &'a str,
//!     ) -> impl TopScope<Family = SingleFamily<usize>> + 'scope {
//!         scope!({ freeze_forever!(&mut s.len()) })
//!     }
//!     let x = "Intel the Beagle".to_string();
//!     let mut scope = BoxScope::<SingleFamily<usize>, _>::new_typed(scope_with_ref(&x));
//!
//!     drop(x);
//!
//!     scope.enter(|x| assert_eq!(*x, 16));
//! }
//! ```
//!
//! # Dropping a borrowed input to a scope, erased version
//!
//! ```compile_fail,E597,E505
//! use nolife::{scope, BoxScope, SingleFamily, TopScope};
//!
//! fn ref_scope() {
//!     fn scope_with_ref<'scope, 'a: 'scope>(
//!         s: &'a str,
//!     ) -> impl TopScope<Family = SingleFamily<usize>> + 'scope {
//!         scope!({ freeze_forever!(&mut s.len()) })
//!     }
//!     let x = "Intel the Beagle".to_string();
//!     let mut scope = BoxScope::<SingleFamily<usize>, _>::new_erased(scope_with_ref(&x));
//!
//!     drop(x);
//!
//!     scope.enter(|x| assert_eq!(*x, 16));
//! }
//! ```
