---
id: overview
title: Overview
---

# Overview

This is an overview of using Kuro to build Go projects. It assumes you have a
basic understanding of Kuro and Go. If you are completely new to Kuro, see the
[Kuro Getting Started](../../../getting_started/index.md) to learn the basic
concepts.

## Just need an example?

Check out the
**[examples/toolchains/go_toolchain](https://github.com/ZeromatterOSS/kuro/tree/main/examples/toolchains/go_toolchain)**
project for an example of a Go project using Kuro. This example supports
hermetic toolchains, third-party dependency management, cross-compilation, and
multiple execution platforms.

## The UX differences between Kuro and `go build`

Kuro is a general-purpose build system, so you need to provide more information
about your project:

- You need to tell Kuro that specific code is Go code. This is done by
  declaring targets like `go_binary` in `BUCK` files.
- You need to tell Kuro where dependencies of a particular target are. This is
  done by adding `deps` to the target definition.
- You need to configure Kuro where to find the Go compiler and other tools by
  adding `go_toolchain` to the `toolchains` cell. You also need to map some
  Kuro configuration options to Go options like GOOS/GOARCH.

## The types of targets

- `go_binary` - a binary target (`package "main"`)
- `go_library` - a library target (other packages)
- `go_test` - a test target (tests for any packages)
- `go_exported_library` - a target that exports a C-compatible interface for Go
  code (a special case of `package "main"`)

## How to write Go targets

Kuro offers lots of flexibility in how you can write your targets, but it makes
sense to stick to the following conventions for better compatibility with the
rest of the Go ecosystem:

- Keep a single Go package per directory. For example, for a Go library, all
  non-test `.go` files should belong to a single `go_library` and all
  `*_test.go` files to a single `go_test`.
- Put a `BUCK` file in the same directory as the Go package, unless you have a
  reason not to.

```python
# File: foo/BUCK

go_library(
    name = "foo",
    srcs = glob(["*.go"], exclude = ["*_test.go"]),
    deps = [
        "//path/to/other:lib",
    ],
)

go_test(
    name = "foo_test",
    srcs = glob(["*_test.go"]),
    target_under_test = ":foo",
    deps = [
        "//path-to-third-party/vendor/go/github.com/stretchr/testify:assert",
    ],
)
```

## How to pass options to `kuro` commands

### Envs GOOS and GOARCH

Compilation for different platforms is done by passing `--target-platforms` or
`-m` (`--modifier`) flags to `kuro` commands.

You need to specify what target platforms you support by declaring them with the
`platform()` rule, or you can avoid pre-declaring them by using configuration
modifiers.

For example, to build for linux/amd64, the following commands are equivalent
(assuming your project confugured similary to
[this example](https://github.com/ZeromatterOSS/kuro/tree/main/examples/toolchains/go_toolchain)):

```sh
$ GOOS=linux GOARCH=amd64 go build example.com/foo/bar
$ kuro build --target-platforms root//platforms:linux_x86_64 root//foo/bar:bar
$ kuro build -m config//os:linux -m config//arch:x86_64 root//foo/bar:bar
```

### Test options like `-test.bench`

To pass test options, use `--` to separate kuro options from test options:

<OssOnly>
```sh
$ kuro test root//foo/bar:bar -- -test.bench=.
```
</OssOnly>
<FbInternalOnly>
> **Note:** You need to use `run` instead of `test` otherwise you'll be passing options to TPX
```sh
$ kuro run root//foo/bar:bar -- -test.bench=.
```
</FbInternalOnly>
