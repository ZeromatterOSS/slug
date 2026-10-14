# WP1220 General executable environment provider

Packet: WP-7-420-m7a-general-run-environment-provider

Status: accepted; [receipt](./wp1220-general-run-environment-provider-acceptance-2026-10-14.json).

Outcome: the verified Rust cquery passes RunEnvironmentInfo construction at
rust.bzl375 through a general native provider API. Admit immutable construction,
field access/publication, configured-return validation and retained executable
launch metadata as one capability. Exact named Bazel9.2 API behavior; structural
identity/canonical native order remain Slug-native. No source/caller profiles.

Owner and contract: Loading snapshots ordered string-map entries and ordered,
duplicate-preserving inherited names into native immutable Arc data. Getters
return fresh mutable containers. Starlark field equality/hash ignore map order;
publication equality preserves observable getter order. Typed native lowering
and frozen materialization preserve nested provider values. Only declared
executable/test rules may return the constructor result; validation precedes
completed-action publication. No evaluator/context or ambient environment retained.

RunfilesSupport retains launch environment separately from configured SymlinkTree
action environment. Reuse RetainedActionEnvironment through a narrow constructor
from independent fixed/inherited canonical channels; overlapping names remain in
both. Inherited-name normalization belongs only to that launch projection. Actual
client resolution and run/test launch, deprecated testing.TestEnvironment warning
policy and Windows runfiles remain deferred. Existing for_action behavior stays.

Scope: small native RunEnvironmentInfo model leaf and existing provider wiring;
Loading genuine value/constructor leaf; Analysis lower/materialize/return and
runfiles completion; native launch-field plumbing and ordinary constructor/test
corrections. Preserve inherited mixed candidates using baseline subtraction.
No new DICE key/lock, global cache/interner, filesystem or source observation.
The public continuation exposed source-demand grouping by module name alone;
partition by (source scope, exact request ID) so distinct canonical repositories
retain separate original requests/paths. Preserve same-ID conflicts, anchor
cardinality/workspace/nested ownership and deterministic validation order.

Validation: pinned RunEnvironmentInfoApi/RunEnvironmentInfo/NativeInfo,
Starlark.fromJava, configured-return validation and RunfilesSupport sources;
unrelated constructor binding/type failures, snapshot/getter mutation isolation,
freeze/drop and hash/equality, ordered publication/nested dependency A/B/A,
configured executable/non-executable and exact declared Test/Suite predicate matrix, fixed/inherited overlap and unchanged
SymlinkTree environment. Full configured test implicit-tool/source closure remains
unproved; no invented tool fixtures. Named owner/direct dependents, fresh CLI/public query,
qualified integration suites and independent final review. Reuse prior Args and
runfiles gates; no authentic/memory/fixed-point closure through remaining adapters.

Decisions: ordered immutable SmallMap/Arc payload, no occurrence equality; public
constructor always error-on-non-executable, deprecated producer unsupported.
Independent design review accepted interfaces/launch allocation contract; empty
typed ranges are accepted as empty Bazel Sequences. Root serializes Cargo and owns Analysis/integration; native and Loading
writers have disjoint leaves. Source preparation and routine fixes stay here.

Predecessor: [WP1219 receipt](./wp1219-general-args-callbacks-acceptance-2026-10-14.json)
accepted at2780bd0ef99121ad36bf9a02730b95967bf60704. It retires the entire Rust/Cargo
Args callback family, passes86 focused checks and isolated owned-tree compilation;
final diagnostic64.693s reaches this provider gap. Other adapters remain banned
replacement work; M7A/M8, action comparison and equivalent-command memory stay open.
