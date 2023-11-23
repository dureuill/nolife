Open a scope and then freeze it in time for future access.

This crate allows constructing structs that contain references and keeping them alive alongside the data they reference,
without a lifetime.

This is especially useful for zero-copy parsers that construct elaborate (and possibly costly) representations that borrow
the source data.

In this regard, this crate has similar use cases as [`yoke`].

Unlike [`yoke`], this crate achieves that by leveraging `async` functions. At their core, `async` functions are self-referential structs. this crate simply provides a way to ex-filtrate references outside of the async function, in a controlled manner.


# Soundness note

While the code of this crate was reviewed carefully and tested using `miri`, this crate is using **extremely unsafe** code that is *easy to get wrong*.

To emphasize what is already written in the license, **use at your own risk**.

# Using this crate

After you identified the data and its borrowed representation that you'd like to access without a lifetime, using this crate will typically encompass a few steps:

1. Define a helper type that will express where the lifetimes of the borrowed representation live.
   Given the following types:

    ```rust
    struct MyData(Vec<u8>);
    struct MyParsedData<'a>(&'a mut MyData, /* ... */);
    ```

   we want to define an helper type that will implement the [`Family`] trait and tie its lifetime to `MyParsedData`'s lifetime.

    ```rust
    # struct MyData(Vec<u8>);
    # struct MyParsedData<'a>(&'a mut MyData, /* ... */);
    struct MyParsedDataFamily; // empty type, no lifetime.
    impl<'a> nolife::Family<'a> for MyParsedDataFamily {
        type Family = MyParsedData<'a>; // Indicates how the type is tied to the trait's lifetime.
        // you generally want to replace all lifetimes in the struct with the one of the trait.
    }
    ```

    2. Define an async function that setups the data and its borrowed representation:

     ```rust
     # struct MyData(Vec<u8>);
     # struct MyParsedData<'a>(&'a mut MyData, /* ... */);
     # struct MyParsedDataFamily; // empty type, no lifetime.
     # impl<'a> nolife::Family<'a> for MyParsedDataFamily {
     #     type Family = MyParsedData<'a>; // Indicates how the type is tied to the trait's lifetime.
     #     // you generally want to replace all lifetimes in the struct with the one of the trait.
     # }
     async fn my_scope(mut time_capsule: nolife::TimeCapsule<MyParsedDataFamily /* 👈 use the helper type we declared */>,
                       data_source: Vec<u8> /* 👈 all parameters that allow to build a `MyData` */)
     -> nolife::Never /* 👈 will be returned from loop */ {
         let mut data = MyData(data_source);
         let mut parsed_data = MyParsedData(&mut data); // imagine that this step is costly...
         loop /* 👈 will be coerced to a `Never` */ {
             time_capsule.freeze(&mut parsed_data).await; // gives access to the parsed data to the outside.
                               /* 👆 reference to the borrowed data */
         }
     }
     ```

     3. Open a [box](`BoxScope`),
     using the previously written async function:

     ```rust
     # struct MyData(Vec<u8>);
     # struct MyParsedData<'a>(&'a mut MyData, /* ... */);
     # struct MyParsedDataFamily; // empty type, no lifetime.
     # impl<'a> nolife::Family<'a> for MyParsedDataFamily {
     #     type Family = MyParsedData<'a>; // Indicates how the type is tied to the trait's lifetime.
     #     // you generally want to replace all lifetimes in the struct with the one of the trait.
     # }
     # async fn my_scope(mut time_capsule: nolife::TimeCapsule<MyParsedDataFamily /* 👈 use the helper type we declared */>,
     #                   data_source: Vec<u8> /* 👈 all parameters that allow to build a `MyData` */)
     # -> nolife::Never /* 👈 will be returned from loop */ {
     #     let mut data = MyData(data_source);
     #     let mut parsed_data = MyParsedData(&mut data); // imagine that this step is costly...
     #     loop /* 👈 will be coerced to a `Never` */ {
     #         time_capsule.freeze(&mut parsed_data).await; // gives access to the parsed data to the outside.
     #                           /* 👆 reference to the borrowed data */
     #     }
     # }
     let mut scope = nolife::BoxScope::new(|time_capsule| my_scope(time_capsule, vec![0, 1, 2]));
     // You can now store the open scope anywhere you want.
     ```

     4. Lastly, enter the scope to retrieve access to the referenced value.
     ```rust
     # struct MyData(Vec<u8>);
     # struct MyParsedData<'a>(&'a mut MyData, /* ... */);
     # struct MyParsedDataFamily; // empty type, no lifetime.
     # impl<'a> nolife::Family<'a> for MyParsedDataFamily {
     #     type Family = MyParsedData<'a>; // Indicates how the type is tied to the trait's lifetime.
     #     // you generally want to replace all lifetimes in the struct with the one of the trait.
     # }
     # async fn my_scope(mut time_capsule: nolife::TimeCapsule<MyParsedDataFamily /* 👈 use the helper type we declared */>,
     #                   data_source: Vec<u8> /* 👈 all parameters that allow to build a `MyData` */)
     # -> nolife::Never /* 👈 will be returned from loop */ {
     #     let mut data = MyData(data_source);
     #     let mut parsed_data = MyParsedData(&mut data); // imagine that this step is costly...
     #     loop /* 👈 will be coerced to a `Never` */ {
     #         time_capsule.freeze(&mut parsed_data).await; // gives access to the parsed data to the outside.
     #                           /* 👆 reference to the borrowed data */
     #     }
     # }
     # let mut scope = nolife::BoxScope::new(|time_capsule| my_scope(time_capsule, vec![0, 1, 2]));
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

[`yoke`]: https://crates.io/crates/yoke
