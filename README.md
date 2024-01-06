<p align="center"><img width="64px" title="The nolife logo is a Rust lifetime crossed with a red cross" src="https://raw.githubusercontent.com/dureuill/nolife/add_assets/assets/nolife-tr.png"/></p>

Open a scope and then freeze it in time for future access.

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache%202%20-green)](#License)
[![Crates.io](https://img.shields.io/crates/v/nolife)](https://crates.io/crates/nolife)
[![Docs](https://docs.rs/nolife/badge.svg)](https://docs.rs/nolife)
[![dependency status](https://deps.rs/repo/github/dureuill/nolife/status.svg)](https://deps.rs/repo/github/dureuill/nolife)
[![Build](https://github.com/dureuill/nolife/actions/workflows/rust.yml/badge.svg)](https://github.com/dureuill/nolife/actions/workflows/rust.yml)

This crate allows constructing structs that contain references and keeping them alive alongside the data they reference,
without a lifetime.

This is especially useful for zero-copy parsers that construct elaborate (and possibly costly) representations that borrow
the source data.

This crate achieves that by leveraging `async` functions. At their core, `async` functions are self-referential structs. this crate simply provides a way to ex-filtrate references outside of the async function, in a controlled manner.

# Using this crate

After you identified the data and its borrowed representation that you'd like to access without a lifetime, using this crate will typically encompass a few steps:

```rust
// Given the following types:
struct MyData(Vec<u8>);
struct MyParsedData<'a>(&'a mut MyData, /* ... */);

// 1. Define a helper type that will express where the lifetimes of the borrowed representation live.
struct MyParsedDataFamily; // empty type, no lifetime.
impl<'a> nolife::Family<'a> for MyParsedDataFamily {
    type Family = MyParsedData<'a>; // Indicates how the type is tied to the trait's lifetime.
    // you generally want to replace all lifetimes in the struct with the one of the trait.
}

// 2. Define an async function that setups the data and its borrowed representation:
async fn my_scope(mut time_capsule: nolife::TimeCapsule<MyParsedDataFamily /* 👈 use the helper type we declared */>,
                  data_source: Vec<u8> /* 👈 all parameters that allow to build a `MyData` */)
-> nolife::Never /* 👈 will be returned from loop */ {
   let mut data = MyData(data_source);
   let mut parsed_data = MyParsedData(&mut data); // imagine that this step is costly...
   time_capsule.freeze_forever(&mut parsed_data).await // gives access to the parsed data to the outside.
                             /* 👆 reference to the borrowed data */
}

// 3. Open a `BoxScope` using the previously written async function:
let mut scope = nolife::DynBoxScope::pin(|time_capsule| my_scope(time_capsule, vec![0, 1, 2]));

// 4. Store the `BoxScope` anywhere you want
struct ContainsScope {
    scope: nolife::DynBoxScope<MyParsedDataFamily>
    /* other data */
}

// 5. Lastly, enter the scope to retrieve access to the referenced value.
scope.enter(|parsed_data| { /* do what you need with the parsed data */ });
```

# Kinds of scopes

This crate only provide a single kind of scope at the moment

|Scope|Allocations|Moveable after opening|Thread-safe|
|-----|-----------|----------------------|-----------|
|[`BoxScope`]|1 (size of the contained Future + 1 pointer to the reference type)|Yes|No|

An `RcScope` or `MutexScope` could be future extensions

# Inner async support

At the moment, although the functions passed to [`BoxScope::new`] are asynchronous, they should not `await` futures other than the [`FrozenFuture`]. Attempting to do so **will result in a panic** if the future does not resolve immediately.

Future versions of this crate could provide async version of [`BoxScope::enter`] to handle the asynchronous use case.

# License

Licensed under either of [Apache License](./LICENSE-APACHE), Version 2.0 or [MIT license](./LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this project by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

# Alternative

[`yoke`] serves a similar use case as this crate, albeit it is expressed in terms of a self-referential struct rather than as an async scope, which is less natural if the intent is to borrow some data.

[`yoke`]: https://crates.io/crates/yoke
