---
id: why
title: Why Kuro
---

Kuro is a Bazel-compatible build tool from Zeromatter Inc, with primary
authorship by Walter Gray. It is derived from Buck2, originally developed by Meta
Platforms, Inc., and preserves attribution to Buck2 and Meta for inherited code,
documentation, and architecture.

Kuro is provided for educational and research purposes and is in large part an
exercise in experimenting with agentic programming on a substantial systems
codebase. This page answers the questions:
[why does Kuro exist](#why-does-kuro-exist),
[what's different about Kuro](#whats-different-about-kuro), and
[why use Kuro](#why-use-kuro).

## Why does Kuro exist?

Meta employs a very large monorepo, consisting of a variety of programming
languages, including C++, Python, Rust, Kotlin, Swift, Objective-C, Haskell,
OCaml, and more. Google employs a different but functionally similar monorepo.

These large scale and multi-language repositories are generally beyond the
capabilities of traditional build systems like `make`. To optimize the build and
performance of these large systems, Facebook and Google developed their own
build systems, respectively Buck and Bazel. While the internal version of Bazel
was started first (also known as Blaze), Buck was open sourced first (back in
March 2013), followed by Bazel a few years later (March 2015).

The retroactively named Buck1 was a capable build system, but had significant
limitations and has been entirely phased out at Meta today. Kuro is a rewrite
that aims to keep the best bits of Buck1 (with a high degree of target
compatibility) but also borrows ideas from
[academic](https://ndmitchell.com/#shake_10_sep_2012)
[research](https://ndmitchell.com/#shake_21_apr_2020) and build systems,
including [Bazel](https://bazel.build/), [Pants](https://www.pantsbuild.org/),
[Shake](https://shakebuild.com/), [Tup](https://gittup.org/tup/), and more.

Following are aspects common to Buck1 and Kuro (and in most cases, Bazel):

- **Targets that can be queried** - the build is defined as a series of targets,
  specified in `BUCK` files, that depend on other targets. This graph of targets
  can be queried to understand how they relate to each other and what the
  potential impact of a change might be.
- **Remote execution** - the build can send actions to a set of remote servers
  to be executed, increasing the parallelism significantly.
- **Multi-language composability** - there can be lots of different languages in
  a single build, and they can be put together. For example, you could have a
  Python library that depends on a Rust library, which, in turn depends on a C
  library.
- **File watching** - at large enough scale, simply looking for changed files is
  prohibitively expensive. Buck can integrate with
  [Watchman](https://facebook.github.io/watchman/) to discover which files have
  changed efficiently. However, for simplicity of setup, the open-source version
  defaults to using `inotify` or similar functionality.
- **Uses Starlark** - Starlark is a deterministic Python-like language used to
  specify the targets, enabling the definition of targets as literals and more
  advanced manipulation/sharing.

## What's different about Kuro?

Kuro has several major differences (as well as many minor differences) from
Buck1. Of particular note, there are a number that give new efficiency or
expressiveness (most of these are also different from Bazel).

- **Kuro is written in Rust** - Buck1 was written in Java. One of the
  advantages of using Rust is the absence of GC pauses, However, Java also has
  advantages, such as better memory profiling tools.
- **Kuro is remote execution first** - local execution is considered a special
  case of remote execution, in contrast to Buck1 where it was added after. That
  means that things such as directory hashes can be pre-computed ready to send
  to remote execution, giving efficiency benefits.
- **All Kuro rules are written in Starlark** - whereas, in Buck1, they were
  written in Java as part of the binary, which makes iteration on rules much
  faster.
- **The Kuro binary is entirely language agnostic** - as a consequence of
  having all the rules external to the binary, the most important and complex
  rule (such as in C++), don't have access to magic internal features. As a
  result, features have been made available to all rules, including:
  - [Dep files](../rule_authors/dep_files.md) - the ability to declare that a
    subset of the files weren't actually used, and thus not be sensitive to
    changes within them.
  - [Incremental actions](../rule_authors/incremental_actions.md) - the ability
    to have the action short-circuit some subset of the work if run again.
- **Kuro uses a dynamic (aka monadic) graph as its underlying computation
  engine** - while most dependencies are specified statically, there are two
  particular features that expose dynamic power to rule authors:
  - [Dynamic dependencies](../rule_authors/dynamic_dependencies.md) - enable
    rules to build a file then look at its contents before specifying the
    dependencies and steps in future actions. Common uses are languages where
    the dependency structure within a project must follow imports (e.g. Haskell,
    OCaml) and distributed ThinLTO (where the best optimization plan is
    generated from summaries).
  - [Anonymous targets](../rule_authors/anon_targets.md) - enable rules to
    create a graph that has more sharing than the original user graph. As a
    result, two unrelated binaries can compile shared code only once, despite
    the shared code not knowing about this commonality. This feature is useful
    for rules like Swift feature resolution.
- **[Transitive-sets](../rule_authors/transitive_sets.md)** - similar in purpose
  to Bazel's [depset](https://bazel.build/rules/lib/depset). But, instead of
  being just a memory optimization, are also wired into the dependency graph,
  providing a reduction in the size of the dependency graph.
- **Kuro is not phased** - there are no target graph/action graph phases, just
  a series of dependencies in a
  [single graph on DICE](https://github.com/ZeromatterOSS/kuro/blob/main/dice/dice/docs/index.md)
  that result in whatever the user requested. That means that Kuro can
  sometimes parallelise different phases and track changes very precisely.
- **Kuro can integrate with the virtual filesystem
  [Eden](https://github.com/facebook/sapling)** - this provides good
  performance, even when the file system is backed by source control fetches.
  However, Eden is not required, and a normal file system will also work well.
- **The Kuro Starlark implementation is available
  [as a standalone library](https://developers.facebook.com/blog/post/2021/04/08/rust-starlark-library/)** -
  this provides features such as IDE integration (both LSP and DAP bindings),
  linters, typecheckers, and more. These features are integrated into Kuro to
  give a better developer experience (which is still evolving).
- **Kuro supports configurations** - (such as `select`) to provide
  multi-platform/architecture builds, which are heavily inspired by Bazel.
  Within that space, there is a number of small differences, such as
  `toolchain_deps`.
- **Kuro is fast** - in our internal tests, we observed that Kuro completed
  builds 2x as fast as Buck1.

For a comprehensive list of benefits, see
[Benefits Compared to Buck1](benefits/compared_to_buck1.md).

## Why use Kuro?

Kuro is early-stage software, so users may run into unexpected issues. If you
encounter an issue, you may report it via
[Github issues](https://github.com/ZeromatterOSS/kuro/issues), but issue and pull
request review is not guaranteed.

Kuro is available as open source for educational and research use.

Kuro inherits a substantial amount of Buck2 documentation. Some pages still
describe Buck2-era concepts or Meta-internal workflows. Those references should
be treated as inherited context unless a page explicitly describes current Kuro
behavior.

There are also some things that aren't quite yet finished:

- There are not yet mechanisms to build in release mode (that should be achieved
  by modifying the toolchain).
- Windows/Mac builds are still in progress; open-source code is mostly tested on
  Linux.

If none of that puts you off, [give Kuro a go](../getting_started/index.md)!
