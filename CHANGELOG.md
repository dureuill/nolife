# Changelog

## v0.4.0

- Breaking change:
  - ⚠️ The `FrozenFuture` no longer return `Poll::Ready`, alleviating the need for a `loop` around the `freeze` call.
    ⚠️ Code using a loop **will keep compiling** ⚠️, but any statement after the first `freeze` **is now unreachable**.
  ```rust
    async fn my_scope(mut time_capsule: nolife::TimeCapsule<MyParsedDataFamily, data_source: Vec<u8>)
    -> nolife::Never {
       let mut data = MyData(data_source);
       let mut parsed_data = MyParsedData(&mut data);
       loop {
          time_capsule.freeze(&mut parsed_data).await; // 👈 ⚠️ No longer returns
          // ⚠️ Now unreachble
          some_operation(parsed_data) // 👈 ⚠️ Never executed
       }
    }
  ```
  This allows to skip the `loop` and return directly the result of awaiting `freeze`.

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
