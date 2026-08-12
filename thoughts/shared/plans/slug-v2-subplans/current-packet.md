# Current Slug V2 Packet

Packet: `WP-5-builtin-bazel-tools-module-value-implementation`
Milestone: cross-stage M7 prerequisite implementation
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: retain the complete pinned built-in MODULE semantic value without
claiming discovery, selection, contextual mappings, or package dispatch.

## Accepted predecessor boundary

`WP-5-builtin-bazel-tools-module-injection-design` ends `REPLAN`. Bazel 9.2
does not append one synthetic root mapping: `ModuleThreadContext.buildModule`
adds `bazel_tools@<empty>` to every module except `bazel_tools`, reserves its
apparent name, and root override construction installs the non-registry
sentinel only when a user or command override did not already win. Discovery
and MVS then select the embedded module's complete dependency closure in the
ordinary graph. `BazelDepGraphFunction` derives canonical module mappings,
extension ids and collision-disambiguated unique names only after selection;
registered platform/toolchain consumers iterate that selected graph.

Slug has the exact embedded bytes/source identity and a complete compact
`EvaluatedNonrootModule`, but no Host discovery/MVS owner joins root, embedded,
registry, and direct-nonregistry modules. Its older `ResolvedGraph` is a pure
scaffold over supplied parsed files, while active Host routing is root-direct-
local plus the unconsumed built-in route. A root-only merge, partial dependency
list, guessed unique extension names, or fabricated RepoSpecs would create a
second/inexact graph. Full injection is therefore not bounded.

## Active implementation contract

Add one crate-private immutable module-value key under the existing built-in
source owner. Its identity is `BuiltinBazelToolsSnapshot`; it computes exactly
`BuiltinBazelToolsSourceFileKey(snapshot, "MODULE.bazel")`, then invokes the
existing complete nonregistry evaluator with expected key
`bazel_tools@<empty>`, logical id `@@bazel_tools//:MODULE.bazel`, and no include
files. The pinned source contains no `include()` or `print()` call. Do not copy,
reparse with the legacy handwritten `ModuleFile`, or expose an evaluator.

The retained complete value contains the existing
`BuiltinBazelToolsRouteIdentity`, the exact source SHA-256, and the existing
`EvaluatedNonrootModule`. The route identity keeps snapshot plus complete
catalog manifest identity distinct from the MODULE content hash; the evaluated
value retains every ordinary/nodep dependency, extension/proxy/use_repo/innate
repo-rule name, tag/import/isolation field, flag alias, and ordered platform/
toolchain registration. Because the expected key is the built-in empty key,
finalization adds no self dependency. Source and evaluator failures remain
typed and values/errors are complete, stable DICE results.

This key has no root, override, registry, lockfile, route, Host, package,
mapping, discovery, selection, extension-evaluation, loading, analysis, or
command dependency and no production consumer. Future combined discovery must
compute the root first; reject root apparent-name collisions; let user/command
override precedence replace the default sentinel without computing this key;
otherwise insert this exact value as `bazel_tools@<empty>` in the sole selected
graph, inject the built-in dep into every other module, and only then derive
canonical/full mappings, unique extension names, registrations, and lockfile-
relevant registry/extension state. The visible lockfile never records the
embedded MODULE hash itself; its registry hashes and extension entries remain
ordinary downstream selected-graph inputs.

## Compatibility

Exact: the Bazel 9.2 MODULE bytes/hash, empty-key built-in identity, complete
evaluated directive contents, and declaration order. Slug-native: DICE type
names, diagnostics, manifest framing, compact allocation, and non-Bazel
identity bytes. Unsupported/deferred: injection/selection, override activation,
canonical or extension-generated mapping derivation, registry fetch, lockfile
publication/replay, package/BUILD/Bzl dispatch, configured toolchains, Test,
execution/results/BEP/coverage, Host scanning, Windows, JVM/Java, and exact
Bazel identity bytes.

## Scope, proof, and stops

Edit only:

- `app/slug_bzlmod_v2/src/builtin_repository.rs`; and
- `app/slug_bzlmod_v2/src/module_eval.rs` only to make the existing complete
  direct-nonregistry evaluator seam crate-visible without changing semantics.

Cap formatted net growth at 110 production lines, 300 test lines, and 410
total. Add no file, public export, Cargo/BUILD metadata, asset, fixture/oracle
record, DICE graph owner beyond this single leaf, dependency, utility, lock,
cache, interner, process-global state, or production caller.

Focused tests must prove the exact route manifest and MODULE SHA domains; exact
ordinary aliases/versions and nodep set; no `bazel_tools` self edge; every
extension label/name/import including the innate winsdk repository rule; exact
four-item toolchain registration order; no execution-platform/flag-alias
state; typed source/evaluation error structure; separately computed equality;
one cold `Evaluated` then warm `Reused` activation with no event data; and
structural absence of root/Host/registry/lockfile/mapping/consumer edges. Run
serially:

```bash
cargo test -p slug_bzlmod_v2 builtin_bazel_tools_module
cargo test -p slug_bzlmod_v2
cargo check -p slug_loading_v2
cargo check -p slug_core_v2
cargo fmt --all -- --check
git diff --check
```

Also run source/asset-hash, credential-pattern, exact-scope/cap, no-new-public-
surface, active-layout archive, and forbidden-edge scans. Obtain independent
latest-diff review before commit. Stop with `REPLAN` on any evaluator change,
print/include behavior, missing retained directive, source identity collapse,
root/override/registry/lockfile/mapping/route/Host dependency, graph injection,
consumer, package/Bzl/configured-analysis/command/Test/execution behavior,
direct filesystem access, second graph, public surface, new dependency, third
file, or cap excess.
