# Current Slug V2 Packet

Packet: `WP-5-builtin-bazel-tools-selected-graph-owner-design`
Milestone: cross-stage M7 prerequisite design
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: freeze the sole future Host discovery-to-MVS owner, or replan into
the first missing prerequisite, before activating the embedded module.

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

## Accepted implementation contract

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

Commit `3bc745de` accepts this callerless leaf. Focused and full bzlmod tests,
downstream loading/core checks, formatting, scope/cap/forbidden-edge scans,
and independent implementation review passed. No production consumer or
forbidden graph edge landed.

## Active design contract

Audit the accepted Host root, override, visible-lockfile, registry-policy and
registry-file owners; direct-nonregistry preparation/evaluation; the embedded
module leaf; and all dormant resolution scaffolds. Trace pinned Bazel 9.2
`ModuleFileFunction`, discovery, override rewriting, selection, and dep-graph
ownership only as far as needed to freeze one compact Host
discovery-to-MVS owner.

The design must specify:

- the sole key identity and exact ordered inputs, including root Need/error
  ordering, normalized registries, command/environment policy, visible
  lockfile semantics, explicit overrides, and the embedded snapshot;
- default built-in sentinel insertion only when no user/command override for
  `bazel_tools` already won, with an override bypass that does not compute the
  embedded leaf;
- discovery request identity, source/provenance equality, recursive frontier
  behavior, override rewriting, registry versus nonregistry routing, and
  complete evaluated-module retention;
- MVS input/output identity, original/nodep dependency treatment,
  single/multiple-version override behavior, compatibility constraints,
  deterministic semantic equality, and DICE invalidation;
- a compact selected-graph value sufficient for later canonical/full mapping,
  extension-name, registration, RepoSpec/yanked/hash, and lockfile owners,
  without computing any of them in this packet; and
- the exact first implementation allowlist, formatted production/test/total
  caps, focused A/B/A/error proof, serial validation, and independent review
  gate.

If the current accepted owners cannot compose into that single key without a
new prerequisite, return `REPLAN` and freeze exactly the first missing owner.
Do not conceal a multi-packet graph implementation inside one large design.

## Compatibility

Exact: the Bazel 9.2 MODULE bytes/hash, empty-key built-in identity, complete
evaluated directive contents, and declaration order. Slug-native: DICE type
names, diagnostics outside accepted shapes, manifest framing, compact
allocation, and non-Bazel identity bytes. Unsupported/deferred: the designed
discovery/selection owner until implemented, post-selection mappings and
extension identities, repository materialization/public routing, lockfile
writing, package/BUILD/Bzl loading, configured toolchains, Test, command
activation, execution/results/BEP/coverage, Windows, JVM/Java, and exact Bazel
identity bytes.

## Scope, proof, and stops

Edit only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`;
  and
- `thoughts/shared/plans/slug-v2-subplans/08-ruleset-and-command-conformance.md`.

Cap formatted net documentation growth at 260 lines. Add no Rust, Cargo/BUILD
metadata, asset, fixture/oracle record, generated file, dependency, public
surface, command behavior, or production representation.

Validation is `git diff --check`, exact-scope/net-line checks, active-layout
archive validation, cross-document packet-name consistency, and independent
latest-diff design review. The source audit must name exact Bazel 9.2 source
owners and the live Slug seams/rejections used by the conclusion. Stop with
`REPLAN` on a required unowned input, incomplete evaluator/provenance value,
cyclic DICE ownership, second graph, guessed RepoSpec/mapping/extension name,
root-only merge, eager full-registry scan, untracked Host/network IO, lock-held
compute, public consumer, JVM/Java, fifth file, or cap excess.
