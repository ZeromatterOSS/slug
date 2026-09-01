# Current Slug V2 Packet

Packet: `WP-6-7B-module-extension-metadata-construction-and-capture-implementation-r2`

Milestone: M7A generic Starlark/ruleset closure; module-extension metadata
construction, return capture and generated-repository validation.

Status: terminally `ACCEPTED`. Architecture R1 returned `REPLAN`, focused R2
rereview accepted the corrected design, and terminal implementation review
accepted the bounded failure-order correction.

The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked
at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`;
do not edit or stage it.

## Replay-selected objective

Commit `3c5603a3c` terminally accepted the complete ordinary module-extension
tag-schema category. The authentic rules_rust 0.73 configured-query replay now
passes that category and stops when `rust/extensions.bzl` returns
`module_ctx.extension_metadata(reproducible = True)`.

Implement the complete Bazel 9.2 metadata construction and capture category,
not that literal call. Add the named-only `root_module_direct_deps`,
`root_module_direct_dev_deps`, `reproducible`, and `facts` constructor surface;
the opaque `extension_metadata` return value; `None`-or-metadata implementation
return validation; generated-repository membership validation; and the
`module_ctx.root_module_has_non_dev_dependency` field that real extensions use
to choose metadata. Carry one heap-independent metadata value in the existing
module-extension invocation receipt and DICE equality.

`ctx.facts` hydration, facts-version/lockfile lifecycle, reproducible lockfile
reuse, incorrect-`use_repo` warning/fixup generation and `mod tidy` mutation are
separate lifecycle categories. This packet must leave typed inputs for them and
must not silently claim those effects.

## Compatibility boundary

Admit as **exact** for Bazel 9.2:

- named-only binding, defaults, types and failure order for all four
  `extension_metadata` parameters;
- paired unspecified direct-dependency fields; paired list/tuple string
  sequences; the single `"all"` plus exactly an empty Starlark-list companion;
  valid
  user-provided repository names; duplicate and regular/dev overlap rejection;
- boolean `reproducible`, including retention when direct dependencies are
  unspecified and facts are empty;
- finite JSON-like facts construction: string-keyed dictionaries, strings,
  arbitrary Starlark integers, finite floats, booleans, `None`, lists, tuple to
  list normalization, recursively sorted dictionary keys and the seven-level
  nesting bound;
- opaque metadata type identity; module-extension implementations may return
  only `None` or metadata produced by this host; `None` normalizes to the same
  default semantic value as explicit `extension_metadata()`; returned metadata
  is detached before evaluator release and participates structurally in DICE
  equality;
- when a root usage exists, validation that explicit direct dependencies were
  generated, `"all"` expansion over generated names, and rejection of nonempty
  regular/dev sets without matching non-dev/dev root proxies; when no root
  usage exists, skip this fixup/validation exactly as Bazel does; and
- exact `root_module_has_non_dev_dependency` from matching root proxies.

Keep as **Slug-native** Rust valid-Unicode strings and error decoration,
compact Rust retention, DICE scheduling/cancellation/accounting, and the fact
that Slug currently evaluates selected extensions instead of reusing Bazel
lockfile extension results.

Keep **unsupported/deferred** non-finite fact floats, `ctx.facts` hydration,
nonempty returned facts until their lockfile/facts-version owner is connected,
reproducible lockfile omission/reuse, hidden/workspace facts merging,
`--lockfile_mode=error` facts comparison, incorrect-`use_repo` warnings and
fixup commands, `mod tidy`, isolated usages and exact Java diagnostic
decoration. Non-finite or nonempty returned facts fail closed; no unmodeled
metadata input may be discarded. Direct-dependency success/error semantics are
implemented now, but warning-only import classification remains deferred.

The stock fixture runner's unsupported query flags, expressions and Starlark
output mode are unrelated CLI surfaces. They remain deferred; the supported
direct configured-query replay is the packet discriminator.

## Bazel 9.2 authority and peer guidance

Bazel tag `9.2.0` commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole semantic authority.
Pinned sources are:

- `ModuleExtensionContext.java`
  `36460dcfafadce9581c146e06d1394b6d5bf47224840d0debeb7c96993898711`;
- `ModuleExtensionMetadata.java`
  `b5b3d4e6486c1f48ae55667ce442b014ed3344ef4011e1f57cdd3580f67cb38c`;
- `Facts.java`
  `820c305b23c92f5780d33a3fc24e90bdb91d1aaf02de81659f4ace4dc49cb160`;
- `RepositoryName.java`
  `29697296225f187fc10281bf068aca2752a78e986e1cdfdb5c2bb34a8498e8e2`;
- `RegularRunnableExtension.java`
  `1c91439270aef8dcd1d4615dd40369e51431cebaee36f8dc085a51a9d0aead20`;
- `LockfileModuleExtensionMetadata.java`
  `42c1195c6693b8d0780582a9dc82690d4708787c8f545e3bb334a55b61b132ea`;
- `SingleExtensionEvalFunction.java`
  `e97d01acd89834147bef7825e25563a1af8787edd59ae4307fbe97d3f23000a2`;
  and
- `ModuleExtensionResolutionTest.java`
  `d8602fd385d34ab5387cb0ef3891ef9acc0ca62cd8f67324e09fd33ea7a3e769`.

Reuse the pinned `extensionMetadata_*`, `facts_*`, return-type and root-usage
tests as discriminating evidence; add no redundant Java oracle artifact.

Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance only.
`module_extension_metadata_value.zig`
`2aa1e18a1042a10f5c102813b298665f387f88c0d67b81ac9fece069d4509d6a`
and `module_extension_execution_capture.zig`
`8f03505b2302f79443d3ab95f12cbca2b65eec8a417ff94e739fb9fafcd06fc0`
support a typed transient callable, evaluator-release detachment and one
capability record. Copy no Zig allocator, evaluator, representation,
diagnostic, lifecycle, scheduler, cache or behavior.

## Frozen architecture

Create one public Bzlmod-owned metadata family and reuse the existing lockfile
facts representation rather than adding a loading-only graph:

- `ModuleExtensionRepositorySelection`: unspecified, all, or explicit compact
  set;
- `ModuleExtensionMetadata`: both selections, `reproducible`, and normalized
  facts; and
- the existing sorted `Facts`/`FactValue`/`FactNumber` family, promoted behind
  a narrow constructor/query API and still used by lockfile v28.

Retain `CompactString`, starlark-rust `SmallSet`/`SortedMap`, immutable `Arc`
slices and `Allocative`. Explicit dependency equality is set-like while source
order remains available for stable future rendering. Facts dictionaries are
sorted; lists remain ordered; arbitrary integers retain canonical decimal
spelling; finite-float equality follows the existing lockfile facts owner.
Add no second fact graph, serde value tree, global table, interner or cache.

Both the selected-owner input and legacy root-aggregate input retain the
already-parsed matching root proxies, preserving dev classification, imports,
locations and include-file ownership for future warning/fixup work. The
selected-owner projection distinguishes an absent root usage from a present
root usage whose retained proxy slice describes its dev/non-dev occurrences;
the legacy root aggregate is root-present by construction. They expose only
narrow accessors; no new DICE key or reverse edge is added. The two invocation
paths must produce identical context fields, metadata capture and validation
behavior for equivalent inputs.

`InvocationContext` constructs an invocation-local opaque Starlark wrapper.
After evaluation, downcast and clone its shared metadata before the `Module`
and evaluator are released. Add one non-optional, default-normalized
`ModuleExtensionMetadata` to the existing receipt: `None` and explicit
`extension_metadata()` publish the same value and are equal in DICE. Do not
retain an explicit-return bit in semantic equality or create a parallel result
path. Repository instantiation stays unchanged. Only when the invocation has a
root usage does certificate validation compare metadata selections to
instantiated generated names and retained root proxy kinds. A selected-owner
nonroot-only invocation skips that validation and reports
`root_module_has_non_dev_dependency = False`.

Nonempty facts are fully type-checked and normalized at construction, but a
returned nonempty facts value fails closed before certificate publication until
the existing lockfile/facts-version inputs are connected. This preserves the
future representation without pretending persistence is implemented.

## Ownership, caps and stop conditions

Production allowlist:

- one new shared metadata module plus `app/slug_bzlmod_v2/src/lib.rs`,
  `app/slug_bzlmod_v2/src/lockfile_v28.rs` and
  `app/slug_bzlmod_v2/src/selected_repo_spec.rs` and
  `app/slug_bzlmod_v2/src/selected_repo_spec/selected_extension_demand.rs`;
- `app/slug_loading_v2/src/module_extension.rs`; and
- `app/slug_loading_v2/src/module_extension_repository_validation.rs`.

Proof allowlist additionally admits only the two mechanical receipt-literal
field additions in
`app/slug_loading_v2/src/module_extension_repository_instantiation.rs`.

Focused proof may use colocated tests, the existing lockfile-v28 facts tests,
the selected-owner input corridor and the selected module-extension invocation
tests. The authentic rules_rust fixture remains external replay evidence; do
not edit it. Gross caps are 900 production Rust lines, 1,000 proof Rust lines
and 1,900 total.

Do not touch the parked proof, parser, rules_rust, toolchain, C++, `cc_common`,
`cc_internal`, query/CLI parsing, another lockfile version, materializer or
remote execution. Add no key, lock, cache, retained evaluator value, custom
Starlark collection or ruleset-specific branch. Stop with `REPLAN` if facts
cannot reuse the lockfile semantic owner, metadata cannot detach before heap
release, root proxy identity is unavailable without another graph key, or a
required exact effect crosses the deferred lifecycle boundary.

## Required proof and validation

Prove the four-keyword matrix, ordinary explicit list/tuple forms, both `"all"`
directions with exactly an empty list companion, and rejection of empty tuple
or `None` companions; omitted/explicit `None`, repository grammar,
duplicates/overlap, facts scalar and nested forms, tuple normalization, key
sorting, arbitrary integers, finite floats, depth and unsupported types. Prove
opaque return acceptance, wrong return rejection, metadata survival after
evaluator drop, DICE equality differences for each retained field, semantic
equality of a `None` return and explicit `extension_metadata()`, empty/nonempty
facts boundary and root non-dev classification.

Prove generated-name membership, all expansion and root dev/non-dev failure
order after repository instantiation when root usage is present. Prove a
selected-owner nonroot-only invocation skips those checks even for metadata
that would fail them if a root usage existed. Reuse existing repository-call
proofs; do not copy a nondiscriminating ruleset fixture.

Run formatting/diff checks, focused Bzlmod/loading tests, full
`slug_bzlmod_v2` and `slug_loading_v2` suites, direct analysis/query/core/server
checks, pinned hashes, clean Bazel/Buck2/Zabel trees, parked hash and the known
archive baseline. Rebuild `slug_cli_v2`, clean `slugd` before and after, and
replay the authentic rules_rust fixture. It must clear
`extension_metadata(reproducible=True)` without a consumer special case; the
next genuine generic failure selects the successor.

Independent architecture review is required before Rust and independent
terminal review before acceptance.

## Immediate predecessor

Commit `3c5603a3c` terminally accepts
`WP-6-7A-module-extension-tag-attribute-schema-category-implementation-r5` at
592 production/777 proof/1,369 total gross Rust lines and advances authentic
replay from `auth: StringDict` to this metadata boundary.

R1 independent architecture review returned `REPLAN` on three bounded Bazel
9.2 mismatches. R2 freezes the `"all"` companion as an empty list rather than
an arbitrary empty sequence, normalizes `None` and explicit default metadata
to one receipt value, and conditions generated-name/root-polarity validation
on root-usage presence. All ownership, allowlists, caps and deferred lifecycle
boundaries remain unchanged. Focused independent R2 rereview is required
before Rust.

Focused R2 rereview returns `ACCEPT`: the empty-list-only `"all"` companion,
default-normalized receipt value and root-presence-conditioned validation are
exact and discriminating. The shared owner, lifetime, allowlist, caps and
deferred boundaries remain coherent. Rust implementation is authorized only
within R2.

Terminal implementation review first returned `REPLAN` because metadata
validation followed import/override validation and resolved regular generated
names before dev names. The correction validates metadata first, resolves dev
then regular, retains regular-then-dev polarity checks, and adds both
dual-invalid precedence proofs. Focused terminal rereview returns `ACCEPT`.
The implementation is accepted at 584 production, 356 proof and 940 total
gross Rust lines. Full owner/dependent suites and the rebuilt CLI pass apart
from recorded out-of-packet core/server baselines. Authentic rules_rust 0.73
replay clears `extension_metadata(reproducible=True)` and stops at the next
generic host gap: attribute flag `SKIP_CONSTRAINTS_OVERRIDE` in rules_shell.
