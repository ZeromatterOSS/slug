---
id: what_is_kuro
title: What is Kuro?
---

Welcome to Kuro! If you're new here, this page will give you a brief overview
of what Kuro is and the key reasons you might consider using it.

## What is Kuro?

Kuro is a Bazel-compatible build tool developed by Zeromatter Inc, with primary
authorship by Walter Gray. It is provided for educational and research purposes
and is in large part an exercise in experimenting with agentic programming on a
substantial systems codebase.

Kuro is derived from Buck2, originally developed by Meta Platforms, Inc. It keeps
credit and attribution for Buck2 and Meta while evolving as a separate project
targeting Bazel 9 compatibility.

Here are a few key things to know about Kuro:

- **Bazel 9 compatibility**: Kuro aims to run standard Bazel 9 BUILD files and
  bzlmod modules.
- **Designed for Large Monorepos**: Kuro inherits Buck2's focus on large,
  incremental builds.
- **Open Source**: You can find its source code and contribute at
  [https://github.com/ZeromatterOSS/kuro](https://github.com/ZeromatterOSS/kuro).
- **Fast internals**: Kuro builds on Buck2's Rust internals, including DICE.
- **Correctness**: Kuro aims to preserve Bazel-compatible hermeticity and
  dependency semantics.
- **Extensible**: Allows developers to easily extend and customize their build
  process through Starlark.

## Why Use Kuro? Key Advantages

- **Performance:**
  - **Faster Parallel Builds:** Kuro is architected to build different parts of
    your project simultaneously (in parallel) whenever possible, significantly
    speeding up the overall build process.
  - **Low Incremental Build Time:** After you make a small change to your code,
    Kuro is very efficient at only rebuilding what's necessary. This leads to
    faster iterations when you're developing and testing.

- **Determinism and Reproducibility:**
  - **Hermetic Builds:** Kuro aims for "hermetic" builds. This means that
    builds are self-contained and don't depend on external factors or
    pre-installed tools on your machine that aren't explicitly declared. This
    ensures that if you build the same code, you get the same result, every
    time, regardless of where or when it's built.

- **Transparency:**
  - **Dependency Comprehension:** Kuro has a clear way of defining and
    understanding the dependencies between different parts of your code. You can
    use queries to explore and understand these relationships, which is
    invaluable in large projects.

- **Correctness at Scale:** By ensuring builds are reproducible and dependencies
  are explicit, Kuro helps maintain correctness even as your codebase grows and
  becomes more complex.

- **Language Extensibility:** Kuro's rule system is designed to be extensible,
  allowing support for new languages and tools to be added.

- **Remote Execution and Caching:** Kuro supports distributing build actions
  across multiple machines (remote execution) and caching build results. This
  can dramatically speed up builds, especially for lage teams and projects, as
  work done by one developer can benefit others.

In essence, Kuro is designed to make the build process faster, more reliable,
and more understandable, especially for large and complex software projects. For
a more in-depth look at these advantages, you can visit
[Why Use Kuro?](../../about/why/).
