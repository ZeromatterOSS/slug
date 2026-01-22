## No-prelude example

This is an example project that does not rely on
https://github.com/facebook/kuro-prelude. Instead the prelude cell points to a
`prelude` directory with an empty `prelude.bzl` file, like so:

```
#.buckconfig
[cells]
root = .
prelude = prelude
```

All rules and toolchains are defined manually within each of the subdirectories.
(e.g. `cpp/rules.bzl`, `cpp/toolchain.bzl`)

## Sample commands

Install Kuro, cd into a project, and run

```bash
# List all targets
kuro targets //...
# Build all targets
kuro build //...
# Run C++ hello_world main
kuro run //cpp/hello_world:main
```
