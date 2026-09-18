# Current Slug V2 Work Packet

Packet: WP-7-26-m7a-cc-immutable-collections-r1
Status: accepted; independent final ACCEPT; ready to integrate

## Outcome and evidence

Run pinned rules_cc public static/PIC library and linker-input constructors into
retained Rustc native-link Args using their real provider values. Inspection
found these are Starlark providers already in the authentic fixture closure;
`cc_internal.freeze` accepting only empty lists is the immediate missing bridge.
Do not invent native Cc constructors or flatten provider fields into structs.

Bazel 9.2 commit 8220c6198837d5c13d53fea211cf3282aa12408a:
CcStarlarkInternal.freeze copies Dict shallowly to an immutable dictionary,
copies Iterable shallowly to an immutable list, and returns other values.
StarlarkList.copyOf/immutableCopyOf and Dict.immutableCopyOf preserve contents;
both checkHashable methods reject direct hashing even for frozen containers.
StarlarkInfoWithSchema.isImmutable checks exported provider fields. Existing
Slug structural provider/depset hashing and AnalysisValueLowerer remain owners.
Pinned rules_cc 0.2.17 create_library_to_link.bzl, create_linker_input.bzl and
cc_info.bzl in the existing source-pinned fixture establish constructor behavior.

Admit list/tuple-to-list and dict-to-dict shallow immutable copies plus
non-iterable Starlark-value pass-through needed by these constructors. Java
string/bool values are rejected by the pinned StarlarkValue signature; local
range/set iterable conversion remains unsupported and rejects explicitly. Preserve order, container
type, element identity and nested mutability. Direct list/dict hashing remains
unsupported as in Bazel. Exact admitted collection operations/mutation rejection;
conservative rejection of structurally mutable nested provider hash inputs and
unsupported iterable forms is Slug-native. Paths retain existing Slug-native
identity; no change to exact configuration/output bytes.

## Ownership and representation

Use separate read-only evaluator collection variants under existing ListGen /
ListLike and DictGen / DictLike abstractions; do not add a flag to every normal
list/dict or reuse the hashable dictionary variant. Boxed immutable list slices
and SmallMap dictionary storage have Trace/Freeze/Allocative ownership. ListRef,
DictRef, collection operations and module freeze must recognize these values.
No unsafe reinterpretation of live values as FrozenValue. Review optimized VM
paths: construction-only mutation may use unchecked mutable access; ordinary
mutation must reject read-only values. Keep generic algorithms at their current
owners and isolate additional representations in cohesive files where possible.

Collections live in evaluator/module heaps; configured lowering copies through
the existing shared AnalysisValueLowerer, preserving all provider fields and
depset alias identity. No evaluator value enters DICE or action storage, no
new cache, DICE key, lock or alternative semantic identity. Module freeze and
GC preserve collection content/alias lifetime. Mutable nested values remain
mutable through a shallow copy; they cannot silently acquire a stable hash.

Full CcInfo publication exposed its native empty HeaderInfo leaf. The pinned
CcCompilationContext.HeaderInfo (lines 558, 744-758) owns an identity token,
equals/hash by that token, and is immutable. Retain a typed Arc-backed
CcHeaderInfoOccurrence through the existing live empty HeaderInfo, module
freeze, AnalysisValue lowering and materialization. Starlark/direct retained
Eq/Hash use token identity; PublicationEqState uses a bidirectional occurrence
mapping to preserve alias partitions across recomputation. Fresh-versus-shared
occurrence behavior follows pinned source; token storage/publication identity
is Slug-native, not Bazel SymbolGenerator bytes. No counter, zero
marker, provider/struct surrogate or pointer-derived persistent identity.
Nonempty HeaderInfo construction remains unsupported; no hidden nonempty state
can enter the empty producer. Prove native equality/hash, distinct/shared
occurrences and materialization round-trip plus full CcInfo publication.

## Scope and validation

Allowlist: starlark-rust/starlark/src/values/types/{list,dict,structs} and their
module export files for read-only allocation/views and structural traversal;
app/slug_loading_v2/src/cc_common.rs plus focused tests (new module preferred)
and obsolete empty-only rejection assertions in host_package_load_tests.rs;
Build API src/{cc_header_info.rs,analysis_value.rs,lib.rs} and tests/analysis_value.rs,
loading lib.rs exports, analysis src/analysis_value.rs and focused round-trip test;
analysis tests/rustc_map_each/{mod.rs,subject.bzl} and fixture.toml; canonical,
manifest, Stage 6/9/bootstrap summaries. The independently reviewed native
HeaderInfo leaf is the documented lowering gap;
no other production analysis/action changes.
No copied upstream body or fixture closure growth. Existing large files receive
bounded dispatch; independent design and final review required.

