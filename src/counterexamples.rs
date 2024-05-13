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
//!         let mut scope = BoxScope::<CovariantFamily, _>::new(scope!({
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
//! ```compile_fail,E0597
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
//!         let mut scope = BoxScope::<CovariantFamily>::new_dyn(scope!({
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
//!         let mut scope = BoxScope::<CovariantFamily, _>::new(scope!({
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
//!         let mut scope = BoxScope::<CovariantFamily, _>::new(scope!({
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
//!         let mut scope = BoxScope::<CovariantDropFamily, _>::new(scope!({
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
//!         let mut scope = nolife::BoxScope::<ContravariantFamily, _>::new(nolife::scope!({
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
//!         let mut scope = BoxScope::<CovariantFamily>::new_dyn(scope!({
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
//! # Signature of `enter` is not sound, #7 <https://github.com/dureuill/nolife/issues/7>
//!
//! Example lifted as-is from [@steffahn](https://github.com/steffahn)
//!
//! ```compile_fail,E0597,E0499
//! use nolife::{BoxScope, Family, TimeCapsule, scope};
//!
//! struct Foo<'a> {
//!     s: String,
//!     r: Option<&'a mut String>,
//! }
//!
//! struct FooFamily;
//!
//! impl<'a> Family<'a> for FooFamily {
//!    type Family = Foo<'a>;
//! }
//!
//! fn storing_own_reference() {
//!     {
//!         let mut scope: BoxScope<FooFamily, _> = BoxScope::new(scope!({
//!             let mut f = Foo {
//!                 s: String::from("Hello World!"),
//!                 r: None,
//!             };
//!             freeze_forever!(&mut f)
//!         }));
//!
//!         scope.enter(|foo| {
//!             foo.r = Some(&mut foo.s);
//!         });
//!         scope.enter(|foo| {
//!             let alias1: &mut String = &mut foo.s;
//!             let alias2: &mut String = foo.r.as_deref_mut().unwrap(); // miri will complain here already
//!             // two aliasing mutable references!!
//!
//!             let s: &str = alias1;
//!             let owner: String = std::mem::take(alias2);
//!
//!             println!("Now it exists: {s}");
//!             drop(owner);
//!             println!("Now it's gone: {s}");
//!         })
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
//! ```compile_fail,E0505
//! use nolife::{scope, BoxScope, SingleFamily, TopScope};
//!
//! fn ref_scope() {
//!     fn scope_with_ref<'scope, 'a: 'scope>(
//!         s: &'a str,
//!     ) -> impl TopScope<Family = SingleFamily<usize>> + 'scope {
//!         scope!({ freeze_forever!(&mut s.len()) })
//!     }
//!     let x = "Intel the Beagle".to_string();
//!     let mut scope = BoxScope::<SingleFamily<usize>, _>::new(scope_with_ref(&x));
//!
//!     drop(x);
//!
//!     scope.enter(|x| assert_eq!(*x, 16));
//! }
//! ```
//!
//! # Dropping a borrowed input to a scope, erased version
//!
//! ```compile_fail,E0597,E0505
//! use nolife::{scope, BoxScope, SingleFamily, TopScope};
//!
//! fn ref_scope() {
//!     fn scope_with_ref<'scope, 'a: 'scope>(
//!         s: &'a str,
//!     ) -> impl TopScope<Family = SingleFamily<usize>, Future = impl Send + Sync + 'a> + 'scope {
//!         scope!({ freeze_forever!(&mut s.len()) })
//!     }
//!     let x = "Intel the Beagle".to_string();
//!     let mut scope = BoxScope::<SingleFamily<usize>, _>::new_dyn(scope_with_ref(&x));
//!
//!     drop(x);
//!
//!     scope.enter(|x| assert_eq!(*x, 16));
//! }
//! ```
//!
//! # Trying to Send with a non-Send Future
//!
//! ```compile_fail
//! let mut scope = nolife::BoxScope::<nolife::SingleFamily<u32>, _>::new(nolife::scope!({
//!     let rc = std::rc::Rc::new(42);
//!     let mut x = 0u32;
//!     loop {
//!         freeze!(&mut x);
//!
//!         x += 1;
//!     }
//! }));
//!
//! std::thread::scope(|t_scope| {
//!     t_scope.spawn(|| {
//!         assert_eq!(scope.enter(|x| *x + 42), 42);
//!         assert_eq!(scope.enter(|x| *x + 42), 43);
//!         scope.enter(|x| *x += 100);
//!         assert_eq!(scope.enter(|x| *x + 42), 145);
//!     });
//! })
//! ```
//!
//! # Trying to Send with a non-send Family
//!
//! ```compile_fail,E0277
//! let rc = std::rc::Rc::new(42);
//! let rc_clone = rc.clone();
//! let mut scope = nolife::BoxScope::<nolife::SingleFamily<std::rc::Rc<u32>>, _>::new(nolife::scope!({
//!     freeze_forever!(&mut rc_clone)
//! }));
//!
//! std::thread::scope(|t_scope| {
//!     t_scope.spawn(|| {
//!         scope.enter(|_| {});
//!     });
//! })
//! ```
//!
//! # Trying to send the time capsule or frozenfuture
//!
//! ```compile_fail,E0728
//! let mut scope = nolife::BoxScope::<nolife::SingleFamily<u32>, _>::new(nolife::scope!({
//!     let rc = std::rc::Rc::new(42);
//!     let mut x = 0u32;
//!     loop {
//!         std::thread::scope(|t_scope| {
//!             t_scope.spawn(|| {
//!                 freeze!(&mut x);
//!             });
//!         });
//!         x += 1;
//!     }
//! }));
//!
//! assert_eq!(scope.enter(|x| *x + 42), 42);
//! assert_eq!(scope.enter(|x| *x + 42), 43);
//! scope.enter(|x| *x += 100);
//! assert_eq!(scope.enter(|x| *x + 42), 145);
//! ```
//!
//! # Trying to sync with a non-sync family
//!
//! ```compile_fail,E0277
//! let rc = std::rc::Rc::new(42);
//! let rc_clone = rc.clone();
//! let scope = nolife::BoxScope::<nolife::SingleFamily<std::rc::Rc<u32>>, _>::new(nolife::scope!({
//!     freeze_forever!(&mut rc_clone)
//! }));
//!
//!
//! let scope_ref = &scope;
//!
//! std::thread::scope(|t_scope| {
//!     t_scope.spawn(|| scope_ref);
//! })
//! ```
//!
//! # Trying to sync with a non-sync future
//!
//! ```compile_fail
//! let scope = nolife::BoxScope::<nolife::SingleFamily<u32>, _>::new(nolife::scope!({
//!     let rc = std::rc::Rc::new(42);
//!     let mut x = 0u32;
//!     loop {
//!         freeze!(&mut x);
//!
//!         x += 1;
//!     }
//! }));
//!
//! let scope_ref = &scope;
//! std::thread::scope(|t_scope| {
//!     t_scope.spawn(|| scope_ref);
//! })
//! ```
