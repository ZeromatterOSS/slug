---
id: buck_out
title: buck-out
---

# buck-out

Kuro stores build artifacts in a directory named `buck-out` in the root of your
[project](glossary.md#project). You should not make assumptions about where
Kuro places your build artifacts within the directory structure beneath
`buck-out` as these locations depend on Kuro's implementation and could
potentially change over time. Instead, to obtain the location of the build
artifact for a particular target, you can use one of the `--show-*-output`
options with the [`kuro build`](../../users/commands/build) or
[`kuro targets`](../../users/commands/targets) commands, most commonly
`--show-output`. For the full list of ways to show the output location, you can
run `kuro build --help` or `kuro targets --help`.

```sh
kuro targets --show-output <target>
kuro build --show-output <target>
```