Prove list/dict type, ordering, equality, indexing, slicing, iteration, ordinary
copy/concat behavior, every mutator category, direct hash rejection, shallow
alias/mutation, module freeze/GC and provider/depset structural behavior. Real
public cc_common static/PIC/alwayslink library -> linker_input -> linking_context
-> CcInfo -> Rustc builder proof checks retained providers, exact argv/param
bytes and same-DICE edit/restoration. Existing struct-backed dynamic/indirect
proof stays a regression; no claim to dynamic solib/LTO/toolchain execution.

Compile owner/dependents separately with pinned nightly under 60s preparation
caps. Preflight exact selectors, run focused expected-subsecond tests; tests
over a few seconds are infrequent and over roughly 30s require strict necessity.
Check direct analysis/query/REAPI consumers, rustfmt, diff/archive/plan. Do not run
full-suite, daemon, compiler action, live transport or Bazel build.

Resolve any immutable representation/VM safety contradiction before activation.
A constructor reaching toolchain/solib/LTO behavior cannot be faked or bypassed;
retain it as an explicit required future dependency. Full M7A requires remaining
Rustc callbacks, Cc toolchain support and resolved Spawn/input execution.

Predecessor WP-7-25 at 1dbb0ad07 is pushed: native-link Args with imported-source
authentication and full tuple/publication identity; nine focused tests and
independent final ACCEPT. The overall bootstrap goal remains open.

## Acceptance receipt

The unchanged public rules_cc static/PIC and alwayslink library constructors,
linker-input and linking-context constructors, and CcInfo initializer now feed
real providers to the unchanged Rustc builder. Full CcInfo publication retains
its empty native HeaderInfo. The proof changes an otherwise unused linker input
(same argv, different retained result), restores it, reloads/restores cc_info.bzl
(new HeaderInfo token, equal publication), and rejects invalid library extensions.
Exact native argv and forced multiline parameter bytes are asserted.

Separate pinned-nightly preparation operations stayed within 60s: the largest
observed test preparation was 47.35s; final API, loading and analysis preparation
finished in 2.30s, 6.12s and 11.15s respectively. The unavailable rustup snap
launcher used the previously verified direct pinned binaries. Initial focused
checks caught and corrected the special-list allocation flag and test-harness
source-path/scope/GC/TupleRef mistakes. Only affected gates were repeated.
Exact-selector preflight and final execution passed:

- Loading 7/7 in 0.01s: reads, all mutator categories, shallow aliases, direct
  hash rejection, GC (automatic/disabled/forced), module freeze/cycle,
  provider/depset hash, native HeaderInfo identities and iterable/scalar negatives.
- Build API 3/3 in 0.00s: HeaderInfo occurrence/publication aliases (including
  cross-provider pairs), prior provider alias and frozen-container barriers.
- Analysis unit 1/1 in 0.00s: native type, fields, hash/alias preservation and
  lower/materialize/lower round-trip.
- Authentic analysis 2/2 in 0.55s: public CcInfo/Rustc proof and prior native-link
  callback/source restoration regression.

`cargo check -p slug_query_v2 -p slug_reapi_v2` exited 0 in 18.63s with existing
warnings. Rustfmt, diff, archive and plan checks pass. Receipts: `target/wp726/`.
Packet/review wall time was not separately measured. No broad suite, daemon,
compiler action, live transport or Bazel build ran.

Fixture hygiene review reused the same 54 pinned source files plus the recorded
reused source/generated proxy; all 54 source digests/lengths match, no copied
implementation or load-closure file was added. Added proofs reuse the existing
workspace constructor, provider lowerer and argument builder.

Independent final review ACCEPT confirmed shallow collection semantics,
Freeze/GC ownership, native occurrence identity, publication alias bijection and
finite source evidence. This advances static/PIC Cc provider production and
retention. Nonempty HeaderInfo, dynamic solib/LTO operations, Cc toolchains,
remaining Rustc callbacks and resolved Spawn/input execution still block M7A.
