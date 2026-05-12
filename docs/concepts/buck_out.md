---
id: buck_out
title: buck-out
---

# buck-out

Slug stores build artifacts in a directory named `buck-out` in the root of your
[project](glossary.md#project). You should not make assumptions about where
Slug places your build artifacts within the directory structure beneath
`buck-out` as these locations depend on Slug's implementation and could
potentially change over time. Instead, to obtain the location of the build
artifact for a particular target, you can use one of the `--show-*-output`
options with the [`slug build`](../../users/commands/build) or
[`slug targets`](../../users/commands/targets) commands, most commonly
`--show-output`. For the full list of ways to show the output location, you can
run `slug build --help` or `slug targets --help`.

```sh
slug targets --show-output <target>
slug build --show-output <target>
```
