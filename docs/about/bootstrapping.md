---
id: bootstrapping
title: Bootstrapping Slug
---

# Bootstrapping Slug

Slug can be built with `cargo` or `slug`. The source repository includes
[DotSlash](https://dotslash-cli.com) files for `slug` itself, so that you can
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
cd slug/
./bootstrap/reindeer --third-party-dir shim/third-party/rust buckify
```

Build a copy of `slug` with `slug`:

```sh
./bootstrap/slug build //:slug
```
