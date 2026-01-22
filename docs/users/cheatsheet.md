---
id: cheat_sheet
title: Cheat Sheet
---

# Kuro Cheat Sheet

This section provides example command lines that you can use to obtain
information about Kuro and about your build. These techniques can help you to
understand how your build works and to troubleshoot issues with your build.
These examples use the [`kuro cquery`](../query/cquery) command. We recommend
cquery over uquery in most cases because cquery operates on the configured
graph, which means that targets have had the expected configurations applied on
them.

---

- How do I find all the targets for a package?
- How do I specify more than one target to `kuro cquery`?
- How do I get the attribute names and values for the targets that result from a
  query?
- How do I perform a query inside of a rule?
- How do I find the dependencies for a target, that is, the targets on which a
  specified target depends?
- How do I find the reverse-dependencies for a target, that is, the targets that
  depend on a specified target?
- How do I find the build file that contains the target that owns a source file?

---

### How do I find all the targets for a package?

Specify a _build target pattern_ that represents the targets in the package.

```sh
kuro cquery path/to/dir/...
```

The `kuro cquery` command can accept a
[build target pattern](../../concepts/target_pattern) as a parameter. If you
specify a build target pattern, Kuro evaluates this pattern and shows all the
build targets that match it.

### How do I specify more than one target to `kuro cquery`?

Use the `kuro cquery set()` operator. The following command line returns the
target `main` in the build file in the root of the Kuro project and all the
targets from the build file in the `myclass` subdirectory of the root.

```sh
kuro cquery "set( ':main' 'myclass:' )"
```

### How do I get the attribute names and values for the targets returned by a query?

Add the `--output-attribute <ATTRIBUTE>` or `--output-all-attributes` option to
the command line, followed by regular expressions that represent the attributes
of interest.

```sh
kuro cquery "deps(foo:bar)" --output-attribute 'name' 'exported_headers'
```

The `--output-attribute` option enables you to specify which attributes Kuro
should return. Instead of returning the names of the targets that match the
query expression, Kuro returns the names and values of the specified attributes
for those targets in JSON format. Attributes are specified as regular
expressions. For example, `'.*'` matches all attributes. See the
[`kuro cquery` docs](../query/cquery) for more details. The output for the
example query above might look something like the following.

```json
{
  "root_cell//foo/bar/lib:lib": {
    "exported_headers": ["App/util.h"],
    "name": "lib"
  },
  "root_cell//foo/bar:app": {"exported_headers": ["App/lib.h"], "name": "app"}
}
```

### How do I perform a query** \***inside**\* **of a rule?

Kuro supports certain string parameter macros to be used when defining a
target. You can use the query macros as such:

```sh
$(query_targets "queryfunction(:foo)")
$(query_outputs "queryfunction(:foo)")
$(query_targets_and_outputs [SEPARATOR] "queryfunction(:foo)")
```

Note, however, that the query macros are supported only for rule attributes of
type `attrs.arg`, such as [`genrule`](../../prelude/rules/core/genrule) and
[`apk_genrule`](../../prelude/rules/android/apk_genrule).

### How do I find the dependencies for a target?

Use the `deps()` operator.

```sh
kuro cquery "deps('foo:bar')"
kuro cquery "deps('foo:bar', 1, first_order_deps())"
kuro cquery "deps(set('foo:bar' 'foo:lib' 'foo/baz:util'))"
```

The `deps` operator finds the dependencies of the specified targets. The first
argument represents the targets of interest. This can be a single
[build target](../../concepts/build_target) or
[build target pattern](../../concepts/target_pattern), or a set of these. The
optional second argument is the _depth_ of the search for dependencies from the
specified targets. For example, `1`, as shown in the example above, returns only
the direct dependencies. If you do not provide this argument, the output is the
complete set of transitive dependencies. How do I find the reverse-dependencies
for a target, that is, the targets that** \***depend on**\* **a specified
target? Use the `kuro cquery rdeps()` (reverse dependencies) operator. The
following example, returns the targets in the
[transitive closure](https://en.wikipedia.org/wiki/Transitive_closure) of
`foo:bar` that depend directly on `example:baz`.

```sh
kuro cquery "rdeps('foo:bar', 'example:baz', 1)"
```

### How do I find the buildfile that contains the target that owns a source file?

In order to find the build file associated with a source file, combine the
`owner` operator with `buildfile`. For example,

```sh
kuro uquery "buildfile(owner('foo/bar/main.cpp'))"
```

or alternatively

```
kuro cquery "buildfile(owner('foo/bar/main.cpp'))" --target-universe 'foo:baz'
```

These two commands first find the targets that _own_ `foo/bar/main.cpp` and then
return the build files, such as `foo/bar/BUCK`, that define those targets.

`cquery` requires a `--target-universe` to be passed when the query has no
target literals. See more in
[target universe glossary entry](../concepts/glossary.md#target-universe)
