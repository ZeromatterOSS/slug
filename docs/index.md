---
id: index
title: Introduction
---

import { FbInternalOnly } from 'docusaurus-plugin-internaldocs-fb/internal';

Welcome to Kuro, a large scale, fast, reliable, and extensible build tool
developed and used by Meta. Kuro supports a variety of languages on many
platforms.

Kuro's core is written in [Rust](https://www.rust-lang.org/).
[Starlark](https://github.com/bazelbuild/starlark), which is a deterministic,
immutable dialect of Python, is used to extend the Kuro build system, enabling
Kuro to be language-agnostic. With Starlark, users can define their own custom
rules.

Kuro leverages the Bazel spec of
[Remote Build Execution](https://bazel.build/remote/rbe) as the primary means of
parallelization and caching, which increases the importance of idempotency (no
matter how many times an operation is performed, it yields the same result) and
hermeticity (code is sealed off from the world), giving the right results,
reliably.

Kuro multi-language support includes C++, Python, Java, Kotlin, Go, Rust,
Erlang, OCaml, and more.

The following sub-sections contain a list of links to key points in the Kuro
Documentation website that explain the advantages of using Kuro for you and
your team.

## Kuro Documentation Website Links

### For end users

- [Getting Started](getting_started/index.md) - how to get started with using
  Kuro.
- [Benefits](about/benefits/compared_to_buck1.md) - the benefits of using Kuro.

<FbInternalOnly>

- [Migration Guide](users/migration_guide.fb.md) - how to port projects from
  Buck to Kuro, including the issues you might face and notable differences.
- [Kuro and Build Observability](users/build_observability/observability.fb.md) -
  how to use Kuro's datasets to analyze specific invocations or classes of
  invocations.
- [Migrating builds to work VPNless](users/advanced/vpnless.fb.md) - how to
  migrate builds to work without VPN or lighthouse access.

</FbInternalOnly>

### For people writing rules

- [Writing Rules](rule_authors/writing_rules.md) - how to write rules to support
  new languages.
- [Build APIs](api/build) - documentation for the APIs available when writing
  rules.
- [Loading Data](users/loading_data.md) - How to load static data from JSON and
  TOML files in rules.
- [Starlark Types](https://github.com/facebook/starlark-rust/blob/main/docs/types.md) -
  rules are written in Starlark (which is approximately Python), but our version
  adds types.

<FbInternalOnly>

- [Rule Writing Tips](rule_authors/rule_writing_tips.fb.md) - tips for migrating
  rules from Buck1 to Kuro.

</FbInternalOnly>

### For people integrating with Kuro

- [Extending Buck via BXL](./bxl) - powerful Starlark scripts for introspection
  of Kuro's graphs.
- [Kuro change detector](https://github.com/facebookincubator/kuro-change-detector) -
  tools for building a CI that only builds/tests what has changed in diff/PR.
- [Kuro GitHub actions installer](https://github.com/dtolnay/install-kuro) -
  script to make GitHub CI with Kuro easier.
- [Reindeer](https://github.com/facebookincubator/reindeer) - a set of tools for
  importing Rust crates from crates.io, git repos etc and generating a BUCK file
  for using them.
- [ocaml-scripts](https://github.com/facebook/ocaml-scripts) - scripts to
  generate a BUCK file enabling the use of OCaml packages from an OPAM switch.
- [Buckle](https://github.com/benbrittain/buckle) - a launcher for Kuro on a
  per-project basis. Enables a project or team to do seamless upgrades of their
  build system tooling.

### External articles about Kuro

- [Introducing Kuro](https://engineering.fb.com/2023/04/06/open-source/kuro-open-source-large-scale-build-system/) -
  our initial introduction when we open sourced Kuro.
- [Reddit AMA](https://old.reddit.com/r/rust/comments/136qs44/hello_rrust_we_are_meta_engineers_who_created_the/)
  where the Kuro team answered a number of questions.
- [Using buck to build Rust projects](https://steveklabnik.com/writing/using-buck-to-build-rust-projects) -
  working through an initial small Rust project, by
  [Steve Klabnik](https://steveklabnik.com/). Followed up by
  [building from crates.io](https://steveklabnik.com/writing/using-cratesio-with-buck)
  and [updating Kuro](https://steveklabnik.com/writing/updating-buck).
- [Awesome Kuro](https://github.com/sluongng/awesome-kuro) is a collection of
  resources about Kuro.
- [Kuro Unboxing](https://www.buildbuddy.io/blog/kuro-review/) is a general
  review of Kuro by [Son Luong Ngoc](https://github.com/sluongng/).
- [A tour around Kuro](https://www.tweag.io/blog/2023-07-06-kuro/) gives an
  overview of Kuro and how it differs from Bazel.

### External videos about Kuro

- [Accelerating builds with Kuro](https://www.youtube.com/watch?v=oMIzKVxUNAE)
  Neil talks about why Kuro is fast.
- [Kuro: optimizations & dynamic dependencies](https://www.youtube.com/watch?v=EQfVu42KwDs)
  Neil and Chris talk about why Kuro is fast and some of the advanced
  dependency features.
- [Building Erlang with Kuro](https://www.youtube.com/watch?v=4ALgsBqNBhQ)
  Andreas talks about building WhatsApp with Kuro.
- [antlir2: Deterministic image builds with Kuro](https://www.youtube.com/watch?v=Wv-ilbckSx4)
  talks about layering a packaging system over Kuro.

### External projects using Kuro

- [System Initiative](https://www.systeminit.com/) build their DevOps product
  [using Kuro](https://nickgerace.dev/post/system-initiative-the-second-wave-of-devops/#under-the-hood),
  with their own custom prelude.
- [Rust `cxx` library](https://github.com/dtolnay/cxx) has examples and tests
  with a wide variety of build systems, including Kuro.
- [`ocamlrep` library](https://github.com/facebook/ocamlrep) allows for interop
  between OCaml and Rust code, and can be
  [built with Kuro](https://github.com/facebook/ocamlrep/blob/main/README-BUCK.md).
- [`kuro-nix`](https://github.com/thoughtpolice/kuro-nix) is an experiment to
  integrate Kuro, [Sapling](https://sapling-scm.com) and
  [Nix](https://nixos.org) together in a harmonious way.

Feel free to
[send a PR](https://github.com/facebook/kuro/edit/main/docs/index.md) adding
your project.

<FbInternalOnly>

### For people developing Kuro

- [Basic README](https://www.internalfb.com/code/fbsource/fbcode/kuro/README.md) -
  how to get started, compile Kuro and the basic workflows.
- [Notes for Developers](developers/developers.fb.md) - more advanced workflows
  and notes around debugging, profiling etc.

</FbInternalOnly>
