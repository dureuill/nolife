# Changelog

## v0.4.0

- Breaking changes:
  - `BoxScope::new` now accepts a `Scope` object that is unsafe to construct directly.
    Safe use of `nolife` now requires using the `scope!({})` macro.
    This is to fix a [soundness issue](https://github.com/dureuill/nolife/issues/8) that can occur when
    a `FrozenFuture` is polled directly, without giving back control to nolife's executor.
    The macros ensure that a frozen future is always immediately awaited.
    To update your usages, replace:
  ```rust
  let scope = BoxScope::new(|time_capsule| async { time_capsule.freeze_forever(&mut some_ref()).await });
  ```

  with:
  ```rust
  let scope = BoxScope::new(scope!({ freeze_forever!(&mut some_ref()) }));
  ```

  Check the [documentation](https://docs.rs/nolife/0.4.0/nolife/macro.scope.html) of the `scope!` macro for details.
  - The lifetime of the `Family` type passed to the closure in `BoxScope::enter` changed to fix a [soundness issue](https://github.com/dureuill/nolife/issues/7).
  - Removed `DynBoxScope`, use `BoxScope::new_dyn` instead.

- Improvement: dynamic scope no longer boxes the future, as it is already behind a level of indirection in a `BoxScope`.

Thanks to [@steffahn](https://github.com/steffahn) for opening the above issues and helping fix them ❤️

## v0.3.3

- Fix documentation links
- Add a beautifully handcrafted icon

## v0.3.2

- Add `TimeCapsule::freeze_forever` as a convenience method
- Add `DynBoxScope::pin` as a convenience method.

## v0.3.1

- Add `DynBoxScope` type for common case of erased future

## v0.3.0

- Breaking changes:
  - Tightened `BoxScope::enter` signature so that the frozen reference cannot escape,
    fixing a soundness issue. Replace:
  ```rust
  let frozen_ref = scope.enter(identity);
  use_frozen_ref(frozen_ref, refs_from_environment);
  ```

  with:
  ```rust
  scope.enter(|frozen_ref| use_frozen_ref(frozen_ref, refs_from_environment));
  ```
  Note that `BoxScope::enter` signature was adjusted to allow passing references from the environment
  to its closure argument.
  If your code specifically relied on storing the escaped reference, it cannot be ported to the new version.
  However, it was likely unsound.

  - Removed `StackScope`s, hid `Scope` and `SingleFamily`.
  - Removed closed scopes again 🤡. Replace:
  ```rust
  let scope = WhateverScope::new();
  let mut scope = scope.open(producer);
  scope.enter(consumer);
  ```

  with:
  ```rust
  let mut scope = WhateverScope::new(producer);
  scope.enter(consumer);
  ```

## v0.2.0

- Breaking change: separated closed and opened scope. Replace:

```rust
let mut scope = WhateverScope::new(...);
scope.open(...);
scope.enter(...);
```

with:

```rust
let scope = WhateverScope::new(...); // removed the mut
let mut scope = scope.open(...); // re-assign the scope
scope.enter(...); // unchanged
```

- Fix a panic that would occur when dropping an unopened scope.

## v0.1.0

- Initial version
