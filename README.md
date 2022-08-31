# `find-folly`

## Overview

This crate is a simple build dependency you can use in your `build.rs` scripts to compile and
link against the [Folly C++ library](https://github.com/facebook/folly).

In theory, the [`pkg-config`](https://crates.io/crates/pkg-config) library would be all you
need in order to locate Folly, because Folly is typically packed with a `.pc` file. In
practice, that is insufficient, because the `.pc` file doesn't fully describe all the
dependencies that Folly has, and it has bugs. This crate knows about these idiosyncrasies and
provides workarounds for them.

The following snippet should suffice for most use cases:

```rust
let folly = find_folly::probe_folly().unwrap();
let mut build = cc::Build::new();
... populate `build` ...
build.includes(&folly.include_paths);
for other_cflag in &folly.other_cflags {
    build.flag(other_cflag);
}
```

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions. 

## Code of conduct

This project follows the same Code of Conduct as Rust itself. Reports can be made to the project authors.
