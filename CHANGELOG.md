# Changelog

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
