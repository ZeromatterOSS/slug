# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-bzl-visibility-owner-design`

Milestone: M7A command/ruleset bootstrap closure.

Result: settle one exact-default Bazel 9.2 `.bzl` load-visibility owner and
direct-edge enforcement architecture shared by every existing Slug Bzl and
BUILD evaluator. Produce an independently accepted implementation contract;
change no Rust.

## Learned facts and source-order trigger

Commit `879d879f5` accepts the complete authenticated 197-line rules_cc
`cc/private/link/link.bzl` defining-module proof. Focused, all 272 loading
library, 24/31 integration, locked analysis/core and CLI, format/diff/source
and archive-baseline gates pass within 0/352/352; independent review returned
`ACCEPT`.

Rules_cc 0.2.17 `cc/private/cc_common.bzl` next reaches dependency-free
`cc/private/rules_impl/cc_toolchain_info.bzl`, 255 lines, SHA-256
`f19589572147b7dc8f1b16ab96791b7651923c36821aed70868a74bbfce963f5`.
Its first executable statement is `visibility(["//cc/..."])` at line 18.
Slug's generic `.bzl` environment has no `visibility` binding and its retained
module value has no load-visibility fact, so a complete freeze proof is not yet
honest. A freeze-only no-op would admit forbidden cross-package loads and is
not an implementation candidate.

Run only `WP-4-7A-bazel-bzl-visibility-owner-design`. Do not change Rust,
freeze `cc_toolchain_info.bzl`, add a no-op global, or claim any C++ provider,
toolchain, rule, configured target or action behavior.

## Bazel authority and exact behavior family

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole compatibility authority:

- `BazelBuildApiGlobals.visibility` validates default-enabled
  `--experimental_bzl_visibility`, `.bzl` initialization, direct module-scope
  use, a single declaration, string-or-list input and package specifications
  resolved in the declaring module's repository context.
- `BzlInitThreadContext` captures the declaration only during evaluation;
  `BzlLoadFunction` defaults an absent declaration to public and publishes the
  policy in `BzlLoadValue`, outside the evaluator module object.
- `BzlVisibility` normalizes empty/private/public policy and matches exact or
  recursive package specifications. `BzlLoadFunction.checkLoadVisibilities`
  always permits the loaded module's own package and validates every direct
  `.bzl` or BUILD load before importer execution.
- `BzlLoadFunctionTest` methods from `testBzlVisibility_disabledWithoutFlag`
  through `testBzlVisibility_errorsDemotedToWarningWhenBreakGlassFlagIsSet`
  establish implicit/explicit public, private same/cross package, empty and
  mixed lists, dependency failure, top-level/once restrictions, exact and
  subtree packages, declaring-repository scope, repository mappings, bad
  types, negative rejection and the two flag surfaces.

The implementation contract must cover one coherent default-enabled family:
implicit/explicit public; private and empty-list behavior; string and list of
strings only; exactly one positional-only argument returning `None`, with
named, missing and excess arguments rejected; `public`, `private`, `//pkg`,
`//pkg/...`, `//...`, apparent and canonical repository spellings, plus
`:__pkg__`/`:__subpackages__` label forms accepted by Bazel's
package-specification parser; declaring-repository mapping; top-level-only and
once-only declaration; same-package override; and rejection of every denied
direct `.bzl` and BUILD load before importer evaluation.

## Selected ownership design

The implementation successor may add a cohesive private
`bzl_visibility.rs` leaf containing only immutable normalized policy,
declaration parsing and pure `allows_load_from`/direct-edge validation. Its
retained shape is `Public | Private | Packages(Arc<[PackageSpec]>)`, with each
package spec carrying canonical `PackageIdentifier` identity and exact versus
subtree kind. It derives semantic equality and `Allocative`; an `Arc` clone is
cheap. Private rows may be removed and a list containing public may normalize
to public because ordering and redundant denying rows are not observable.

The existing evaluation-scratch `BzlEvaluationContext` is the one
initialization context. Add an interior `Option<BzlLoadVisibility>` used only
during the live evaluator call. The `.bzl`-only global validates direct
module-scope use through starlark-rust's native-caller API, resolves through
the context's exact `BzlModuleIdentity`, rejects a second call, and leaves no
heap `Value` or context borrow in retained state. A focused regression must
cover direct, local-function, imported-function and compiler-inlined call
shapes; inability to distinguish them is `REPLAN`, not permission to scan raw
source or patch starlark-rust speculatively.

`FrozenBzlModule` is the durable semantic owner, parallel to Bazel's
`BzlLoadValue`. It retains the extracted policy beside `BzlLoadManifest` and
includes it in semantic equality. The manifest fingerprint covers the owner
source digest plus every direct child's identity and transitive fingerprint;
the root identity and repository mapping participate separately in manifest
equality and are framed when a parent/module package consumes this value.
Policy extraction adds no independent input and no new digest domain. Frozen
Starlark module pointers and recursive heap ownership remain lifetime-only.

Use one pure direct-edge checker before importer evaluation at all five live
composition sites:

1. `compute_host_bzl_module`;
2. `compute_external_bzl_module`;
3. local `BzlModuleEvalKey::compute`;
4. `evaluate_host_package_attempt`, shared by root and repository packages;
5. local `PackageLoadKey::compute`.

