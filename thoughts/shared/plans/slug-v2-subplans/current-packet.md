# Current Slug V2 Work Packet

Packet: WP-7-24-m7a-bzl-source-provenance-r1
Status: accepted; independent design and final reviews ACCEPT

## Outcome and demand

Retain each loaded Bzl module's observed source SHA alongside its lexical
identity, and transport that provenance through the existing source-filename
carrier to configured evaluation. Existing pinned Args callback classification
must compare the caller's evaluated source with its manifest-owned digest.
This is Slug-native source/admission integrity, not a new Bazel surface.

Demand: the next native-link callbacks in pinned rules_rust rustc.bzl
(`_libraries_dirnames`, `_make_link_flags_default_*`) depend on imported
utils.bzl `get_preferred_artifact` and `get_lib_name_default`. Hashing the native
caller's rustc.bzl span cannot authenticate those imported implementations.
At d26b42a51 the manifest retains their aggregate fingerprint but discards each
source SHA needed by the consumer. That prerequisite must be fixed before
mapping those functions into retained Rust recipes. No native-link callbacks,
Cc constructors, or execution surfaces are admitted by this packet.

## Ownership and representation

`BzlLoadManifest::new` receives observed root source bytes' SHA in all three
loading routes. Its flat first-seen reachable closure will retain
`BzlModuleSourceProvenance { identity, source_digest }` per module. Keep lexical
`BzlModuleIdentity`, load order/dedup, aggregate fingerprint, repository mapping
and frozen-module lifetime ownership unchanged. The two projections
`manifest_starlark_sources` and `package_bzl_call_sources` derive filename plus
that provenance. Replace the existing shared carrier throughout loading and
analysis; do not introduce a parallel digest bag, filesystem read, cache or
fallback for missing evidence. Non-source consumers borrow the identity field.

This is loading/DICE-retained semantic memory using existing immutable Arc
slices, CompactString, SmallSet and Allocative (Stage 9 utility dispositions).
Digests come only from the already observed source used to evaluate the module.
Source changes continue through child manifests and their aggregate fingerprint;
provenance participates in equality and rule publication. Frozen values and
source-text scratch do not become provenance. Release follows existing
manifest/context/rule lifetimes; no lock or async ownership changes.

Research: current parsed/evaluated source flow in bzl_module.rs; pinned Bazel
9.2 BzlLoadFunction:858–879 source/transitive digest ownership at
8220c6198837d5c13d53fea211cf3282aa12408a; DICE principles
in docs/developers/dice.md. Existing diamond/observed-source tests supply the
first-seen order, shared-child, update/delete/restore and event contracts.
No new upstream fixture is needed. This is a prerequisite implementation packet
because imported-function authentication otherwise has no retained source owner.

## Scope and validation

Allowlist: app/slug_loading_v2/src/{bzl_module.rs,provider.rs,package.rs,
subrule_invocation.rs,builtin_restriction.rs,analysis_fragments.rs,attrs.rs,
lib.rs} and affected source-carrier/manifest tests in that crate; direct analysis
source-carrier signature/constructor corrections only; manifest, canonical and
Stage 4/6/9/bootstrap summaries; scripts/v2_archive_status.sh for the stale
explicit V2-crate allowlist correction (accepted slug_reapi_cache_v2 only). Large existing files receive bounded transport
edits; the provenance type remains next to its sole manifest owner. No new
collection abstraction or compatibility shim is needed.

Test observed external diamond provenance (exact root/helper bytes, dedup/order,
shared cold/warm, helper edit/restoration on one Dice), manifest/context transport
and caller mismatch rejection. Protect existing authentic Rustc callback/source
restoration plus one ordinary configured Args test. Compile direct analysis,
query and REAPI consumers. Compile separately under 60s preparation operations;
preflight exact selectors; expected tests subsecond and only focused batches.
Tests above a few seconds run sparingly; over roughly 30s need strict necessity.
No daemon, full CLI, Bazel/compiler action or broad suite. Run rustfmt, diff,
archive and plan checks. Independent final review is required before integration.

Missing/ambiguous imported rows must be rejected by the later callback admission;
this packet only establishes and proves the authentic carrier. If any production
route cannot provide its observed source digest, resolve that ownership instead
of filling a sentinel or reading host files.

Predecessor WP-7-23 at d26b42a51 accepted forced virtual parameter-file expansion
and atomic REAPI tree composition with 11 focused tests and independent review.
M7A and typed Spawn execution remain open.

## Acceptance receipt

Observed provenance is retained by the sole manifest constructor in all three
production loading routes. The existing filename table carries it through
loading and configured evaluation; lexical identity and aggregate fingerprint
algorithms are unchanged. The pinned callback now rejects manifest/span digest
mismatch. No imported native-link callback or Cc constructor was activated.

Pinned nightly direct binaries (the already verified fallback for the unavailable
rustup snap launcher) compiled loading in 20.89s. After routine test-constructor
corrections, separate `cargo test --no-run --message-format=json` preparation
succeeded for loading unit tests in 38.25s, Bzl invalidation integration in
23.75s and authentic analysis in 33.90s. All preparation operations stayed
within their 60s cap. Exact-selector preflight/execution passed:

- Loading 6/6, 0.04s: imported-helper source SHA/order/A/B/A, observed diamond
  cold/warm events, pinned caller digest mismatch, lexical caller identity and
  private API checks.
- Bzl invalidation 2/2, 0.03s: first-seen diamond and leaf/load-edge changes.
- Analysis 2/2, 0.43s: authentic Rustc source-edit/restoration and generic Args.

`cargo check -p slug_analysis_v2 -p slug_query_v2 -p slug_reapi_v2` exited 0
in 14.83s, with three existing REAPI deprecation warnings. Rustfmt, diff, archive
and plan checks pass. The archive checker required one reviewed allowlist entry
for the already accepted V2 cache crate; no V1 negative was weakened.
Receipts: `target/wp724/`. Total packet/review wall time was not separately
measured. No full suite, daemon, compiler action or live transport ran.

Independent final review ACCEPT confirmed owner/equality/lifetime and evidence.
This closes the imported-source provenance prerequisite only; native-link
callback authentication/recipes, ordinary input resolution and Spawn execution
remain required M7A work.
