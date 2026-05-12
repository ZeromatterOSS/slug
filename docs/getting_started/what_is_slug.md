---
id: what_is_slug
title: What is Slug?
---

Welcome to Slug! If you're new here, this page will give you a brief overview
of what Slug is and the key reasons you might consider using it.

## What is Slug?

Slug is a Bazel-compatible build tool developed by Zeromatter Inc, with primary
authorship by Walter Gray. It is provided for educational and research purposes
and is in large part an exercise in experimenting with agentic programming on a
substantial systems codebase.

Slug is derived from Buck2, originally developed by Meta Platforms, Inc. It keeps
credit and attribution for Buck2 and Meta while evolving as a separate project
targeting Bazel 9 compatibility.

Here are a few key things to know about Slug:

- **Bazel 9 compatibility**: Slug aims to run standard Bazel 9 BUILD files and
  bzlmod modules.
- **Designed for Large Monorepos**: Slug inherits Buck2's focus on large,
  incremental builds.
- **Open Source**: You can find its source code and contribute at
  [https://github.com/ZeromatterOSS/slug](https://github.com/ZeromatterOSS/slug).
- **Fast internals**: Slug builds on Buck2's Rust internals, including DICE.
- **Correctness**: Slug aims to preserve Bazel-compatible hermeticity and
  dependency semantics.
- **Extensible**: Allows developers to easily extend and customize their build
  process through Starlark.

## Why Use Slug? Key Advantages

- **Performance:**
  - **Faster Parallel Builds:** Slug is architected to build different parts of
    your project simultaneously (in parallel) whenever possible, significantly
    speeding up the overall build process.
  - **Low Incremental Build Time:** After you make a small change to your code,
    Slug is very efficient at only rebuilding what's necessary. This leads to
    faster iterations when you're developing and testing.

- **Determinism and Reproducibility:**
  - **Hermetic Builds:** Slug aims for "hermetic" builds. This means that
    builds are self-contained and don't depend on external factors or
    pre-installed tools on your machine that aren't explicitly declared. This
    ensures that if you build the same code, you get the same result, every
    time, regardless of where or when it's built.

- **Transparency:**
  - **Dependency Comprehension:** Slug has a clear way of defining and
    understanding the dependencies between different parts of your code. You can
    use queries to explore and understand these relationships, which is
    invaluable in large projects.

- **Correctness at Scale:** By ensuring builds are reproducible and dependencies
  are explicit, Slug helps maintain correctness even as your codebase grows and
  becomes more complex.

- **Language Extensibility:** Slug's rule system is designed to be extensible,
  allowing support for new languages and tools to be added.

- **Remote Execution and Caching:** Slug supports distributing build actions
  across multiple machines (remote execution) and caching build results. This
  can dramatically speed up builds, especially for lage teams and projects, as
  work done by one developer can benefit others.

In essence, Slug is designed to make the build process faster, more reliable,
and more understandable, especially for large and complex software projects. For
a more in-depth look at these advantages, you can visit
[Why Use Slug?](../../about/why/).
