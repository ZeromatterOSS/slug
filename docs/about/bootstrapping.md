---
id: bootstrapping
title: Bootstrapping Kuro
---

# Bootstrapping Kuro

Kuro can be built with `cargo` or `kuro`. The source repository includes
[DotSlash](https://dotslash-cli.com) files for `kuro` itself, so that you can
quickly self-bootstrap the build. This is particularly useful if you're writing
patches and need to test both builds.

For dependencies on Rust crates from [crates.io](https://crates.io), we use
[reindeer](https://github.com/facebookincubator/reindeer) to automatically
generate `BUCK` files.

Note that the resulting binary will be compiled without optimisations or
[jemalloc](https://github.com/jemalloc/jemalloc), so we recommend using the
Cargo-produced binary in further development.

First, install `dotslash` with `Cargo`:

```sh
cargo install --locked dotslash
```

Next, use `reindeer` to buckify dependencies:

```sh
cd kuro/
./bootstrap/reindeer --third-party-dir shim/third-party/rust buckify
```

Build a copy of `kuro` with `kuro`:

```sh
./bootstrap/kuro build //:kuro
```
