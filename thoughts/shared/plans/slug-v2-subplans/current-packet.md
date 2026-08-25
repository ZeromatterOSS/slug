# Current Slug V2 Packet

Packet: `WP-1-4-5-6-7A-selected-registry-extension-source-observable-oracle`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: `01-compliance-oracle-harness.md`,
`04-starlark-loading-and-build-packages.md`,
`05-bzlmod-and-repository-graph.md`, and
`06-analysis-toolchains-and-actions.md`
Base: scope-correction design after stopped contract `dc85f527`

Result: add the uniquely smaller Bazel 9.2 evidence prerequisite before any
selected-registry source-owner Rust implementation.

## Why evidence is first

The selected-source ownership remains unchanged: Bzlmod's retained canonical
selected-module definition owns semantic association and repository view;
loading owns source bytes and recursive evaluation. The earlier rules_rust
success observable was invalid because its source evaluates deferred
`repository_rule(doc=...)` and collection schemas before exporting `rust`.

Accepted fixtures do not isolate the corrected surface. The rules_rust fixture
crosses the deferred declaration/toolchain stack. The nonroot extension
fixture has no root request and its extension declares repository rules. Pinned
Bazel's `testLoadBzlFileFromBzlmod` proves a selected module's mapped load but
does not compose a root usage, same-repository cross-package child and clean
module-extension export.

Pinned Bazel 9.2 source closes the design law but not that composite proof:

- `RegularRunnableExtension.java:105-166` loads the definition through
  `BzlLoadValue.keyForBzlmod` before named-export inspection;
- `BzlLoadFunction.java:797-839,935-955,1071-1125` resolves declarations with
  the current producer repository's full mapping and requests recursive
  children in declaration order; and
- `BzlLoadFunctionTest.java:1045-1083`,
  `testLoadBzlFileFromBzlmod`, records the selected producer's self and mapped
  entries and successfully consumes its mapped dependency.

Two independent read-only audits therefore reject immediate Rust and select
one Bazel-only oracle.

## Mandatory fixture-hygiene preflight

The last recorded reset is accepted tree `51540963`. Fixture/harness payloads
in the live checkout match last fixture-tree commit `3ac0a85b`, and more than
five accepted row-bearing packets have landed between those points. Before
creating the new fixture, compare the tracked fixture/payload archives at those
two commits; inventory each added/changed row, asset, mutation, manifest and
expected field; inspect duplicate blobs/subtrees; record exact regular-file,
symlink, newline and row counts, packet attribution, pruning allowlist and
affected replay set in Stage 1; and obtain independent hygiene review.

If that checkpoint does not return `ACCEPT`, stop without adding the fixture.
If it accepts, `3ac0a85b` becomes the reset and this oracle is packet one.

## Exact fixture

Add only `tests/v2_oracle/fixtures/selected-registry-extension-source-owner/`.
It is a hermetic local-registry fixture with two deterministic archive-backed
modules and the minimal local stubs for the ten ordinary pinned `bazel_tools`
dependencies.

- Root declares only `owner@1.0`, then
  `use_extension("@owner//:extension.bzl", "probe")`. It has no direct
  dependency or mapping for `mapped_dep`, and no root `BUILD` file/package.
- `owner@1.0` declares `mapped_dep@1.0` with apparent name `mapped_dep`.
  Its `extension.bzl` loads `//shared:local.bzl` and
  `@mapped_dep//:mapped.bzl`, prints exactly
  `SELECTED_REGISTRY_MARKER:local:mapped`, defines an otherwise no-op
  implementation function, and exports
  `probe = module_extension(implementation = _probe_impl)`. Use no repository
  rule, tag class, repository context or generated repository.
- Both archive roots contain required `BUILD.bazel` package markers. Track the
  source trees and two deterministic ustar artifacts; record exact SHA-256/SRI
  values and source.json integrity. Create archives with sorted names, epoch
  mtime, numeric owner/group zero and no host path.
- The ten ordinary built-in dependency entries are the exact versions from
  pinned `@bazel_tools/MODULE.bazel`. Use fixture-local `local_path` stubs.
  Seven need only registry MODULE/source metadata. `buildozer`, `rules_java`
  and `rules_cc` additionally retain only the already-established extension/
  autoload files required by this command. Copy those stub bytes from the
  accepted local-registry fixture; do not invent upstream content or add an
  online fallback.

Run exactly one command:

```text
mod show_extension owner//:extension.bzl%probe
    --lockfile_mode=off
    --registry=file://%workspace%/registry
```

It must exit zero, print the combined marker on stderr, and show both
`@@owner+//:extension.bzl%probe` and `Usage in <root>` on stdout. The root's
missing mapped dependency makes root-view resolution fail; the absent root
`shared` package makes root-relative resolution fail. Success therefore
discriminates the selected owner association, same-owner cross-package load,
mapped selected child view and named clean export in one row.

Use `comparison = "message_shape"`, empty command mutations and no Slug run.
Slug is known to stop before this owner; replaying it would assert an
unsupported path rather than evidence. Generate once with pinned
`/usr/bin/bazel` 9.2.0, then exact-replay from two distinct fresh absolute run
roots with only the local registry. Require identical normalized marker/show
fields, exit and empty mutation/manifest state, plus focused/full harness,
schema/listing, archive/integrity, cap, credential, process and diff checks.

The fixture is exactly 46 regular files, zero links and one command: fixture
TOML/expected JSON, root MODULE, registry descriptor, owner/mapped metadata,
their two source trees and artifacts, and 28 ordinary-dependency stub files.
Cap newline-counted text at 500 lines, the two generated artifacts at 24 KiB
aggregate, fixture TOML at 75 lines and expected JSON at 90 lines. The
implementation records measured counts and stops instead of widening them.

## Compatibility, architecture and stops

The fixture is **exact** evidence for Bazel 9.2 root-owned, non-isolated
selected-registry definition loading, same-repository cross-package loading,
mapped selected-repository loading and module-extension export. Harness
normalization/provenance is **Slug-native**. Actual rules_rust repository-rule
declarations, collection schemas/calls, repository effects, toolchains,
providers/actions/input trees, crate_universe, public Slug activation, M8/M7B
and exact configuration/output bytes remain **unsupported/deferred**.

Pinned `../zabel` commit `c7298478e2e56262a2f438e9c065325744c9f0fc`
remains concept-only guidance: the producer's selected descriptor/view must be
consumed as a typed source fact, and physical realization cannot repair
visibility. It supplies no oracle bytes, diagnostics or Zig implementation.

Write authority is exactly the new fixture subtree, Stage 1 hygiene record,
canonical/current/Stages 4/5/6 and the orchestration routing log. Harness code,
payload, every accepted fixture, Rust/tests/Cargo/BUILD outside the fixture,
generated/vendored production content and callers are read-only. Documentation
growth is capped at 520 net lines and fixture growth at the exact bounds above.

STOP online registry access, a second command/fixture, mutation, Slug replay,
existing-fixture drift, harness/payload change, repository-rule or tag/schema
semantics, invented `@bazel_tools` content, Java/JVM, Rust, public breadth,
cap/provenance/replay drift, milestone closure, M8/M7B or a failed hygiene
checkpoint. After oracle `ACCEPT`, design only the corrected six-file
`WP-4-5-6-7A-selected-registry-extension-bzl-source-owner-design`; do not
reactivate the stopped implementation contract directly.
