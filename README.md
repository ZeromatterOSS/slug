<div class="title-block" style="text-align: center;" align="center">

# Slug

**Bazel-compatible builds, powered by Buck2 and Rust**

![Status] ![License]

[Status]:
  https://img.shields.io/badge/status-pre--alpha-orange.svg
[License]:
  https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blueviolet.svg

---

</div>

Slug is a Bazel-compatible build tool that uses
[Buck2](https://github.com/facebook/buck2)'s high-performance Rust internals to
run standard [Bazel](https://github.com/bazelbuild/bazel) 9.0 BUILD files and
bzlmod modules.

Named after the [Costasiella kuroshimae](https://en.wikipedia.org/wiki/Costasiella_kuroshimae)
(the "leaf sheep" sea slug) &mdash; this slug absorbs the chloroplasts in the
algae it eats via kleptoplasty, which seemed apt given the goal of absorbing
Bazel into Buck2's codebase. Incidentally, the original author was a UC Santa
Cruz alumnus, so marine slugs seemed like a nice homage.

## Why Slug?

Slug is a research project to answer two questions: how much can AI do when
given a perfect reference spec, and how fast could Bazel be if it dropped the
JVM and used Buck2's execution engine?

It aims to be a drop-in replacement for Bazel, making use of Buck2's internals.
Specifically, it leverages:

- **DICE** &mdash; Buck2's deterministic incremental computation engine for fast,
  correct rebuilds
- **starlark-rust** &mdash; a mature Starlark interpreter with optional type
  annotation support (ahead of Bazel's upcoming type system)
- **Rust throughout** &mdash; the entire build tool is native Rust, from the
  Starlark evaluator to the action execution pipeline

The result is a build tool that reads your existing Bazel BUILD files and
MODULE.bazel configuration, fetches from the Bazel Central Registry, and runs
your builds with less overhead.

## Status

Slug is in **pre-alpha**. It is under active development and not yet suitable for
production use. APIs, CLI flags, and behaviors may change without notice.
The project is provided for educational and research purposes, and is in large
part an exercise in experimenting with agentic programming on a substantial
systems codebase.

### What works today

- **BUILD.bazel / MODULE.bazel** &mdash; Bazel 9.0 build files and bzlmod
  dependency management
- **Bazel Central Registry** &mdash; fetching and caching BCR modules with
  lockfile support
- **Rules ecosystem** &mdash; tested against:
  - [rules_cc](https://github.com/bazelbuild/rules_cc) 0.2.16 (cc_library,
    cc_binary, cc_test; static and dynamic linking)
  - [rules_rust](https://github.com/hermeticbuild/rules_rust) 0.40.0
    (rust_library, rust_binary)
  - [rules_python](https://github.com/bazelbuild/rules_python) 1.8.0
    (py_library, py_binary, py_test)
  - [protobuf](https://github.com/protocolbuffers/protobuf) 33.4+
    (proto_library, cc_proto_library)
  - [rules_oci](https://github.com/bazel-contrib/rules_oci) (oci_image via
    rules_pkg)
  - [bazel_skylib](https://github.com/bazelbuild/bazel-skylib) 1.5.0
- **Platforms** &mdash; Linux and Windows (macOS support is planned)
- **Query** &mdash; `deps`, `rdeps`, `allpaths`, `somepath`, `kind`, `attr`,
  `filter`, `buildfiles`, `tests`; `--output=label/json/build/graph`
- **Local sandboxing** &mdash; namespace-based build isolation on Linux
- **Remote execution** &mdash; RE API compatible (BuildBarn, BuildBuddy,
  EngFlow, NativeLink)

### What's not supported

- **Bazel versions before 9.0** &mdash; no WORKSPACE file support
- **Android / iOS rules** &mdash; not a current priority
- **macOS** &mdash; not yet tested

## Installing

Slug is currently build-from-source only. You'll need a recent Rust nightly
toolchain.

```bash
git clone https://github.com/ZeromatterOSS/slug.git
cd slug
cargo build --release
```

The binary will be at `./target/release/slug`.

## Quick start

Slug reads standard Bazel project layouts. If you have an existing Bazel 9.0
project with `MODULE.bazel` and `BUILD.bazel` files, you can try:

```bash
slug build //...
slug test //...
slug query "deps(//my:target)"
slug run //:my_binary
```

## Credits

Slug is developed by Zeromatter Inc, with primary authorship by Walter Gray
([walter-zeromatter](https://github.com/walter-zeromatter) /
[yeswalrus](https://github.com/yeswalrus)).

Slug is a fork of [Buck2](https://github.com/facebook/buck2) by Meta Platforms,
Inc. The DICE incremental computation engine, starlark-rust interpreter,
superconsole terminal UI, and remote execution architecture originate from the
Buck2 project. We're grateful for Meta's decision to open-source Buck2 under a
permissive license.

## License

Slug is licensed under both the MIT license and Apache-2.0 license; the exact
terms can be found in the [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE) files, respectively.