Bzl importers derive their canonical package from the manifest root. Package
attempt inputs must receive canonical `PackageIdentifier` from their existing
root or repository route; they may not infer repository identity from a path.
Validation reads already-computed direct child values and occurs before
creating or evaluating the importer. A typed denial retains dependency label
and importer package, then maps through each existing driver error owner. Do
not create a second evaluator, DICE key, side registry, post-evaluation repair,
raw-source scanner or path-derived repository fallback.

The existing `BzlModuleEvalKey`, `HostBzlModuleEvalKey`/observation key and
`ExternalBzlModuleEvalKey`/observation key remain natural DICE producers.
Their source, child, mapping and route dependencies already explain the policy.
Source or imported-policy changes invalidate and recompute normally; equality
cutoff compares the retained policy. There is no lock, await under a lock,
manual invalidation edge or command-side cache.

## Prior art, memory and compatibility

Clean Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
concept/test-only peer guidance, not authority. Its `bzl_visibility.zig`,
`engine_bzl_visibility_capture.zig`, process-stable declaration binding and
durable-module plan independently support evaluation-scoped capture,
canonical immutable policy, same-package-aware pure checking and enforcement
before importer publication. Copy no Zig code, layout, parser, diagnostics or
behavior.

Buck2 commit `088c75c7e36805df99c3de29062baa95db700b8b` is utility
guidance only. Existing Slug `Arc<[T]>`, `Dupe`, `Allocative`, canonical label
types and `starlark_map` small collections suffice; import no utility, add no
global interner and update no Stage 9 extraction row. The new policy is
DICE-retained semantic memory published with `FrozenBzlModule`, released with
that value, and never borrows evaluator, command or scratch memory. Parsing
temporaries and the mutable declaration slot are evaluation scratch. There is
no separate eviction, cancellation, task, join or shutdown owner.

- **Exact:** Bazel 9.2 default-enabled positional-only callable ABI,
  declaration placement/cardinality/type and package-spec behavior; declaring
  repository/mapping identity; implicit public and normalized policy semantics;
  same-package override; default fail-closed direct `.bzl` and BUILD edge
  result before importer evaluation; observable A/B/A restoration of those
  semantic results.
- **Slug-native:** compact Rust enum/`Arc` representation, starlark-rust
  evaluator integration, existing typed driver wrappers and diagnostic
  prefixes; DICE keys, equality cutoff and invalidation mechanics;
  normalization of redundant private/public rows; first-denial reporting
  rather than Bazel's Java event aggregation.
- **Unsupported/deferred:** `--noexperimental_bzl_visibility`; warning-only
  `--nocheck_bzl_visibility`; exact multi-violation event aggregation and Java
  stack/message bytes; `.scl`; BUILD-level invocation of `visibility`; any
  target visibility, rule/provider/toolchain/configuration/action semantics;
  complete `cc_toolchain_info.bzl` freezing until a successor proof.

No request option is silently ignored: the two deferred flags remain rejected
outside the admitted CLI option surface. The accepted default has no mutable
request projection. Overlapping requests compute immutable key values from
their own existing DICE dependency graph; no historical filesystem snapshot or
cross-request mutable capture is introduced.

## Evidence, implementation successor and stops

This docs packet changes only this manifest, the canonical plan and the Stage
4 owner plan under 0 production/0 test/320 documentation additions. Run source
hash/line checks, targeted canonical/manifest agreement, structure/diff/archive
checks and independent reserved-architecture review. Add no fixture: pinned
Bazel source and the named upstream regression family already discriminate the
decision.

If accepted, roll one implementation packet limited to:

- new private `app/slug_loading_v2/src/bzl_visibility.rs`;
- `app/slug_loading_v2/src/{lib.rs,provider.rs,package.rs,bzl_module.rs}`;
- colocated tests plus existing `host_package_load_tests.rs`,
  `tests/bzl_invalidation.rs` and `tests/build_file_loading.rs` only as needed.

Credible ceilings are 500 production, 850 tests and 1,350 total additions.
Require pure parser/matcher and global positional-only ABI/return,
placement/cardinality/type tests;
direct/imported/inlined function negatives; implicit/public/private/list,
same/cross/subtree/repository-mapping checks; all five composition-site guards;
source and imported-policy A/B/A invalidation; exact rules_cc
`visibility(["//cc/..."])` evaluation at its real owner; focused/all loading,
24/31 integrations, locked analysis/core, CLI build, format/diff/archive and
independent ownership/representation review. Reuse upstream test themes with
pinned-source comments; create no oracle fixture.

STOP and `REPLAN` for a new key/lock/global registry, evaluator or command
cache; raw-source scanning; path-derived repository identity; retained heap
value/context borrow; policy omitted from semantic equality; validation after
importer execution; any unguarded live composition site; starlark-rust change;
ignored option flag; copied Zabel behavior; source/hash mismatch; public API or
cross-crate change; allowlist/cap escape; or inability to prove top-level call
shape. Stop after design acceptance and roll the implementation successor.

## Immediate predecessor

Commit `879d879f5` accepts only complete `link.bzl` defining-module freezing.
It does not accept `.bzl` load visibility, `cc_toolchain_info.bzl`, function
invocation, configured linking, toolchains or actions.
