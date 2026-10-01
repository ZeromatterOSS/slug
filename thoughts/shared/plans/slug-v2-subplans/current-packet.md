# Current Slug V2 Work Packet

Packet: WP-7-175-m7a-rust-toolchain-captured-callable-design-r1
Status: accepted design; independent DESIGN/FINAL accepted; M7A remains partial

## Outcome and evidence

Baseline `da449cbbb` (WP874). Resolve the actual captured Rust-toolchain callback's
semantic/source/lifetime owners before activating publication. This is a design
packet because generic frozen closures versus finite native recipes is a new
retained-value/code ownership choice; an implementation-only getter cannot satisfy
it. No runtime activation, callable placeholder, precomputed CcInfo substitution,
producer ctx retention or generic raw Def serialization is authorized here.
Select a distinct finite source-authenticated Rust-toolchain recipe with complete
invocation behavior, following accepted Stage 6 WP763 ownership. Generic frozen
code/module carriers remain deferred. This decision is bootstrap-aligned only if
the full imported helper semantics execute against the consumer's live actions.

Authentic unchanged WP87478.763s exit2 stops at recursive ToolchainInfo publication:
`$[0].make_libstd_and_allocator_ccinfo: unsupported analysis value of type function`.
Preserved verbatim Loading host_package_load_tests.rs:5568+ toolchain source
43,390 bytes SHA c4b613cee96540a94fbdf4fbdca7b8dc4ef6d3082024c4d3636afc2e9c4d468e
(test31042–31050); source560–570 defines nested make_ccinfo, source606 publishes it.
This is NOT the imported/global helper itself. Four captures are cc_toolchain,
feature_configuration, experimental_link_std_dylib and rust_std. Producer ctx/actions
occur only in separate make_local_ccinfo; consumer supplies label/actions/allocator/
std to the published closure. No capture mutation follows the constructor.
Allocator helper source at host_package_load_tests.rs:6572–6875 is11,932 bytes
SHA ae4acb50ac6a1b922254a07346d97b4649810d33836f2be4824fd0b7a81e536e (test30580–30586).
Root independently extracted/hashes these constants; target/wp875/source-research.json.

Pinned Bazel9.2 `8220c619`: ToolchainInfo.java:58–99 admits arbitrary Starlark values;
CcToolchainProviderTest.java:199–226 publishes a loaded global function through a
provider and invokes it in a new thread. That lower-bound lifetime evidence does
not prove nested captures. Actual rules_rust source and new genuine captured-producer/
consumer discriminators will own that gate. Source-exact named recipe behavior,
Slug-native structural configuration/artifact and occurrence/publication identity;
generic closure and full callable/inspection parity remain unsupported/deferred.

## Chosen owners and implementation obligations

Loading authenticates canonical caller/source, observed AND evaluated SHA, exact
ToolchainInfo callsite and _rust_toolchain_impl frame, imported allocator-helper
source and load provenance. Existing evaluator.native_call_local_value at
runtime/evaluator.rs:532–558 reads/unwraps one directly calling Def local; it neither
searches outer/module frames nor grants provenance. After authentication it can
read all four captures without exposing private DefGen captures/code. Register the
genuine function's occurrence/name-hash and captures in a traced evaluator/module
root, freeze/GC safely, and snapshot only after evaluation stops. Missing/malformed
captures or changed caller/helper/import provenance fail closed. Source aliases,
shadowed/imported wrappers and native-call-context confusion must not authenticate.

BuildAPI owns a distinct immutable Rust-toolchain callable model/source authority
and all structurally lowered captures, with existing Arc/Dupe/Allocative patterns.
Keep BuildAPI Starlark-free; don't repurpose CcToolchainCallable's four kinds. Public
function equality/hash preserve original occurrence/name hashing; publication
compares every capture/source and complete alias partition, preserving A/B/A cutoff
without a heap address as semantic identity. No evaluator/frame/module Value or
producer sink/token enters retained semantic data. New representation must use the
Buck2 utility skill and relevant Stage9 existing callable disposition.

Loading runtime and Analysis lower/materialize bridge rematerialize capture graphs
and the callable. Implement the complete original four-argument binding, validation
order and helper behavior via existing Cc/provider/depset/native action owners:
allocator struct/field/provider validation; self-contained compilation outputs and
linking pipeline/additional toolchain inputs; allocator archive variants; ordered
alloc/between/core/panic/memchr/std/test topological library layers and panic filters;
static/dynamic std, std/no_std_with_alloc modes; CcInfo merging or original None case.
Consumer label/actions, active-token checks, current configured root and output owner
remain authoritative. Never replace this with publication-only success or None.

Implementation must resolve the concrete builder/dispatcher seam and source/private
access authority before activation. Existing public builders mostly live in
rules_cc Starlark bodies, so neither blindly looking up a same-named export in the
consumer nor retaining unmodeled frozen helper code is an approved shortcut. A
source-owned existing helper execution seam or full native operation composition
needs its own reviewed implementation contract; this packet chooses the finite
semantic owner, not an unverified invocation adapter. Any discovered unadmitted
self-contained/dynamic action prerequisite stays named/open rather than replaced
with a generic Spawn or swallowed error. Coupled publication/invocation activation
must land atomically after those gates; preparatory data models alone are not parity.

## Scope, acceptance and next gate

Docs only: manifest/canonical and Stage6 callable owner contract; Stage9 only if a
new reuse disposition is actually selected. No Rust, dependencies, Starlark engine,
fixture/source bodies, CLI, query/executor or platform changes. Windows minimal.
Independent DESIGN/FINAL review of the actual chosen contract; source hashes,
source/owner cross-checks, diff/archive/plan checks. No Cargo/oracle required for
this ownership decision; WP874 replay is retained evidence, not callback execution.
Implementation's required discriminators: genuine producer disposal/heap release,
GC/freeze, original/proxy equality/hash and nested/aliased republication; all captures/
source/helper/configuration ABA; four-argument binding and invalids; both allocator
forms/std modes, nonempty ordered libraries/panic filters, dynamic/self-contained
branches; consumer-owned native actions and expired/suspended refusal. Reuse existing
cc_toolchain_tests/rustc_map_each lifecycle fixtures and full-source authentic rules
instead of new copied archives. Unchanged authentic replay and eventual allocator/
link execution remain required; cquery advancement alone proves no invocation.

REPLAN implementation for raw frozen-code lifetime ownership, unmodeled globals/
defaults/cycles/captures, new action prerequisites or a publication-only adapter.
Root may resolve authorized architecture choices without user confirmation. This
design is complete only when the owners and remaining implementation seam are
explicitly reviewed; full M7A/bootstrap stays unproved.

Independent DESIGN/FINAL accepted the docs-only ownership decision. Source hashes,
owner checks and diff/archive/plan checks pass; no Cargo or new oracle was needed.
The feasible next seam uses Loading's existing DICE-owned FrozenBzlModule and load
manifest, prepared before synchronous Analysis evaluation and passed to both
materializers as phase code bindings. Its finite API, source/private authority and
invocation lifetime still require implementation review. No runtime gate advanced.
Packet elapsed about20 minutes (03:37–03:57 UTC); target/wp875/source-research.json
retains source evidence. No workflow speedup claim.

## Predecessor

WP874 `da449cbbb`: independent corrected DESIGN/FINAL,12 tests in0.479s, Core/CLI
pass. Source-exact artifact-symlink eligibility with Slug-native first-row
canonicalization clears sysroot overlap; three hashes and actual cleanup pass.
Unchanged query78.763s reaches this captured-function boundary; target/wp874 receipts.
