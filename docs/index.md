---
id: index
title: Introduction
---

import { FbInternalOnly } from 'docusaurus-plugin-internaldocs-fb/internal';

Welcome to Slug, a Bazel-compatible build tool developed by Zeromatter Inc, with
primary authorship by Walter Gray. Slug is provided for educational and research
purposes and is in large part an exercise in experimenting with agentic
programming on a substantial systems codebase.

Slug is derived from [Buck2](https://github.com/facebook/buck2), originally
developed by Meta Platforms, Inc. Slug preserves attribution to Buck2 and Meta
for inherited code, documentation, and architecture while evolving as a separate
project targeting Bazel 9 compatibility.

Slug's core is written in [Rust](https://www.rust-lang.org/).
[Starlark](https://github.com/bazelbuild/starlark), which is a deterministic,
immutable dialect of Python, is used to extend the Slug build system, enabling
Slug to be language-agnostic. With Starlark, users can define their own custom
rules.

Slug leverages the Bazel spec of
[Remote Build Execution](https://bazel.build/remote/rbe) as the primary means of
parallelization and caching, which increases the importance of idempotency (no
matter how many times an operation is performed, it yields the same result) and
hermeticity (code is sealed off from the world), giving the right results,
reliably.

Slug's compatibility work currently focuses on Bazel 9 BUILD files, bzlmod, and
the Bazel rules ecosystem.

The following sub-sections contain a list of links to key points in the Slug
Documentation website that explain the advantages of using Slug for you and
your team.

## Slug Documentation Website Links

### For end users

- [Getting Started](getting_started/index.md) - how to get started with using
  Slug.
- [Benefits](about/benefits/compared_to_buck1.md) - the benefits of using Slug.

<FbInternalOnly>

- [Migration Guide](users/migration_guide.fb.md) - how to port projects from
  Buck to Slug, including the issues you might face and notable differences.
- [Slug and Build Observability](users/build_observability/observability.fb.md) -
  how to use Slug's datasets to analyze specific invocations or classes of
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
  rules from Buck1 to Slug.

</FbInternalOnly>

### For people integrating with Slug

- [Extending Slug via BXL](./bxl) - powerful Starlark scripts for introspection
  of Slug's graphs.
- [Slug change detector](https://github.com/facebookincubator/slug-change-detector) -
  tools for building a CI that only builds/tests what has changed in diff/PR.
- [Slug GitHub actions installer](https://github.com/dtolnay/install-slug) -
  script to make GitHub CI with Slug easier.
- [Reindeer](https://github.com/facebookincubator/reindeer) - a set of tools for
  importing Rust crates from crates.io, git repos etc and generating a BUCK file
  for using them.
- [ocaml-scripts](https://github.com/facebook/ocaml-scripts) - scripts to
  generate a BUCK file enabling the use of OCaml packages from an OPAM switch.
- [Buckle](https://github.com/benbrittain/buckle) - a launcher for Slug on a
  per-project basis. Enables a project or team to do seamless upgrades of their
  build system tooling.

### External articles about Buck2 and inherited components

- [Introducing Buck2](https://engineering.fb.com/2023/04/06/open-source/slug-open-source-large-scale-build-system/) -
  Meta's initial introduction when Buck2 was open sourced.
- [Reddit AMA](https://old.reddit.com/r/rust/comments/136qs44/hello_rrust_we_are_meta_engineers_who_created_the/)
  where the Slug team answered a number of questions.
- [Using buck to build Rust projects](https://steveklabnik.com/writing/using-buck-to-build-rust-projects) -
  working through an initial small Rust project, by
  [Steve Klabnik](https://steveklabnik.com/). Followed up by
  [building from crates.io](https://steveklabnik.com/writing/using-cratesio-with-buck)
  and [updating Slug](https://steveklabnik.com/writing/updating-buck).
- [Awesome Slug](https://github.com/sluongng/awesome-slug) is a collection of
  resources about Slug.
- [Slug Unboxing](https://www.buildbuddy.io/blog/slug-review/) is a general
  review of Slug by [Son Luong Ngoc](https://github.com/sluongng/).
- [A tour around Slug](https://www.tweag.io/blog/2023-07-06-slug/) gives an
  overview of Slug and how it differs from Bazel.

### External videos about Slug

- [Accelerating builds with Slug](https://www.youtube.com/watch?v=oMIzKVxUNAE)
  Neil talks about why Slug is fast.
- [Slug: optimizations & dynamic dependencies](https://www.youtube.com/watch?v=EQfVu42KwDs)
  Neil and Chris talk about why Slug is fast and some of the advanced
  dependency features.
- [Building Erlang with Slug](https://www.youtube.com/watch?v=4ALgsBqNBhQ)
  Andreas talks about building WhatsApp with Slug.
- [antlir2: Deterministic image builds with Slug](https://www.youtube.com/watch?v=Wv-ilbckSx4)
  talks about layering a packaging system over Slug.

### External projects using Slug

- [System Initiative](https://www.systeminit.com/) build their DevOps product
  [using Slug](https://nickgerace.dev/post/system-initiative-the-second-wave-of-devops/#under-the-hood),
  with their own custom prelude.
- [Rust `cxx` library](https://github.com/dtolnay/cxx) has examples and tests
  with a wide variety of build systems, including Slug.
- [`ocamlrep` library](https://github.com/facebook/ocamlrep) allows for interop
  between OCaml and Rust code, and can be
  [built with Slug](https://github.com/facebook/ocamlrep/blob/main/README-BUCK.md).
- [`slug-nix`](https://github.com/thoughtpolice/slug-nix) is an experiment to
  integrate Slug, [Sapling](https://sapling-scm.com) and
  [Nix](https://nixos.org) together in a harmonious way.

Feel free to
[send a PR](https://github.com/ZeromatterOSS/slug/edit/main/docs/index.md) adding
your project. Pull requests may or may not be reviewed.

<FbInternalOnly>

### For people developing Slug

- [Basic README](https://www.internalfb.com/code/fbsource/fbcode/slug/README.md) -
  how to get started, compile Slug and the basic workflows.
- [Notes for Developers](developers/developers.fb.md) - more advanced workflows
  and notes around debugging, profiling etc.

</FbInternalOnly>
