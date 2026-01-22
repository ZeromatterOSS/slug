<div class="title-block" style="text-align: center;" align="center">

# Kuro: fast multi-language build system

![Version] ![License] [![Build Status]][CI]

[Version]:
  https://img.shields.io/badge/release-unstable,%20"Developer%20Edition"-orange.svg
[License]:
  https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blueviolet.svg
[Build Status]:
  https://github.com/facebook/kuro/actions/workflows/build-and-test.yml/badge.svg
[CI]: https://github.com/facebook/kuro/actions/workflows/build-and-test.yml

<strong>
  <a href="https://kuro.build">Homepage</a>&nbsp;&nbsp;&bull;&nbsp;&nbsp;<a href="https://kuro.build/docs/getting_started/">Getting Started</a>&nbsp;&nbsp;&bull;&nbsp;&nbsp;<a href="./CONTRIBUTING.md">Contributing</a>
</strong>

---

</div>

Kuro is a fast, hermetic, multi-language build system, and a direct successor
to the original [Buck build system](https://buck.build) ("Buck1") &mdash; both
designed by Meta.

But what do those words really mean for a build system &mdash; and why might
they interest you? "But why Kuro?" you might ask, when so many build systems
already exist?

- **Fast**. It doesn't matter whether a single build command takes 60 seconds to
  complete, or 0.1 seconds: when you have to build things, Kuro doesn't waste
  time &mdash; it calculates the critical path and gets out of the way, with
  minimal overhead. It's not just the core design, but also careful attention to
  detail that makes Kuro so snappy. Kuro is up to 2x faster than Buck1 _in
  practice_[^perf-note]. So you spend more time iterating, and less time
  waiting.
- **Hermetic**. When using Remote Execution[^hermetic-re-only], Kuro becomes
  _hermetic_: it is required for a build rule to correctly declare all of its
  inputs; if they aren't specified correctly (e.g. a `.c` file needs a `.h` file
  that isn't correctly specified), the build will fail. This enforced
  correctness helps avoids entire classes of errors that most build systems
  allow, and helps ensure builds work everywhere for all users. And Kuro
  correctly tracks dependencies with far better accuracy than Buck1, in more
  languages, across more scenarios. That means "it compiles on my machine" can
  become a thing of the past.
- **Multi-language**. Many teams have to deal with multiple programming
  languages that have complex inter-dependencies, and struggle to express that.
  Most people settle with `make` and tie together `dune` to `pip` and `cargo`.
  But then how do you run test suites, code coverage, or query code databases?
  Kuro is designed to support multiple languages from the start, with
  abstractions for interoperation. And because it's completely scriptable, and
  _users_ can implement language support &mdash; it's incredibly flexible. Now
  your Python library can depend on an OCaml library, and your OCaml library can
  depend on a Rust crate &mdash; and with a single build tool, you have a
  consistent UX to build and test and integrate all of these components.

[^perf-note]:
    This number comes from internal usage of Buck1 versus Kuro at Meta. Please
    note that _appropriate_ comparisons with systems like Bazel have yet to be
    performed; Buck1 is the baseline because it's simply what existed and what
    had to be replaced. Please benchmark Kuro against your favorite tools and
    let us know how it goes!

[^hermetic-re-only]:
    Kuro currently does not sandbox _local-only_ build steps; in contrast,
    Kuro using Remote Execution is _always_ hermetic by design. The vast
    majority of build rules are remote compatible, as well. Despite that, we
    hope to lift this restriction in the (hopefully short-term) future so that
    local-only builds are hermetic as well.

If you're familiar with systems like Buck1, [Bazel](https://bazel.build/), or
[Pants](https://www.pantsbuild.org/) &mdash; then Kuro will feel warm and cozy,
and these ideas will be familiar. But then why create Kuro if those already
exist? Because that isn't all &mdash; the page
_["Why Kuro?"](https://kuro.build/docs/about/why/)_ on our website goes into
more detail on several other important design criteria that separate Kuro from
the rest of the pack, including:

- Support for ultra-large repositories, through filesystem virtualization and
  watching for changes to the filesystem.
- Totally language-agnostic core executable, with a small API &mdash; even C/C++
  support is written as a library. You can write everything from scratch, if you
  wanted.
- "Buck Extension Language" (BXL) can be used for self-introspection of the
  build system, allowing automation tools to inspect and run actions in the
  build graph. This allows you to more cleanly support features that need graph
  introspection, like LSPs or compilation databases.
- Support for distributed compilation, using the same Remote Execution API that
  is supported by Bazel. Existing solutions like BuildBarn, BuildBuddy, EngFlow,
  and NativeLink all work today.
- An efficient, robust, and sound design &mdash; inspired by modern theory of
  build systems and incremental computation.
- And more!

If these headline features make you interested &mdash; check out the
[Getting Started](https://kuro.build/docs/getting_started/) guide!

## 🚧🚧🚧 **Warning** 🚧🚧🚧 &mdash; rough terrain lies ahead

Kuro currently **does not have a stable release tag at this time**. Pre-release
tags/binaries, and stable tags/binaries, will come at later dates. Despite that,
it is used extensively inside of Meta on vast amounts of code every day, and
[kuro-prelude](/prelude/) is the same code used internally for all these
builds, as well. (However, Meta retains large amounts of Starlark code which
builds on top of the prelude.)

Meta just uses the latest committed `HEAD` version of Kuro at all times. Your
mileage may vary &mdash; but at the moment, tracking `HEAD` is ideal for
submitting bug reports and catching regressions.

The short of this is that you should consider this project and its code to be
battle-tested and working, but outside consumers will encounter quite a lot of
rough edges right now &mdash; several features are missing or in progress, some
toolchains from Buck1 are missing, and you'll probably have to fiddle with
things more than necessary to get it nice and polished.

Please provide feedback by submitting
[issues and questions!](https://github.com/facebook/kuro/issues)

## Installing Kuro

You can get started by downloading a
[bi-monthly version](https://github.com/facebook/kuro/tags) or the
[latest](https://github.com/facebook/kuro/releases/tag/latest) built binary for
your platform. The `latest` tag always refers to a recent commit; it is updated
on every single push to the GitHub repository, so it will always be a recent
version.

Alternately, you can use [dotslash](https://dotslash-cli.com/) with the
bi-monthly releases where it's easy to deploy into a repo with a single text
file and auto pull the correct platform as needed.

You can also compile Kuro from source, if a binary isn't immediately available
for your use; check out the [HACKING.md](./HACKING.md) file for information.

## Terminology conventions

Frequently used terms and their definitions can be found on the
[glossary page](https://kuro.build/docs/concepts/glossary/).

## License

Kuro is licensed under both the MIT license and Apache-2.0 license; the exact
terms can be found in the [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE) files, respectively.
