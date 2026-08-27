# Slug V2 Clean-Restart Implementation Plan

## Canonical Status

This is the canonical Slug implementation plan after the V1 archive decision.
The January roadmap and numbered V1 subplans remain valuable reference material,
but new implementation work should start from this plan and the V2 subplans
under [slug-v2-subplans](./slug-v2-subplans/).

Slug keeps its name and repository. The archived V1 implementation is a Buck2
fork migrated toward Bazel compatibility. V2 keeps the proven lessons and
selected code from V1, but the active trunk is a Bazel-shaped Rust
implementation from the first architectural boundary.

## Live Status

This table is the scheduling authority for a clean root agent. Historical
checkpoint sections below are evidence only. A request such as
`/goal follow the implementation plan` uses the repo orchestration skill and
advances the **Current packet**, not an older `next` paragraph. The compact
[current-packet manifest](./slug-v2-subplans/current-packet.md) supplies startup details
and must name the same packet.

| Milestone | Status | Accepted evidence | Blocking gap | Current or next packet |
|-----------|--------|-------------------|--------------|------------------------|
| M0: archive and baseline health | **accepted** | both archive refs peel to `e218054d…`; clean-root checker green in `9897e940` | none | preserve the refs and checker gate |
| M1: one semantic spine | **accepted** | Host loading observations, typed command/event ownership, direct local-override external query loading, the query-only unsupported-cycle boundary in `ea2019f8`, direct-local exported-source build completion in `42f4a64b`, the first private core repository source-observation consumer in `53152727`, the pinned in-flight loading/source-lock oracle in `2ffad088`, the private request-revision/source-certificate vertical in `207fe438`, the sole-root native publication bridge in `f0849151`, the exact callerless observed-path/Host-file frontier in `308b409a`, the accepted observed root REPO, repository-ignore, package-marker, root-module, anchor, root-package source, and recursive Host `.bzl` frontiers in `f2c7305f`, `43adf74b`, `0875728b`, `2640d1c0`, `c6e61d60`, `2225cf99`, and `b9fda97d`; Host-glob listing/boundary, segment, traversal, complete root-package loading, singleton root-package-all publication, observed configured analysis, neutral singleton-root `Single`, public cquery publication, observed external repository routing, routed Host path/source, routed REPO/ignore policy, external package-marker lookup, and direct-local MODULE file, inspection, include-package horizon, recursive preparation, evaluation, repository-package source, external-Bzl evaluation, repository-package-load, loading-query publication, epoch-shaped source-certificate acceptance, external singleton build and root-only multi-build publication are accepted through `bd4fb8db`, `dc6f6e02`, `2bccb48e`, `daf5eef9`, `31a8b1d3`, `69d37ddb`, `941db0d0`, `03f2db3e`, `e4555dca`, `e4ee0a8e`, `2a8dd968`, `33717f27`, `99d78875`, `a61de5d4`, `79248832`, `cc34e31d`, `1815c019`, `ac7b8bdf`, `93f43264`, `a9270586`, `2e1c1334`, `3f1d4dd4`, and `51127df8`; the exit audit proves shared typed roots, overlapping isolation/cancellation, bounded revision retry, exact final certificate reobservation, atomic materializer/path/repository/event publication, lifecycle restoration, warm nonreplay, compact retained state, and deferred snapshot adapters | none | preserve the accepted semantic spine |
| M2: analysis graph | **accepted (Slug-native semantic identity)** | recursive configured analysis, bounded root cquery in `135b0567`, transitions, toolchain context, recursive action closure, and the reviewed complete Rust-native default structural vertical | exact Bazel configuration/output bytes remain deferred to M9; exact ActionKey projections are admitted just in time per action family | preserve structural identity and the distinct digest/projection domains |
| M3: `query` | **accepted** | all 16 default functions; default/explicit `label`, graph, `label_kind`, and `package` output; the 18-lane/165-pair Bazel 9.2 `attr()` oracle in `4ea8f6c7`; complete retained descriptors in `83fe6037`; and runtime activation in `ed38f82a` | Sky Query-only functions and non-text formats remain later breadth, not M3 gates | preserve the accepted loading-query graph |
| M4: `cquery` | **accepted** | the same provider/action/edge-bearing configured analysis result, full structural/null Target/Exec identity, transitions, toolchain/delegation topology, forward/reverse graph semantics, admitted formatters, Need/error ordering, and one-shot/daemon recovery | none; remaining expression and topology shapes are later breadth | preserve the accepted configured-query graph |
| M5: `aquery` | **accepted (bounded FileWrite; Slug-native identity/order)** | recursive action ownership, complete structural configuration identity, closure-resolved toolchain-backed FileWrite semantics, exact literal owner order/framing, bounded aspect-free `deps()` owner membership, stable-daemon A/B/A restoration, and sole-candidate selected-implementation action platforms | broader action kinds, expressions, formats, ordinary zero-toolchain owners, multi-platform choice, and the exact FileWrite ActionKey projection remain later breadth | preserve the admitted FileWrite boundary; pair each newly admitted action family with its exact projection |
| M6: execution and caching | **accepted (bounded FileWrite)** | the resolved semantic view is the sole FileWrite executor input; canonical inline Directory/Command/Action SHA-256 identity, selected-platform properties, raw-path rejection, one-shot and stable-daemon A/B/A, and zero direct-local actions are accepted | broader actions, input trees, backends, cache/materializer policy, and transport breadth remain later Stage 7 work | preserve the accepted FileWrite handoff |
| M7: command/ruleset breadth | **partial; complete LTO context proof selected** | selected-registry source/root-load and exact rules_rust root realization are accepted through `2f373248`; commits through `68e458b4` accept bounded keyword-only syntax, exact live `.bzl` `struct` placement/operations, provider/rule docs and typed string/bool/list definitions with structural repeatability; `840d28e7` accepts the first fixed aspect-definition subset; `84ddb6a3` accepts bounded `.bzl` `Label` construction; `eda81a4d`, `61cb0ad0`, and `129ff448` accept the rust-analyzer rule closure; `2cbdb148`, `d4d4d6dc`, `275e0b24`, `50205fb3`, and `88304c2f` accept lint/rustfmt declarations; commits through `4b2396f0a` accept bounded loading breadth through direct-provider and ObjcInfo proxy children plus exact empty `depset()`; `07077e23d`, `badf5844a`, and `9b44f0352` complete private CcInfo, launcher info, and shared-library hint info | compilation outputs first reaches incomplete 97-line LTO context; toolchain config remains broad | run only `WP-4-7A-rules-cc-private-lto-compilation-context-complete-loading-proof`; preserve M7A -> M8 -> M7B |
| M8: bootstrap | **developer graph accepted; parked behind M7A only** | exact 33-package CLI boundary plus accepted Gates A-B; the 43-test BuildBuddy developer gate is `PROVED_CACHE_ONLY` and `PROVED_RBE` with clean lifecycle; CI explicitly not admitted | the bootstrap closure still needs its repository sources, rules_rust/provider/toolchain semantics, action kinds/input trees, normalized aquery, and REAPI execution/materialization; accepted bounded M2/M5/M6 are no longer the named blocker | begin Stage 10.3/10.4 as soon as the bootstrap-critical M7A closure is accepted; do not wait for run/test/BEP or unrelated public-ruleset breadth |
| M9: exact Bazel configuration/output identity bytes | deferred | four-domain C0/C1/P0/P1/content/path evidence in `f00e99db` | in-depth Rust-only analysis and reproduction of Bazel configuration checksum and output-directory identity; only residual unadmitted ActionKey families remain here | begin only after the functional semantic graph/bootstrap path |

### Current packet

[WP-4-7A-rules-cc-private-lto-compilation-context-complete-loading-proof](./slug-v2-subplans/current-packet.md).

Prove exact complete private `lto_compilation_context.bzl` loads the accepted
helper/internal children, freezes its provider and lazy-function rows, and
constructs its exact empty context. Add no production or lazy invocation.

### M7 complete shared-library hint accepted; LTO context selected (2026-08-26)

Commit `9b44f0352` adds 88 proof lines and no production. It byte-verifies all 56
dependency-free shared-library-hint lines and proves the exact public provider
identity without invocation. All 245 loading-library, 24 invalidation and 31
BUILD-loading tests, locked checks, CLI build and hygiene pass. Independent
review accepts caps and boundaries.

Private `cc_common` next reaches compilation outputs, whose first incomplete
child is 97-line `lto_compilation_context.bzl` (`a17435cd…`). Its helper and
internal children are complete; its eager surface is two provider declarations
and one empty context. Run only
`WP-4-7A-rules-cc-private-lto-compilation-context-complete-loading-proof` under
0/220/220 caps. Clean `../zabel` `0795445f…` guides defining-module/recursive
freeze ownership only; Bazel 9.2 and authenticated rules_cc remain exact
authority.

### M7 complete launcher info accepted; shared-library hint selected (2026-08-26)

Commit `badf5844a` adds 80 proof lines and no production. It byte-verifies all
31 launcher-info lines, rebuilds the complete helper closure, retains the loaded
wrapper identity, and proves initialized provider/raw/private-constructor
identities and types without invocation. All 244 loading-library, 24
invalidation and 31 BUILD-loading tests, locked checks, CLI build and hygiene
pass. Independent review accepts caps and boundaries.

Private `cc_common` source order next reaches dependency-free 56-line
`cc_shared_library_hint_info.bzl` (`7d067aad…`), whose only evaluated row is the
public two-field provider declaration. Run only
`WP-4-7A-rules-cc-private-cc-shared-library-hint-info-complete-loading-proof`
under 0/100/100 caps. Clean `../zabel` `0795445f…` guides defining-module
ownership/freeze only; Bazel 9.2 and authenticated rules_cc remain exact
authority.

### M7 complete private CcInfo accepted; launcher info selected (2026-08-26)

Commit `07077e23d` adds 892 proof lines and no production. It byte-verifies all
656 private CcInfo lines, rebuilds the complete four-child closure, retains every
imported identity, and proves six provider identities plus all eager empty-context
shapes without lazy invocation. All 243 loading-library, 24 invalidation and 31
BUILD-loading tests, locked checks, CLI build and hygiene pass. Independent
review accepts caps and compatibility boundaries.

The generated proxy still reaches private `cc_common` before toolchain config.
The helper, private CcInfo and `cc_internal` children are complete; its first
incomplete child is dependency-light 31-line `cc_launcher_info.bzl`
(`41da5476…`), which loads only the accepted helper and declares one initialized
provider/raw pair. Run only
`WP-4-7A-rules-cc-private-cc-launcher-info-complete-loading-proof` under
0/120/120 caps. Clean `../zabel` `0795445f…` guides defining-module identity and
recursive freeze only; Bazel 9.2 and authenticated rules_cc remain exact
authority.

### M7 complete extra-link library accepted; private CcInfo proof selected (2026-08-26)

Commit `30ec1de4f` adds 316 proof lines and no production. It hashes/freezes all
192 extra-link-library lines, rebuilds the exact helper/internal closure, proves
both imports, four distinct providers/private visibility, and exact `_EMPTY`
identity/list. All 242 loading library, 24 invalidation and 31 BUILD-loading
tests, locked checks and CLI build pass; independent review accepts boundaries.

All four children of 656-line private `cc_info.bzl` (`4424bb87…`) are complete.
Its remaining eager surface is six provider declarations, three empty contexts,
zero-argument depsets, one admitted header-info projection and the initialized
CcInfo pair; all other bodies are lazy. Run only
`WP-4-7A-rules-cc-private-cc-info-complete-loading-proof` under 0/900/900 caps.
Clean `../zabel` `0795445f…` guides defining-module/recursive freeze ownership;
Bazel 9.2 and authenticated source remain exact authority.

### M7 zero-argument depset accepted; exact ObjcInfo selected (2026-08-26)

Commit `498e5efc7` adds 9 production and 50 proof lines. Zero/no-name `depset()`
reuses the existing empty frozen representation in BUILD and `.bzl`; one-list
validation/order remains unchanged; named zero-position, wrong-type and excess
positional calls fail closed. All 237 loading-library tests, 24 invalidation
tests, 31 BUILD-loading tests, analysis/core checks and the CLI build pass. Two
reviewers accept caps, placement and the allocation-free arity branch.

The exact 97-line ObjcInfo child is now source-complete and freezeable. Run only
`WP-4-7A-rules-cc-compatibility-proxy-objc-info-loading-proof` under 0/220/220
caps. Prove the initializer/raw bindings remain private functions, public
ObjcInfo remains a distinct provider callable, and both exact proxy exports
pointer-alias only that public callable. Invoke nothing; keep the complete
proxy/public CcInfo route deferred. Clean `../zabel` `0795445f…` guides only
defining-module ownership and reexport reachability; Bazel 9.2 and authenticated
rules sources remain exact authority.

### M7 direct-provider proxy children accepted; zero-argument depset selected (2026-08-26)

Commit `0699dffe7` adds 158 proof lines and accepts exact complete
`CcSharedLibraryInfo` and `DebugPackageInfo` modules, provider-callable types,
the actual rules_cc repository mapping and pointer-identical narrowed proxy
reexports. Focused proof, all 236 loading-library tests, 24 invalidation tests,
31 BUILD-loading tests, analysis/core checks and the CLI build pass. Independent
review accepts source/range hashes, caps and exact/Slug-native/deferred claims.

The next 97-line dependency-free `ObjcInfo` child is not yet freezeable:
defining `_objcinfo_init` eagerly evaluates five `depset()` default expressions,
and Slug's current loading callable requires one positional list. A provider-only
slice would not prove the complete child. Run only
`WP-4-7A-bazel-zero-argument-depset-loading` under 20/50/70 caps, reusing the
existing empty frozen representation, rejecting names on the zero-positional
branch and preserving one-list behavior. Pinned
Bazel's default-`None` signature and `testEmptyGenericType` are exact authority.
Clean `../zabel` `0795445f…` guides only reuse of the existing empty ownership
shape; copy no Zig representation, caching, order or behavior. Schedule exact
ObjcInfo plus its two public proxy aliases only after this prerequisite.

### M7 public CcInfo route audit selects direct-provider proxy children (2026-08-26)

Audit `242325974` confirms that accepted initialized-provider commits prove the
`CcInfo` declaration abstraction but not the exact eager public route. The
18-line public module loads generated 15-line `symbols.bzl`, which eagerly loads
six children. Full exact route parity therefore cannot use a single-symbol
stub.

The proxy children are: private `cc_common.bzl` (788 lines, `5e6ab737…`),
`cc_info.bzl` (656, `4424bb87…`), dependency-free
`cc_shared_library_info.bzl` (27, `5b7dcd1f…`) and
`debug_package_info.bzl` (26, `b22666c6…`), initialized `objc_info.bzl` (97,
`675fffb0…`), and toolchain-config info (143, `8c522773…`) with further loads.
The private CcInfo producer likewise retains four children plus eager contexts;
its accepted source-shaped declaration is insufficient for full-module parity.

Select the two complete dependency-free direct-provider children as one
coherent bounded prerequisite under 0/160/160 caps. Exact claims cover their
full bytes, producers, provider types/identities and pointer-preserving proxy
reexports. The narrowed proxy composition is Slug-native; all omitted loads and
the complete proxy/public CcInfo route remain deferred. Architecture review
accepts this classification. Clean `../zabel` `0795445f…` guides only provider
definition/reexport reachability; Bazel 9.2 and authenticated sources remain
exact authority.

### M7 exact utils compute-crate-name accepted; public CcInfo route audit selected (2026-08-26)

Commit `7d45bee02` embeds and hash-verifies the five new exact crate-name slices,
reuses the three accepted eager encoding slices in authenticated source order,
and proves exact public/private visibility, retained eager pointer identities
and actual parent import identity without invocation.

Focused proof, all 235 loading-library tests, 24 invalidation tests, 31 BUILD
loading tests, analysis/core checks and the CLI build pass. Independent review
accepts exact bytes/order, visibility, identities, nonexecution, 230/240 proof
scope, and Zabel's guidance-only role.

The only residual utils exports are `transform_link_deps` and `transform_deps`.
Both require exact `CcInfo` through public `cc/common:cc_info.bzl`, generated
compatibility `symbols.bzl`, and private `cc_info.bzl`. Commits `9c51999f9` and
`152caa6fe` accept the provider initializer and source-shaped `CcInfo`
declaration, but do not by themselves prove the full proxy/private loaded route.
Run only `WP-4-7A-rules-cc-cc-info-public-route-frontier-audit`; admit no stub,
implementation or parity widening. Pinned Bazel and authenticated sources are
exact authority; clean `../zabel` `0795445f…` guides only recursive loaded-value
reachability and freeze ownership.

### M7 exact utils transform-sources export accepted; crate-name selected (2026-08-26)

Commit `4d037e48d` hash-verifies and freezes exact `utils.bzl:878-917` and
private helper 937-965 with the accepted exact Skylib paths child. It proves the
actual apparent-to-canonical repository mapping, loaded paths identity, private
visibility and parent identity without invocation or action/path behavior.

Focused proof, all 234 loading-library tests, 24 invalidation tests, 31 BUILD
loading tests, analysis/core checks and the CLI build pass. Independent review
accepts exact bytes, mapping, identities, nonexecution, 152/180 proof scope,
and Zabel's guidance-only role.

Select exact `compute_crate_name` and its four dependency helpers: 104 new source
lines total, reusing accepted exact `_substitutions`, `_encode_raw_string` and
`_replace_all` slices in authenticated source order. Run only
`WP-4-7A-rules-rust-utils-compute-crate-name-export-loading-proof` under
0/240/240 caps. Prove exact hashes, function types, private visibility, retained
accepted eager identities and actual parent import; invoke nothing. The two
dependency transforms remain deferred on the exact CcInfo proxy/private
closure. Pinned Bazel remains authority; clean `../zabel` `0795445f…` guides
only closure reachability and freeze ownership.

### M7 exact utils output-diagnostics export accepted; transform-sources selected (2026-08-26)

Commit `53c4d7d78` embeds and hash-verifies exact
`providers.bzl:120-128` and `utils.bzl:967-991`, then freezes the narrowed
provider -> utils -> parent chain under exact producer and load spellings. The
proof establishes `provider_callable`/function types plus loaded and public
pointer identities without invocation.

Focused proof, all 233 loading-library tests, 24 invalidation tests, 31 BUILD
loading tests, analysis/core checks and the CLI build pass. Independent review
accepts exact bytes, owners, identities, nonexecution, 109/120 proof scope, and
Zabel's guidance-only role.

Of the four residual exports authenticated by audit `6381223ce`, select the
smallest bounded closure: exact `utils.bzl:878-917` `transform_sources`
(`1006a8da…`) and private helper `utils.bzl:937-965` (`c5105f74…`), reusing the
accepted exact 320-line Skylib `paths.bzl` child (`96cce438…`). Use the exact
apparent `@bazel_skylib//lib:paths.bzl` load under the rules_rust mapping and an
actual parent `:utils.bzl` import under 0/180/180 caps. Prove types, private
visibility and loaded/public pointer identities; invoke nothing. Pinned Bazel
remains exact authority; clean `../zabel` `0795445f…` guides only loaded-binding
reachability and freeze ownership.

### M7 exact utils can-build-metadata export accepted; diagnostics selected (2026-08-26)

Commit `cf76c0443` embeds and hash-verifies exact
`providers.bzl:109-118` and `utils.bzl:742-765`, reuses accepted exact
`can_use_metadata_for_pipelining` in source order, and freezes the narrowed
provider -> utils -> parent chain under exact producer and load spellings. The
proof establishes `provider_callable`/function types plus loaded and public
pointer identities without invoking either function or the provider.

Focused proof, all 232 loading-library tests, 24 invalidation tests, 31 BUILD
loading tests, analysis/core checks and the CLI build pass. Independent review
accepts exact bytes, owners, identities, nonexecution, 115/120 proof scope, and
Zabel's guidance-only role.

Select the sole remaining minimum closure from audit `6381223ce`: exact
`utils.bzl:967-991` `generate_output_diagnostics` (SHA-256 `8535acbf…`) plus
exact `providers.bzl:120-128` `RustcOutputDiagnosticsInfo` (SHA-256
`a066585f…`). Use only a narrowed provider child, narrowed utils load and actual
parent `:utils.bzl` import under 0/120/120 caps. Prove types and pointer
identities; invoke neither declaration and admit no diagnostic/action behavior.
Pinned Bazel remains exact authority; clean `../zabel` `0795445f…` continues
to guide only loaded-binding reachability and freeze ownership.

### M7 post-private-helper audit selects can-build-metadata export (2026-08-26)

Audit `f3ddca46a` authenticates all six remaining utils closures. Exact roots
hash as follows: `compute_crate_name` `8b79565b…`, `transform_deps`
`6983d42f…`, `transform_link_deps` `c6b644e8…`, `can_build_metadata`
`4d57fbea…`, `transform_sources` `1006a8da…`, and
`generate_output_diagnostics` `8535acbf…`.

`compute_crate_name` reaches 104 new helper lines plus accepted eager encoding
slices. `transform_sources` reaches 69 new local lines plus the accepted exact
320-line Skylib paths child. Both transform-dependency functions reach exact
rules_rust provider declarations but remain deferred because `CcInfo` crosses
the generated compatibility proxy and broad private initialized-provider
closure; no stub is admitted.

The two minimum new-source closures are 34 lines each. Select the earlier
parent import: exact `utils.bzl:742-765` `can_build_metadata` plus exact
`providers.bzl:109-118` `AlwaysEnableMetadataOutputGroupsInfo`, reusing accepted
exact `can_use_metadata_for_pipelining`. Run only
`WP-4-7A-rules-rust-utils-can-build-metadata-export-loading-proof` under
0/120/120 caps in the existing proof owner. Use proof-only narrowed actual
`:providers.bzl` and `:utils.bzl` loads; prove types and pointer identities and
invoke neither function nor the declared provider.

Exact compatibility covers source bytes/hashes, producers, symbol/load
spelling, provider/function types and imported identities. Narrowed proof
modules and frozen representation are Slug-native. Results, diagnostics,
configured behavior, complete provider/utils/parent loads and the other five
exports remain deferred. Pinned Bazel 9.2 is sole behavior authority; clean
`../zabel` `0795445f…` guides only loaded-binding reachability and ownership.

### M7 exact utils crate-root export accepted; loaded frontier audit selected (2026-08-26)

Commit `cdd2f68f7` freezes exact `utils.bzl:788-816` plus `:818-833` in source
order, verifies the helper is a hidden function, and proves pointer-identical
public import through the proof-only exact parent using actual `:utils.bzl`
spelling. Neither function is invoked. The +107 proof/0 production change ends
at 8,858 below the 8,881 ceiling; focused proof, 231 loading units, 24
invalidation tests, 31 BUILD-loading tests, dependent checks, CLI build and
hygiene pass. Independent review returned `ACCEPT`.

Six parent imports remain: `can_build_metadata`, `compute_crate_name`,
`generate_output_diagnostics`, `transform_deps`, `transform_link_deps`, and
`transform_sources`. Every one now crosses at least one loaded provider,
accepted eager composite, bazel_skylib path binding or the large crate-name
helper closure, so no further implementation packet is selected from a name-
only inventory.

Run only `WP-4-7A-post-utils-private-helper-loaded-frontier-audit`. Authenticate
each exact local slice plus every required loaded or same-module binding,
identify which dependencies are already accepted versus still missing, and
select exactly one smallest coherent compile/freeze/import proof with explicit
line/hash facts, compatibility class, allowlist, caps, proof and STOPs. Edit
only the canonical plan, Stage 4 subplan and current-packet manifest; add no
Rust, fixture or oracle evidence.

Pinned Bazel 9.2 resolver tests and authenticated rules_rust sources remain
sole exact authority. Clean `../zabel` `0795445f…` guides only recursively
reachable defining-module and loaded-binding retention; no Zig code,
representation, traversal/order algorithm, diagnostic, identity or behavior is
copied.

### M7 exact utils expand-dict export accepted; crate-root export selected (2026-08-26)

Commit `216b83ac0` freezes exact `utils.bzl:268-313` plus `:315-348`, verifies
the private helper is a hidden function, and proves pointer-identical public
import through a proof-only exact parent using actual `:utils.bzl` spelling.
Neither function is invoked. The +145 proof/0 production change ends at 8,751
below the 8,786 ceiling; focused proof, 230 loading units, 24 invalidation
tests, 31 BUILD-loading tests, dependent checks, CLI build and hygiene pass.
Independent correction review added retained private-visibility evidence and
returned `ACCEPT`.

Seven dependency-bearing imports remain. The smallest source-complete closure
without a loaded provider, accepted eager composite or bazel_skylib binding is
public `crate_root_src` at exact `utils.bzl:788-816` plus private helper
`_shortest_src_with_basename` at `:818-833`. The 29- and 16-line slices hash to
`f5a21bb9…` and `7157302d…` and total 45 lines.

Run only `WP-4-7A-rules-rust-utils-crate-root-export-loading-proof` in the
existing proof owner under 0/130/130 caps. Freeze the two exact slices in source
order under the utils producer, prove the public/private function and visibility
boundary, and import only `crate_root_src` with actual `:utils.bzl` spelling
through the proof-only exact parent. Prove pointer identity and invoke neither
function.

Exact compatibility covers both source bytes/hashes, defining producer, actual
load spelling, function types, private visibility/helper reachability and
public import identity. Proof-only concatenation/parent and starlark-rust
frozen representation are Slug-native. Every result/diagnostic, configured
behavior, the other six dependency-bearing exports, whole-utils freeze and
parent body remain deferred.

Pinned Bazel resolver tests remain exact authority. Clean `../zabel`
`0795445f…` guides only reachable defining-module helper retention; no Zig
code, representation, traversal/order algorithm, diagnostic, identity or
behavior is copied.

### M7 exact utils leaf exports accepted; expand-dict export selected (2026-08-26)

Commit `13ebf0a14` freezes the six remaining helper-free functions imported by
exact `rust.bzl` and proves their real parent-relative order and pointer-
identical bindings through a proof-only exact parent using actual `:utils.bzl`
spelling. All six functions remain uninvoked. The +191 proof/0 production
change ends at 8,606 below the 8,665 ceiling; focused proof, 229 loading units,
24 invalidation tests, 31 BUILD-loading tests, dependent checks, CLI build and
hygiene pass. Independent review returned `ACCEPT`.

Eight dependency-bearing parent imports remain. The earliest source-complete
closure is private helper `_expand_location_for_build_script_runner` at exact
`utils.bzl:268-313` plus public `expand_dict_value_locations` at `:315-348`.
The 46- and 34-line slices hash to `73cd67a0…` and `0c8ce893…`; the public body
captures only that helper, while the helper body references predeclared values
and methods. No loaded binding or eager composite enters this closure.

Run only `WP-4-7A-rules-rust-utils-expand-dict-export-loading-proof` in the
existing test owner under 0/180/180 caps. Freeze the two exact slices under the
utils producer, prove both are functions, and import only the public function
with actual `:utils.bzl` spelling through the proof-only exact parent. Prove
pointer identity and invoke neither function.

Exact compatibility covers both source bytes/hashes, defining producer, actual
load spelling, function types, private-helper reachability at freeze and public
import identity. Proof-only concatenation/parent and starlark-rust frozen
representation are Slug-native. Every result/diagnostic, configured behavior,
the other seven dependency-bearing exports, whole-utils freeze and parent body
remain deferred.

Pinned Bazel resolver tests remain exact authority. Clean `../zabel`
`0795445f…` guides only reachable defining-module helper retention; no Zig
code, representation, traversal/order algorithm, diagnostic, identity or
behavior is copied.

### M7 exact utils find-toolchain export accepted; leaf family selected (2026-08-26)

Commit `d3cb959f6` freezes exact rules_rust `utils.bzl:61-70`
`find_toolchain` under the utils producer and proves pointer-identical import
through a proof-only exact-parent module using actual `:utils.bzl` spelling.
The function and its `Label` body remain uninvoked. The +53 proof/0 production
change ends at 8,415 below the 8,482 ceiling; focused proof, 228 loading units,
24 invalidation tests, 31 BUILD-loading tests, dependent checks, CLI build and
hygiene pass. Independent review returned `ACCEPT`.

The accepted closure audit leaves six other helper-free parent-needed functions:
`determine_output_hash`, `deduplicate`, `dedent`,
`can_use_metadata_for_pipelining`, `determine_lib_name`, and `get_edition`.
Their six separately authenticated slices total 128 lines and reference only
predeclared builtins, comprehensions, field access or standard value methods.

Run only `WP-4-7A-rules-rust-utils-leaf-exports-loading-proof` in the existing
test owner under 0/250/250 caps. Freeze the six exact slices together under the
utils producer, then import them with actual `:utils.bzl` spelling and their
real parent-relative order in a proof-only exact-parent module. Prove function
types and pointer identities; invoke none.

Exact compatibility covers slice bytes/hashes, producers, load spelling/order,
function types and imported identities. Proof-only concatenation/parent and the
Rust frozen representation are Slug-native. Every result/diagnostic, configured
behavior, the eight dependency-bearing exports, whole-utils freeze and parent
body remain deferred.

Pinned Bazel resolver tests remain exact authority. Clean `../zabel`
`0795445f…` guides only reachable defining-module function retention; no Zig
code, representation, algorithm, diagnostic, identity or behavior is copied.

### M7 post-utils audit selects exact find-toolchain export proof (2026-08-26)

Audit `d4e264cdc` maps all fifteen functions imported by exact parent
`rust.bzl:40-57` to their source-complete compiler/freeze closures. Seven are
leaf functions over predeclared globals or field/string operations; the others
require same-module helpers, accepted eager composites, loaded providers or
bazel_skylib paths. None is treated as invoked or configured behavior.

The earliest parent-needed definition is `utils.bzl:61-70`
`find_toolchain`, whose ten exact lines hash to
`75fe3e764290fcfcec78cc25d25b4d2486708dafabb112f5d1e44b8e21081be1`.
Its body resolves only the already-admitted `Label` predeclared global. Run only
`WP-4-7A-rules-rust-find-toolchain-export-loading-proof` in the existing test
owner under 0 production, 120 proof and 120 total addition caps. Freeze the
exact slice under the utils producer, then import it with actual `:utils.bzl`
spelling in a proof-only exact-parent consumer and prove pointer identity. Call
neither `find_toolchain` nor `Label`.

Exact compatibility covers the slice bytes/hash, child and parent producers,
actual relative load spelling, frozen function type and imported identity. The
proof-only parent and Rust frozen representation are Slug-native. Function
invocation/result/diagnostics, configured toolchain lookup, the other exports,
whole utils freeze and parent body remain deferred.

Pinned Bazel 9.2 resolver tests authenticate global and loaded-closure binding.
Clean `../zabel` `0795445f…` guides only retaining reachable defining-module
functions after closure; no Zig code, representation, algorithm, diagnostic,
identity or behavior is copied.

### M7 exact rules_rust utils eager values accepted; export audit selected (2026-08-26)

Commit `adde01290` embeds five separately hashed, unabridged rules_rust 0.73.0
`rust/private/utils.bzl` slices totaling 124 upstream lines. Under exact producer
`@@rules_rust+//rust/private:utils.bzl`, it freezes the ordered six unsupported
features, false C++ kill switch, all 63 ordered encoding substitutions and both
public aliases. The encode alias retains exact frozen function identity.

Lines 692-740 are present only because `_encode_raw_string` resolves
`_replace_all` while compiling/freezing; neither function nor any other utility
is invoked. Exact compatibility covers the five source-slice bytes and eager
values/aliases. Proof-only private projection and Rust frozen representation are
Slug-native. Whole-module freeze, utility results/diagnostics, configured
toolchain/allocator behavior and parent source remain deferred.

The change is +202 proof and 0 production lines, ending at 8,362 below the
8,410 ceiling. One focused proof, all 227 loading units, 24 invalidation tests,
31 BUILD-loading tests, direct-dependent checks, CLI build and hygiene pass.
Independent review returned `ACCEPT` after verifying exact hashes, order,
identity, non-invocation, caps and authority boundaries.

Run only docs audit `WP-4-7A-post-utils-eager-values-parent-import-frontier-audit`.
The authenticated 1,821-line `rust.bzl` imports fifteen named functions from
utils at lines 40-57; the accepted eager proof does not establish those exports
or authorize returning to the parent. Inventory their transitive compiler/
freeze closure and select one bounded exact-source proof or `REPLAN`.

Clean `../zabel` `0795445f…` guided only recursive defining-module reachability
for composites, aliases and functions. No Zig code, representation, ordering,
diagnostic, identity or behavior was copied. Bazel 9.2 remains sole authority.

### M7 post-find-toolchain audit selects bounded utils eager-values proof (2026-08-26)

After exact `cc/find_cc_toolchain.bzl` returns, authenticated rules_rust 0.73.0
`rust/private/utils.bzl` resumes through already-admitted rules_cc `cc_common`,
rules_cc `CcInfo` and rules_rust providers. Its full 1,032-line source hashes to
`8aa49b9312d4ae5c4aed033aba65392a039a681b3ee21ca83da0f05acac28ace`.
No unsupported eager expression remains.

The complete eager body is six families: ordered `UNSUPPORTED_FEATURES`, the
private false kill switch, a 31-pair encoding tuple, the ordered 63-pair nested-
comprehension substitution list, its public alias, and the public alias of lazy
`_encode_raw_string`. All other top-level declarations are lazy functions.
Pinned Bazel Starlark loop/comprehension tests authenticate tuple destructuring,
nested clause order and list result order.

Run only proof packet `WP-4-7A-rules-rust-utils-eager-values-loading-proof`,
changing the existing loading test file under 0 production, 250 proof and 250
total addition caps. Embed and hash only exact source lines 32-42, 73, 601-650,
664-676 and 692-740; do not copy the full module. The fifth slice closes the
lazy function's compiler/freeze dependency on `_replace_all` but does not admit
its behavior. Prove exact ordered strings and all 63 derived pairs, false kill-
switch capture, both alias identities and frozen function type. Invoke no
utility and stop before later source.

Exact compatibility is the five source-slice bytes and the ordered eager
values/aliases under the exact producer. The proof-only private projections and
frozen Rust representation are Slug-native. `_replace_all` invocation/results,
whole-file source freeze, every utility result/diagnostic, configured toolchain/
allocator behavior and later parent source remain unsupported/deferred.

Clean `../zabel` `0795445f…` guides only freezing all values reachable from
exported composites and aliases after evaluator closure. No Zig code,
representation, owner pointer, ordering algorithm, diagnostic, identity or
behavior is copied. Bazel 9.2 remains sole authority; no retained utility or
ledger change is selected.

### M7 exact rules_cc find-toolchain child accepted; utils audit selected (2026-08-26)

Commit `ee9ef5254` freezes exact unabridged rules_cc 0.2.17
`cc/find_cc_toolchain.bzl` at SHA-256
`3f62d3ea99f59674f71dbc669c80dd0dc5ef14637933d727b74f0bd556334655`
under producer `@@rules_cc+//cc:find_cc_toolchain.bzl` and exact cached child
`@@rules_cc+//cc/common:cc_common.bzl`. Five source-defined exports retain their
dict/Label/function types. The canonical C++ toolchain Label and singleton
`_cc_toolchain` Label/default survive a proof-only consumer; no helper or rule
implementation runs.

All 226 loading units, 24 invalidation tests, 31 BUILD-loading tests, dependent
core checks, rebuilt CLI and hygiene pass. The packet adds 225 proof-only lines,
ending at 8,160 below the 8,235 ceiling. Independent review caught and corrected
the child package/target identity, then returned `ACCEPT`.

Run only docs packet `WP-4-7A-post-find-cc-toolchain-utils-frontier-audit`.
Return to exact 1,032-line `rust/private/utils.bzl` after the child freezes,
account for its cached `cc_common`, `CcInfo` and providers loads, then classify
the remaining eager module body and select one bounded proof/implementation or
`REPLAN`. Do not edit Rust or invoke utility/toolchain functions.

Exact compatibility covers source freeze, producer/child identities, five
source-defined export types and canonical eager label/declaration facts. Frozen
Rust representation and the proof-only consumer are Slug-native. Helper
execution, configured/legacy C++ toolchain lookup, exact display text and later
utils/allocator/parent bodies remain unsupported/deferred.

Clean `../zabel` `0795445f…` guided only frozen reachability of exported closures
and the nested declaration dictionary. No Zig code, representation, owner
pointer, ordering, capture algorithm, diagnostic, identity or behavior was
copied. Bazel 9.2 remains sole authority; no retained utility or ledger changed.

### M7 post-paths audit selects exact rules_cc find-toolchain proof (2026-08-26)

Exact `rust/private/rust.bzl` resumes after paths through already-admitted
bazel_skylib `common_settings.bzl`, rules_cc `cc_info.bzl`, and rules_rust
`common.bzl`/`providers.bzl`. The first new direct child is the 302-line
`rust_allocator_libraries.bzl`, SHA-256
`ae4acb50ac6a1b922254a07346d97b4649810d33836f2be4824fd0b7a81e536e`.
Its cached rules_cc children return before it enters the previously unseen
1,032-line `utils.bzl`, SHA-256
`8aa49b9312d4ae5c4aed033aba65392a039a681b3ee21ca83da0f05acac28ace`.

After cached bazel_skylib paths, utils first reaches rules_cc 0.2.17
`cc/find_cc_toolchain.bzl`, 131 lines at SHA-256
`3f62d3ea99f59674f71dbc669c80dd0dc5ef14637933d727b74f0bd556334655`.
Its sole `cc_common` child is admitted. Its eager body creates canonical
`CC_TOOLCHAIN_TYPE`, singleton label descriptor map `CC_TOOLCHAIN_ATTRS`, and
three lazy functions using already-admitted loading shapes.

Run only proof packet `WP-4-7A-rules-cc-find-cc-toolchain-loading-proof` in the
existing loading test file under 0 production, 300 proof and 300 total addition
caps. Embed the exact source, verify its hash and producer/child identities,
prove the exact source-defined export/type set, canonical toolchain label and
singleton label attribute/default through a proof-only consumer. Invoke no helper and stop
when this child returns.

Exact compatibility is exact-source freeze, canonical producer/load identities,
source-defined export/type set and the eager label/declaration constants.
Existing frozen Rust representation and the proof consumer are Slug-native. Function
execution, configured toolchain lookup, exact display text and later utils/
allocator/parent bodies remain unsupported/deferred.

Clean `../zabel` `0795445f…` guides only module-freeze reachability for exported
closures and a nested declaration dictionary. No Zig code, representation,
owner pointer, field ordering, capture algorithm, diagnostic, identity or
behavior is copied. Bazel 9.2 remains sole behavior authority; no retained
utility or ledger change is selected.

### M7 exact bazel_skylib paths child accepted; parent audit selected (2026-08-26)

Commit `8440742f7` freezes the exact unabridged 320-line bazel_skylib 1.8.2
`lib/paths.bzl` at SHA-256
`96cce43871d8228126a12ceff771351f9030b1e9d029f2185853aa6541766a83`
under producer `@@bazel_skylib+//lib:paths.bzl`. The exported `paths` composite
retains the exact ten source-bound members as frozen function values without
invoking a helper. The comparison sorts only the observed names and makes no
Bazel-exact iteration-order claim.

All 225 loading units, 24 invalidation tests, 31 BUILD-loading tests, dependent
core checks, rebuilt CLI and hygiene pass. The packet adds 361 proof-only lines,
ending at 7,935 below the 7,994 ceiling. Independent terminal review returned
`ACCEPT`.

Run only docs packet `WP-4-7A-post-paths-rust-parent-frontier-audit`. Resume the
exact parent load order after paths returns. Account for the already-admitted
`@bazel_skylib//rules:common_settings.bzl` child and every later cached child,
then authenticate and classify the first newly evaluated module and its first
unsupported eager loading expression. Do not edit Rust or enter configured
rule/provider/action behavior.

Exact compatibility is exact-source freeze, exact producer identity and the
ten name-to-function bindings. Frozen Rust representation and proof-only sorted
comparison are Slug-native. Exact struct iteration order and every path-helper
result/diagnostic remain unsupported/deferred.

Clean `../zabel` `0795445f…` guided only closure reachability from an exported
composite through module freeze. No Zig code, representation, field ordering,
owner pointer, capture algorithm, diagnostic or behavior was copied. Bazel 9.2
remains sole behavior authority; no retained utility or ledger changed.

### M7 post-lints audit selects exact bazel_skylib paths proof (2026-08-26)

Authenticated `rust/defs.bzl` next reaches exact
`rust/private/rust.bzl` (SHA-256
`a645bd5db6344bd3c0997dcf73600475c0af53fb4dd025890be24b8e1e2dbfd8`).
Its first direct child is previously unseen bazel_skylib 1.8.2
`lib/paths.bzl`, 320 lines at SHA-256
`96cce43871d8228126a12ceff771351f9030b1e9d029f2185853aa6541766a83`.
It has no loads. Its eager surface is ten function declarations, four integer
constants and one exported struct retaining those functions; bodies remain
lazy.

Run only proof packet `WP-4-7A-bazel-skylib-paths-loading-proof`, changing
`host_package_load_tests.rs` under 0 production, 420 proof and 420 total
addition caps. Embed the exact source, verify its hash, freeze it under the
exact producer identity and prove the exact ten-member name set with every
member retained as a frozen function. Invoke none of them and stop when paths
returns.

Exact compatibility is exact-source freeze and exported name-to-function
bindings. Existing generic frozen Rust values and constructor-order iteration
are Slug-native. Bazel sorts schemaless struct keys, so observable struct
iteration/order is not an exact claim. All path function behavior and the
parent `rust.bzl` frontier remain unsupported/deferred.

Clean `../zabel` `0795445f…` guides only the requirement that functions held by
an exported composite remain reachable through module freeze. No Zig code,
representation, field ordering, owner pointer, capture algorithm, diagnostic or
behavior is copied. Bazel 9.2 remains sole behavior authority; no retained
utility or memory ledger changes.

### M7 exact lints child accepted; next parent audit selected (2026-08-26)

Commit `227257a90` freezes the exact unabridged 98-line rules_rust 0.73.0
`rust/private/lints.bzl` at SHA-256
`0c6dcf615bb9f43d57c4056253f89a9f1bed0b16b9e17d8eed64da85d1b05677`.
Its imported `LintsInfo` is pointer-identical to the provider-child export, and
the exported rule retains exact ordered StringDict/StringListDict/StringDict/
StringDict declaration schemas with nonmandatory/configurable policy and
omitted (`None`) defaults. Successful exact-source freeze proves the helper and
provider construction remain lazy.

All 224 loading units, 24 invalidation tests, 31 BUILD-loading tests, dependent
core checks, rebuilt CLI and hygiene pass. The packet adds 180 proof-only lines,
ending at 7,574 below the 7,614 ceiling. Independent terminal review returned
`ACCEPT`.

Run only docs packet `WP-4-7A-post-lints-parent-frontier-audit`. Resume exact
`rust/defs.bzl` direct-load order, account for already completed children,
authenticate the next newly evaluated module and classify its first unsupported
loading expression. Do not edit Rust or enter configured semantics.

Exact compatibility is recursive lints source freeze and its provider/export/
ordered schema identities. Existing frozen Rust storage and proof probes are
Slug-native. Rule/helper execution, LintsInfo construction, configured
dictionaries and configured provider/action behavior remain deferred.

Clean `../zabel` `0795445f…` guided only producer-owned provider identity and
declaration-owned attribute order. No Zig code, representation, owner pointer,
capture, algorithm, diagnostic or behavior was copied. Bazel 9.2 remains sole
behavior authority; no retained utility or ledger changed.

### M7 post-clippy audit selects exact lints proof (2026-08-26)

Authenticated `rust/defs.bzl` loads toolchain, clippy, common and lints in that
order. Clippy already completed common and providers, so `rust/private/lints.bzl`
is the first newly evaluated child after clippy returns. Its exact SHA-256 is
`0c6dcf615bb9f43d57c4056253f89a9f1bed0b16b9e17d8eed64da85d1b05677`.
The sole provider load is cached; the function body and its `LintsInfo(...)`
call are lazy. The only eager declaration is `rust_lint_config`, with ordered
StringDict/StringListDict/StringDict/StringDict attributes and docs, all on
already-admitted loading surfaces.

Run only proof packet `WP-4-7A-lints-child-loading-proof` in
`host_package_load_tests.rs` under 0 production, 220 proof and 220 total
addition caps. Use the exact unabridged 98-line source and the existing loaded-
child harness. Prove the imported `LintsInfo` is pointer-identical to its
provider-child export, the exact implementation source binding and rule export
identity, ordered names/kinds and omitted (`None`) declaration defaults, and
successful freeze without invoking the helper.

Exact compatibility is recursive exact-source freeze and producer/order/schema
identity. Existing frozen Rust storage and proof probes are Slug-native.
Configured lint dictionaries, rule/helper execution, provider construction and
configured action behavior remain unsupported/deferred.

Clean `../zabel` `0795445f…` guides only producer-owned imported provider
identity and declaration-owned attribute order. Copy no Zig code,
representation, owner pointer, capture, algorithm, diagnostic or behavior.
Bazel 9.2 remains sole behavior authority. The proof adds no retained utility,
hashing, collection, clone path or memory-ledger entry.

### M7 imported frozen lint descriptors accepted; parent audit selected (2026-08-26)

Commit `db51996b9` projects imported plain frozen attribute declarations and an
imported frozen transition into Slug's existing loading wrappers. Exact
`clippy.bzl:463-596` now freezes recursively with all lint/provider/common
imports retaining their producer identities. The final rule preserves ordered
common-attribute kinds and defaults, canonical allowlist/runner labels, attached
aspect/provider alternatives, pointer-identical transition implementation and
its exact output. Rich imported provider/aspect/transition descriptors still
fail closed.

All 223 loading units, 24 invalidation tests, 31 BUILD-loading tests, dependent
core checks, rebuilt CLI and hygiene pass. Final growth is 39 production and
259 proof additions; both files remain under their packet ceilings. Independent
terminal review returned `ACCEPT`.

Run only docs packet `WP-4-7A-post-clippy-parent-frontier-audit`. Return to the
source-ordered direct loads in exact `rust/defs.bzl`; account for already cached
children, authenticate the next newly evaluated child and classify its first
unsupported loading expression. Do not edit Rust or infer configured behavior.

Exact compatibility is imported plain descriptor validity/fields, imported
transition implementation/output and complete clippy-tail freeze. Rust wrapper
reconstruction and the rich-import fail-closed boundary are Slug-native.
Transition execution, identity bytes and configured provider/aspect/test/
build-setting/action behavior remain unsupported/deferred.

Clean `../zabel` `0795445f…` supplied architecture guidance only: declarations
remain producer-owned across freeze and are projected by consumers. No Zig
code, representation, pointer identity, capture, algorithm, diagnostic or
behavior was copied; Bazel 9.2 remains sole behavior authority. Existing
Arc/CompactString/default storage was reused with no new utility or ledger.

### M7 imported-transition correction exposes frozen common attributes (2026-08-26)

The selected transition correction advanced exact tail evaluation past
`targets.cfg`, then stopped when `rule(attrs=...)` processed `platform`, the
first frozen child-owned descriptor in `LINT_TEST_COMMON_ATTRS`. The rule
adapter also discarded the frozen half of `AttributeDefinition::from_value`.
The complete 9-production/248-proof candidate was removed and both files
restored to their accepted hashes.

Run only `WP-4-7A-imported-frozen-attribute-transition-clippy-tail-loading`.
In addition to the transition projection, reconstruct only plain frozen
attribute definitions whose provider/aspect/nested-transition fields are
empty. Preserve all scalar/default/label policy, fail closed on rich frozen
attrs, and rerun the exact tail proof under 55/260/315 caps.

Exact compatibility is imported plain descriptor validity and retained fields,
plus imported transition implementation/output. Existing Rust generic-wrapper
reconstruction and the rich-frozen fail-closed boundary are Slug-native.
Identity bytes, transition execution and all configured semantics remain
unsupported/deferred.

Clean `../zabel` `0795445f…` guides only producer-owned attribute and transition
publication across module freeze. Slug copies no Zig owner pointer,
representation, identity, capture, algorithm or behavior. Bazel 9.2 remains
sole behavior authority. Existing Arc/CompactString/default storage is reused
once during declaration loading, with no new utility or ledger entry.

### M7 clippy-tail proof exposes imported frozen-transition prerequisite (2026-08-26)

The selected exact recursive proof stopped at `clippy.bzl:502`, where
`targets.cfg` is the frozen `platform_transition` imported from
`lint_test.bzl:37-41`. Slug accepts a transition declared in the consuming
module but its attribute converter explicitly discards the frozen half of the
transient/frozen Starlark value union. The 246-line proof candidate was removed
and the test file restored byte-for-byte.

Run only `WP-4-7A-imported-frozen-transition-clippy-tail-loading`. Reconstruct
the existing transient generic wrapper from the imported frozen
implementation/output fields, leave invalid and `"exec"` paths unchanged, and
rerun the exact tail proof. The projected package schema must retain a
pointer-identical lint-child implementation plus the exact output. This changes
only the attribute converter and one test file under 20/260/280 caps; no
identity, registry, DICE or configured transition semantics are admitted.

Exact compatibility is acceptance of an imported frozen transition and
retention of its implementation/output in the frozen rule schema.
Reconstruction through existing Rust generic values is Slug-native.
Transition evaluation/identity bytes and configured provider/aspect/test/
build-setting/action behavior remain unsupported/deferred.

Clean `../zabel` `0795445f…` guides the producer-ownership boundary only: its
transition declaration keeps publication owner and definition-module identity
with the producer before detached capture. Slug copies no Zig representation,
identity, ordinal, capture, algorithm or behavior. Bazel 9.2 remains sole
behavior authority. The Buck2 utility review selects existing
`CompactString`/value projection with no retained collection or ledger change.

### M7 post-RunEnvironmentInfo clippy-tail audit accepts proof-only closure (2026-08-26)

The authenticated source-order audit finds no new production terminal in
`clippy.bzl:463-596`. The documented `RustClippyTestInfo`, ordered string list,
one required/advertised test aspect, child-based attribute merge, label-list
provider/aspect/transition schema, test capability and two
`config.bool(flag = True)` declarations all match already-accepted loading
contracts. Provider-constructor calls remain inside lazy helpers.

The bounded remaining gap is proof, not behavior: recursively retain the exact
lint-test exports and the provider/common producer identities, append the exact
unabridged tail to the accepted clippy prefix, and discriminate every retained
aspect/rule/build-setting edge. Run only
`WP-4-7A-clippy-test-tail-loading-proof`, changing one test file under
0/260/260 caps.

Exact compatibility is source-order tail freeze and the authenticated
producer/field/order identities. Existing Rust frozen/Arc ownership and
fail-closed invocation diagnostics are Slug-native. All helper execution and
configured provider/aspect/transition/test/build-setting/action semantics
remain unsupported/deferred.

Clean `../zabel` `0795445f…` guides only the architecture: producer-module and
export-name provider identity, declaration-owned rules and detached
build-setting descriptors support reuse of Slug's existing owners. No Zig
code, representation, capture, configured behavior, algorithm or diagnostic
is copied. Bazel 9.2 remains sole behavior authority.

### M7 RunEnvironmentInfo declaration and exact lint-test child accepted; clippy-tail audit selected (2026-08-26)

Commit `45b479e56` installs a dedicated zero-state `RunEnvironmentInfo` token
only in complete `.bzl` globals. It renders exactly as
`<function RunEnvironmentInfo>`, remains distinct from `OutputGroupInfo` and
user providers, is absent from BUILD globals, freezes without evaluator state,
and rejects every invocation before producing a value. Construction, fields,
equality/hash and configured environment/test behavior remain deferred.

The proof recursively compiles the exact unabridged 159-line rules_rust 0.73.0
`rust/private/lint_test.bzl` source with SHA-256
`4f4fade9218980db0296f99e5d199059c91ebebc7b9745bee18ad58c37b551c8`.
Its parent uses the exact `clippy.bzl:19-25` four-symbol load and proves
`LINT_TEST_COMMON_ATTRS`, `platform_transition`, `lint_test_aspect_impl` and
`lint_test_rule_impl` are pointer-identical to their frozen child exports.
Successful freeze proves neither helper nor a native-provider constructor ran.

All 222 loading units, 24 invalidation tests, 31 BUILD-loading tests, locked
analysis/core checks, rebuilt CLI, formatting and hygiene gates pass. Growth is
28 production and 217 proof additions, 245 total, within 35/220/255 caps.
Independent terminal review returned `ACCEPT`.

Run only docs audit `WP-4-7A-post-run-environment-info-clippy-tail-audit`.
Authenticate `clippy.bzl:463-596` and every imported provider/helper identity
in source order before selecting one bounded exact loading closure or
`REPLAN`. Helper execution and configured provider/aspect/transition/test,
build-setting and action semantics remain unsupported/deferred.

Clean `../zabel` `0795445f…` guided only the architecture: its distinct builtin
provider ID and declaration-owned loading binding supported one separate
native token. No Zig code, representation, value, constructor, configured
lowering, diagnostic or behavior was copied. Bazel 9.2 remains sole behavior
authority.

### M7 post-rust_clippy audit selects RunEnvironmentInfo global (2026-08-26)

The authenticated recursive source order corrects the apparent clippy-local
frontier. `clippy.bzl:19-25` loads `rust/private/lint_test.bzl` before its own
line 463 provider. The accepted rustfmt proof recreated common declarations and
stubbed helper bodies; it did not compile the exact defining helper module.
That real module has no children. After the accepted `OutputGroupInfo`,
`DefaultInfo` and `depset` globals resolve, its first missing name is
`RunEnvironmentInfo` at line 154. Starlark resolves the name while compiling
the lazy `lint_test_rule_impl`, before the helper can execute.

Pinned Bazel 9.2 installs `RunEnvironmentInfo.PROVIDER` only in its fixed
`.bzl` environment. It is a native `BuiltinProvider`, is distinct from user
providers and `OutputGroupInfo`, and renders as
`<function RunEnvironmentInfo>`. Constructor values, fields, equality/hash and
all configured test/environment behavior remain outside the bounded loading
need.

Run only `WP-4-7A-run-environment-info-declaration-global-loading`. Add a
dedicated zero-state declaration token beside `OutputGroupInfo`, install it
only in complete `.bzl` globals, reject every invocation, and recursively
compile the exact unabridged `lint_test.bzl` child through line 159. Stop before
`clippy.bzl:463`; do not infer closure of the remaining tail.

Clean `../zabel` `0795445f…` guides only the architecture: its distinct
`BuiltinProviderId.run_environment_info`, native/user identity split and
separate loading binding support one declaration-owned token. Slug copies no
Zig code, discriminant, layout, value, constructor, configured lowering,
diagnostic or behavior. Bazel 9.2 remains sole compatibility authority. The
Buck2 utility review selects the existing zero-state `Allocative` pattern and
no collection, interner, cache, hash owner or ledger update.

### M7 OutputGroupInfo declaration and rust_clippy accepted; tail audit selected (2026-08-26)

Commit `993ba5e4` installs one zero-state `OutputGroupInfo` declaration token
only in `.bzl` globals. It renders exactly as `<function OutputGroupInfo>`, is
internally distinct from user providers, freezes without an evaluator and
rejects every invocation before producing a value. Observable provider
equality/hashability and all constructed/configured output-group semantics
remain deferred.

The exact rules_rust helper now compiles without executing and `rust_clippy`
freezes with its sole provider-constrained, aspect-bearing dependency schema.
Proof establishes that its attached aspect is the identical frozen exported
producer. All 221 loading units, 24 invalidation tests, 31 BUILD-loading tests,
locked checks, rebuilt CLI and hygiene gates pass. Independent review returned
`ACCEPT` at 28 production and 124 proof additions.

Source order next reaches `RustClippyTestInfo`, a two-field documented provider,
then a fixed string list, lazy helpers, a test aspect/rule shaped like the
accepted rustfmt test declarations, and two Boolean build-setting rules. Run
only docs audit `WP-4-7A-post-rust-clippy-source-audit`: authenticate the exact
tail and imported `LINT_TEST_COMMON_ATTRS`/transition/lint-helper identities,
then select one bounded source closure or `REPLAN`.

Clean `../zabel` `0795445f…` remains guidance only. Its separate native
provider declaration/value identity influenced the accepted token; for the
tail, consult its declaration-owned provider/aspect/rule schemas only to test
owner reuse. Copy no Zig code or behavior. Bazel 9.2 remains sole authority.

### M7 OutputGroupInfo global audit accepts bounded loading (2026-08-26)

Commit `fc9473b1` shares one evaluator-detached
`ToolchainTypeRequirement` slice between rule and aspect declarations. It
retains String, Label and typed requirements in order, canonicalizes them in
the defining module, and keeps mandatory true/false in semantic identity.
Clippy's mandatory Rust and optional C++ requirements now freeze with the
complete aspect. Existing duplicate rejection remains the explicit deferred
boundary rather than approximating Bazel's strictest-wins normalization.

All 220 loading units, 24 invalidation tests, 31 BUILD-loading tests, locked
analysis/core checks, rebuilt CLI and hygiene gates pass. Independent review
returned `ACCEPT`; the only archive-status misses remain its three known
archive-only plan/evidence/prompt paths. The change is 31 production and 90
proof additions, within caps.

The independently accepted proof-only `WP-4-7A-clippy-rule-loading` candidate
was attempted without production edits. Its exact helper body fails during
Starlark compilation because `OutputGroupInfo` is not present in Slug's `.bzl`
globals; laziness prevents invocation but does not defer global name
resolution. The partial test edit was fully reverted, leaving a clean tree.

Pinned Bazel 9.2 installs `OutputGroupInfo.STARLARK_CONSTRUCTOR` directly in
the fixed `.bzl` environment. Its `BuiltinProvider` class supplies stable
provider identity, while named-group construction and artifact-depset
conversion belong to configured analysis. `BuiltinProvider.equals` and its Key
compare the concrete provider class, so Slug must not reuse its
module/export-owned user `ProviderId` or pretend that the generic
`AnalysisBuiltinCallable` is a provider identity.

Run only `WP-4-7A-output-group-info-declaration-global-loading`. Add one
zero-state native-provider declaration token in the loading provider owner,
install it only in complete `.bzl` globals, and make every constructor call
fail closed. Extend the exact clippy source proof through its helper and rule;
the helper may capture the token but may not execute. Stop before
`RustClippyTestInfo` and all output-group values/configured behavior. Match the
exact `<function OutputGroupInfo>` representation, but defer observable
equality/hashability; the distinct Rust type is Slug-native internal identity.

Clean `../zabel` `0795445f…` remains guidance only: its process-stable
`BuiltinProviderId.output_group_info` keeps native-provider identity separate
from module/export-owned user providers. That is an ownership clue, not a
compatibility decision. Slug uses its own zero-sized Rust type and copies no
Zig code, discriminant, layout, constructor, configured value or behavior.
Bazel 9.2 remains sole authority. The Buck2 utility review selects a zero-state
`Allocative` value and no collection, interner, cache or ledger update.

### M7 clippy aspect attributes accepted; toolchain requirements selected (2026-08-26)

Commit `5f8dd852` admits the exact ordered 11 private label rows, preserves
defining-repository defaults and every retained flag, and lowers them through
the same immutable schema as ordinary rules and rustfmt aspects. Source and
mutation proofs cover order, missing/extra rows, public/defaultless/wrong-kind
inputs, explicit configurability, file/provider/aspect/transition additions,
and executable/exec mismatches. The complete 219 loading units, 24 invalidation
tests, 31 BUILD-loading tests, compile/format/hygiene checks and independent
terminal review pass at 87 production and 160 proof additions.

The unchanged source-shaped call now terminates at the mixed aspect toolchain
list. Pinned Bazel 9.2 accepts String, Label and typed requirements through one
parser and retains mandatory state; the two clippy labels are distinct, so
duplicate normalization is not required for this source slice. Run only
`WP-4-7A-bazel-aspect-toolchain-requirements-loading-r2`: share Slug's existing
typed rule requirement and parser with aspects, freeze the complete source
aspect, and stop before `_rust_clippy_rule_impl`.

Clean `../zabel` `0795445f…` informed the shared rule/aspect declaration-owned
requirement slice and evaluator detachment only. The Buck2 utility review keeps
Slug's existing `CanonicalLabel`, Boolean, immutable `Arc` slice and
`Allocative`; no Zig code/behavior, new utility or Stage 9 ledger row is used.
Bazel 9.2 remains sole behavior authority.

### M7 clippy aspect attribute audit accepted; loading selected (2026-08-26)

Pinned Bazel 9.2 converts valid private Starlark names to implicit internal
names, preserves attribute-dictionary order, rejects user-set configurability,
requires defaults for implicit attributes and retains ordinary label
descriptor state. Its focused tests accept a private label default, reject a
defaultless private label and a public label parameter, and cover executable,
exec-configuration and single-file construction.

The clippy map is exactly 11 ordered private `attr.label` rows. Every row has a
concrete defining-repository default and omitted configurability; `_config`
alone has `allow_single_file=True`, while `_process_wrapper` alone is
executable in the exec configuration. No row has ordinary file allowance,
providers, attached aspects, allowed values or a custom transition.

Slug already owns every required fact in `AttributeDefinition`,
`declared_attribute_schema` and the transient/frozen
`AspectDefinitionGen.attributes` slice. Run only
`WP-4-7A-clippy-aspect-attribute-loading`: add one exact source gate beside the
rustfmt pair and reuse that owner. The source-shaped proof may omit or simplify
the later toolchain list to show all 11 rows freeze; the unchanged mixed list
must remain the next terminal. Broader aspect attributes, configured use and
toolchain parsing remain deferred.

Clean `../zabel` `0795445f…` informed only the shared rule/aspect
`NamedAttribute` ownership and evaluator-detached retention. Slug keeps its
existing Rust canonical labels and immutable schema, copies no Zig code or
behavior, and changes no retained representation. Bazel 9.2 remains sole
behavior authority. Independent audit approved the bounded implementation.

### M7 post-toolchain source-order correction; aspect attribute audit selected (2026-08-26)

The external `.bzl` driver resolves all parent loads before child lookup, then
computes children serially in AST source order and returns at the first child
terminal. Its structural route-plus-label DICE keys and manifest regression
prove repeated completed children are reused. Consequently the completed
private toolchain returns through the alias-only public wrapper; the remaining
`rust/toolchain.bzl` children are already-complete analyzer/rustfmt/toolchain
modules; and `rust/defs.bzl` next evaluates `rust/private/clippy.bzl`.

Clippy's first import evaluates bazel_skylib 1.8.2 `lib/structs.bzl`, whose
sole top-level struct uses the accepted surface; its other six imports reuse
completed children. Its provider and two string-list build-setting rules
freeze, while function bodies and documentation examples stay lazy.

Commit `7bba3a4e` selected aspect toolchains as the next surface, but its first
source-shaped proof disproved that ordering before any Rust was accepted. All
keyword values evaluate, then Slug's `aspect()` body calls `aspect_attributes`
before `aspect_toolchain_requirement`. That owner rejects clippy's line
317-364 11-label map because only the fixed rustfmt `_config` and
`_process_wrapper` pair is admitted. The attempted two-file refactor was fully
reverted; no production or test change remains.

Run docs-only `WP-4-7A-clippy-aspect-attribute-audit`. Authenticate Bazel's
private aspect-attribute validation, defaults, configuration and executable
rules, then select a bounded implementation or `REPLAN`. The mixed mandatory/
optional toolchain list remains a later source surface.

Clean `../zabel` `0795445f…` guides only the shared rule/aspect named-attribute
ownership and evaluator-detached schemas useful to this audit. It defines no
accepted behavior. No Zig code, layout, diagnostics or algorithm may be
copied; Bazel 9.2 remains sole behavior authority.

### M7 config-common toolchain requirement accepted; caller audit selected (2026-08-26)

Commit `4aed2438` adds a typed Rust rule-toolchain requirement owning canonical
label plus mandatory state. `.bzl` `config_common.toolchain_type` accepts the
selected String/Label forms, existing bare strings remain mandatory, distinct
requirements retain order, duplicates fail closed, and optional target
invocation rejects before publication. The rules_rust optional C++ requirement
freezes and completes `rust/private/toolchain.bzl` without running its
implementation.

All 217 loading units, 24 invalidation tests, 31 BUILD-loading tests, the
configured mandatory-toolchain regression, locked checks and rebuilt CLI pass.
Final growth is 172 production, 111 proof and 283 total additions; independent
terminal review returned `ACCEPT`.

The source-text candidate returns through alias-only
`rust/rust_toolchain.bzl`, completes the remaining wrapper load and aliases in
`rust/toolchain.bzl`, then names `rust/private/clippy.bzl` next in
`rust/defs.bzl`. Run only docs packet
`WP-4-7A-post-toolchain-source-order-audit`; replay the recursive manifest and
cache order before accepting that route or pinning the first unsupported
expression.

Clean `../zabel` `0795445f…` continues as architectural guidance only for typed
rule/aspect requirement ownership and evaluator-detached capture. It supplies
no source-order or compatibility conclusion; Bazel 9.2 and the selected
rules_rust archive remain authoritative.

### M7 scalar-label provider predicate accepted; toolchain requirement selected (2026-08-26)

Commit `ef910068` admits omitted/empty and one exported provider in a flat
scalar-label predicate, retains its canonical provider identity in the existing
nested immutable schema, and rejects broader shapes and unsupported projections.
Both source provider rows freeze, constrained target invocation fails before
recording, and all 215 loading tests plus downstream gates pass within 22
production, 88 proof and 110 total additions. Independent terminal review
returned `ACCEPT`.

Source order now reaches the sole rule-level
`config_common.toolchain_type("@bazel_tools//tools/cpp:toolchain_type",
mandatory=False)` call, after which only the `rust_toolchain` documentation and
the end of `rust/private/toolchain.bzl` remain. Pinned Bazel 9.2 constructs a
typed requirement from String or Label input, resolves strings in the defining
`.bzl` repository mapping, defaults `mandatory` true, and retains false through
`rule(toolchains=...)`.

Run only `WP-4-7A-bazel-config-common-toolchain-type-loading`. Introduce one
Rust label-plus-mandatory requirement shared by the declaration, frozen rule
and package schema, keep existing string requirements mandatory, and reject
optional target invocation before publication. Duplicate normalization,
aspects, configured optional resolution and other `config_common` members stay
deferred. Re-audit the caller after the child completes.

Clean `../zabel` `0795445f…` guides the same declaration-owned typed requirement
and evaluator-detached canonical capture. Slug uses its own Rust
`CanonicalLabel`, Boolean and immutable `Arc` slice; no Zig code, layout or
behavior is copied. Bazel 9.2 remains sole behavior authority.

### M7 scalar-label file allowance accepted; provider predicate selected (2026-08-26)

Commit `b1edbe0e` adds Boolean/`None` `allow_files` to scalar labels, performs
the simultaneous non-None single-file conflict before normalization, and
retains the existing Boolean through freeze and package schemas. True remains
distinct from single-artifact identity; repository/tag projections fail
closed. The rules_rust prefix crosses both LLVM file rows. All 214 loading
tests, configured analysis, locked checks, rebuilt CLI and hygiene pass within
10 production, 91 proof and 101 total additions. Independent terminal review
returned `ACCEPT`.

Source order now reaches `lto` with `providers=[RustLtoInfo]`, and later the
hidden allocator setting repeats the same shape with `BuildSettingInfo`.
Pinned Bazel 9.2 normalizes a flat provider list into one conjunctive predicate
of exported provider identities. Run only
`WP-4-7A-bazel-label-provider-predicate-loading`: accept the source-required
singleton flat list, reuse the existing nested immutable provider schema, and
fail closed at invocation and unsupported projections. Stop after the complete
attribute map at `config_common.toolchain_type(...)`.

Clean `../zabel` `0795445f…` guides sharing the same provider-predicate
declaration slot across dependency attribute kinds and detaching it before
package lowering. Slug reuses its existing Rust nested `Arc` provider identity
and copies no Zig evaluator value, code, layout or behavior. Bazel 9.2 remains
sole behavior authority; no new Buck2 utility or Stage 9 ledger row is needed.

### M7 string allowed values accepted; scalar-label file allowance selected (2026-08-26)

Commit `80425ce9` replaces parallel integer-only storage with one evaluator-
free integer/string allowed-values enum. String constraints normalize into
compact immutable sets, participate in schema equality, and check explicit
direct, selectable and final concatenated candidates. Ordinary defaults remain
unchecked and unsupported projections fail closed. Both rules_rust linker
constraints freeze without invoking the implementation. All 213 loading tests,
configured analysis, locked checks, rebuilt CLI and hygiene pass within 77
production, 165 proof and 242 total additions. Independent terminal review
returned `ACCEPT`.

Source order now reaches `llvm_lib` and `llvm_tools`, whose scalar
`attr.label(allow_files=True)` rows are the next missing constructor subset.
Pinned Bazel 9.2 treats true as `ANY_FILE`, false/omitted/`None` as no files,
rejects simultaneous non-None `allow_files` and `allow_single_file`, and keeps
plain file allowance distinct from `SINGLE_ARTIFACT`. Run only
`WP-4-7A-bazel-label-allow-files-loading`: wire the Boolean/`None` subset into
the existing declaration-owned Boolean and presence conflict check. Stop at
`lto`, whose `providers=[RustLtoInfo]` remains unadmitted.

Clean `../zabel` `0795445f…` guides the same separate `allows_files` and
`allows_single_file` ownership and pre-normalization conflict boundary. Slug
reuses its existing Rust Boolean and copies no Zig code, layout or behavior.
Bazel 9.2 remains sole behavior authority; no new Buck2 utility or Stage 9
ledger row is needed.

### M7 integer allowed values accepted; string allowed values selected (2026-08-26)

Commit `563699ab` retains a normalized signed-32-bit integer allowed-value set
through transient, frozen and package schemas. Nonempty constraints participate
in structural equality, disallowed explicit/plain-select candidates reject,
ordinary omitted defaults stay unchecked, and repository/tag projections fail
closed. The rules_rust prefix crosses `[-1, 0, 1]` and stops at its first
string constraint. All 212 loading tests, configured analysis, locked checks,
rebuilt CLI and hygiene pass within 73 production, 160 proof and 233 total
additions. Independent terminal review returned `ACCEPT`.

Source order now reaches `linker_preference` and `linker_type` at lines
766-772, whose `attr.string(values=...)` rows are the next absent evaluated
arguments. Pinned Bazel 9.2 types these as string sequences, installs no
predicate for empty sequences, and checks direct, selectable and concatenated
explicit candidates while leaving ordinary defaults unchecked. Run only
`WP-4-7A-bazel-string-allowed-values-loading`: replace the integer-only field
with one typed integer/string enum, retain normalized compact slices, and reuse
the existing correlated candidate expansion for string enforcement. Stop at
`llvm_lib` line 781, whose label `allow_files=True` remains unadmitted.

Clean `../zabel` `0795445f…` guides the same unified declaration-owned
`allowed_values` boundary and evaluator detachment. Slug uses one Rust enum,
existing `Arc`/`CompactString`/`Allocative` patterns, and copies no Zig code,
layout or behavior. Bazel 9.2 remains sole behavior authority; no new Buck2
import or Stage 9 ledger row is needed.

### M7 data-attribute docs accepted; integer allowed values selected (2026-08-26)

Commit `8d3f9b6e` accepts omitted, string and explicit `None` documentation on
the int, string-list, string-dict and string-list-dict constructors used by
`rust_toolchain`. Wrong types reject, distinct doc text leaves frozen schemas
and typed defaults equal, and no documentation enters semantic identity. All
210 loading units, configured analysis, locked checks, rebuilt CLI and hygiene
pass at 8 production and 61 proof additions. Independent terminal review
returned `ACCEPT`.

Source order now reaches
`experimental_use_allocator_libraries_with_mangled_symbols` at lines 727-738,
whose `attr.int(values = [-1, 0, 1], default = -1)` is the first unadmitted
evaluated argument. Pinned Bazel 9.2 types `values` as a list/tuple of integers,
normalizes empty to no predicate, retains a nonempty allowed set and checks
every possible explicitly supplied/select candidate during package loading;
ordinary rule defaults remain unchecked. Run
only `WP-4-7A-bazel-int-allowed-values-loading`: detach a normalized immutable
integer set into the existing declaration/frozen/package schemas and enforce it
before target recording. Stop at `linker_preference` line 768, whose
`attr.string(values = ["cc", "rust"])` remains unadmitted.

Clean `../zabel` `0795445f…` guides keeping allowed values beside the
declaration-owned default and detaching evaluator state. Slug uses its existing
Rust `Arc<[T]>` plus `Allocative` pattern, with no Zig code, behavior or layout
copied. Bazel 9.2 remains sole behavior authority; no new Buck2 import or
Stage 9 ledger row is needed.

### M7 rust stdlib filegroup accepted; data-attribute docs selected (2026-08-26)

Commit `75709828` retains Bazel's normalized Boolean `allow_files` predicate
through transient, frozen and package-owned label-list schemas. Omitted,
explicit `None` and false remain no-file; true is any-file; extension lists and
actual file resolution remain fail-closed. The source-shaped
`rust_stdlib_filegroup` freezes and projects into a target schema without
running its implementation. All 209 loading units, configured analysis,
locked checks, rebuilt CLI and hygiene pass within 37 production, 84 proof and
121 total additions. Independent terminal review returned `ACCEPT`.

The next evaluated `rust_toolchain` attributes pass accepted label/string
shapes until `debug_info` calls `attr.string_dict(doc = ...)` at line 695.
Slug's remaining data constructors lack the otherwise-shared string/`None`
documentation ABI. Run only `WP-4-7A-bazel-data-attribute-doc-loading`: apply
the existing validation-and-discard helper to int, string-list, string-dict and
string-list-dict descriptors used by this rule. Stop at
`experimental_use_allocator_libraries_with_mangled_symbols`, whose
`attr.int(values = [-1, 0, 1])` remains unadmitted.

Clean `../zabel` `0795445f…` guides the same transient validation-and-discard
boundary; Bazel 9.2 remains sole behavior authority. No retained representation
changes, collections or Buck2 ledger rows are needed.

### M7 cc_common wrapper accepted; label-list file allowance selected (2026-08-26)

Commit `4bdd64bf` exposes only Bazel's deprecated
`do_not_use_tools_cpp_compiler_present` property as `None`. Direct and captured
reads, wrapper freezing, property presence, non-callability and unknown-field
absence are proved while BUILD exposure and configured C++ semantics remain
unchanged. All 207 loading units, configured analysis, locked checks, rebuilt
CLI and hygiene pass at 4 production and 34 proof additions. Independent
terminal review returned `ACCEPT`.

Source order now resumes `rust/private/toolchain.bzl`. Its first declaration,
`rust_stdlib_filegroup`, reaches `attr.label_list(allow_files = True)` at line
115; Slug's label-list constructor has no `allow_files` parameter. Pinned Bazel
9.2 maps Boolean true to `FileTypeSet.ANY_FILE` and keeps the attribute a
non-single-artifact label list. Run only
`WP-4-7A-bazel-label-list-allow-files-loading`: retain the normalized Boolean
predicate through freeze, export and target schema identity, then freeze the
source-shaped rule. Extension lists, actual source-file target resolution and
the later `rust_toolchain` declaration remain deferred.

Clean `../zabel` `0795445f…` guides the distinct declaration-owned
`allows_files` fact and separation from single-artifact policy. No Zig code,
layout, algorithm or behavior is copied; Bazel 9.2 remains sole compatibility
authority. The Buck2 reuse audit selects one inline Boolean in existing
retained schemas, with no collection, allocation, interner or ledger row.

### M7 empty compilation outputs accepted; cc_common compiler sentinel selected (2026-08-26)

Commit `b0cd7855` accepts only the exact empty-list row of
`cc_internal.freeze`. Ten source-default empty lists now produce evaluator-
owned frozen lists and top-level `EMPTY_COMPILATION_OUTPUTS` freezes. Non-empty
and general container shapes remain fail-closed. All 206 loading units,
configured analysis, locked checks, rebuilt CLI and hygiene pass within the
15/69/84 addition caps; independent terminal review returned `ACCEPT`.

Recursive source order passes lazy `compile.bzl` declarations and reaches
`cc/private/cc_common.bzl:735`, which captures the deprecated native field
`do_not_use_tools_cpp_compiler_present`. Pinned Bazel 9.2 defines its value as
`None`. Run only `WP-4-7A-bazel-cc-common-compiler-sentinel-loading`. Clean
`../zabel` `0795445f…` guides the same direct-property wrapper boundary and
`None` observation only; no Zig code or behavior is copied.

### M7 documented provider initializer accepted; empty-list freeze selected (2026-08-26)

Commit `152caa6f` generalizes the existing initialized provider schema parser
to documented string dictionaries and completes the source-shaped `CcInfo` and
`CcLauncherInfo` declarations without a second owner or representation. All
205 loading units, configured analysis, locked checks, rebuilt CLI and hygiene
pass; independent terminal review returned `ACCEPT`.

Source order next reaches top-level `EMPTY_COMPILATION_OUTPUTS` and its ten
`_cc_internal.freeze` calls, all with default empty lists. Pinned Bazel 9.2
returns an immutable list copy. Run only
`WP-4-7A-bazel-empty-list-freeze-loading`: reuse starlark-rust's existing
frozen empty-list singleton and fail closed for non-empty/general container
shapes. Clean `../zabel` `0795445f…` guides the evaluator-owned immutable-copy
boundary and mutation proof only; no Zig code or behavior is copied.

### M7 empty HeaderInfo accepted; documented provider initializer selected (2026-08-26)

Commit `2ebc6fe1` adds only the no-argument private
`create_header_info()` method and one loading-only immutable `HeaderInfo` with
fresh occurrence identity, four `None` module fields and four immutable empty
header-list observations. Hashing, named/non-empty calls, dependencies and
configured C++ lowering remain unsupported. Focused proof, all 204 loading
units, configured analysis, locked checks, rebuilt CLI and hygiene pass at 77
production, 74 proof and 151 total additions. Independent review corrected the
source stop to `CcInfo` at lines 260–269, then terminal review returned
`ACCEPT`.

Pinned Bazel 9.2 accepts both string-list and string-to-string documented
schemas with a callable initializer. The argument processor and raw constructor
are otherwise identical. Extending the accepted initialized definition's
schema parser completes `CcInfo` and then `CcLauncherInfo`; source order passes
the shared-library hint and LTO children before stopping at
`cc_compilation_outputs.bzl:86` on `_cc_internal.freeze(objects)`.

Run only `WP-4-7A-bazel-documented-provider-initializer-loading`. Reuse the
same `ProviderId`, initializer/raw owner, compact schema names/ordinals and
loading-only instance. Clean `../zabel` `0795445f…` guides that single complete
definition owner and normalized schema projection only. No Zig code or
behavior is copied; Bazel 9.2 remains sole compatibility authority.

### M7 provider schemas accepted; empty HeaderInfo selected (2026-08-26)

Commit `f65c9ce0` accepts omitted/`None`, unique string-list and documented-map
provider schemas, optional arbitrary direct loading values, compact schema
ordinals and schemaless dynamic names. The existing full documented-string
configured projection remains unchanged; every other new instance is loading-
only. Focused proof, all 203 loading units, configured analysis, locked checks,
the rebuilt CLI and hygiene pass within the 173 production, 102 proof and 275
total addition caps. Independent review returned `ACCEPT`.

Recursive source order now freezes
`cc/private/link/create_extra_link_time_library.bzl` and returns to
`cc/private/cc_info.bzl`. Its first absent expression is line 134,
`_cc_internal.create_header_info()`, while building the top-level empty
compilation context. Pinned Bazel 9.2 creates a fresh immutable `HeaderInfo`
whose four module fields are `None` and whose four direct header lists are
empty. No arguments, dependencies, Files or configured C++ lowering are needed
for this source row. Accepting it resumes the file until lines 260–269, where
the dictionary-schema initialized `CcInfo` provider is still unsupported and
becomes the next separate packet.

Run only `WP-4-7A-bazel-empty-header-info-loading`. Keep the value loading-only,
retain fresh occurrence identity and immutable empty field observations, and
leave hashing, non-empty fields, dependency DAGs and analysis lowering
unsupported. Clean `../zabel` `0795445f…` guides the evaluator-local owned
HeaderInfo and later retained-lowering phase split only; no Zig code or behavior
is copied, and Bazel 9.2 remains sole compatibility authority.

### M7 provider initializer accepted; provider schemas selected (2026-08-26)

Commit `9c51999f` accepts the initialized-provider declaration, normal/raw
construction, original-argument forwarding, dictionary/schema validation,
optional fields, shared assignment-bound identity and freezeable arbitrary
values required by rules_cc artifact categories. The new family remains
loading-only and cannot downcast as the configured string provider. Focused
proof, all 202 loading units, the configured regression, locked core check,
rebuilt CLI and hygiene pass. Final growth is 300 production and 97 proof
additions. Independent review restored the legacy unbound-provider diagnostic
and returned `ACCEPT`.

Recursive loading next reaches
`cc/private/link/create_extra_link_time_library.bzl` through `cc_info.bzl`.
Its first absent call is `provider("ExtraLinkTimeLibraryInfo")`; the same child
also declares a string-list schema and immediately constructs a documented-map
provider with `libraries = []`. Run only
`WP-4-7A-bazel-provider-schema-loading`: distinguish schemaless from schemaful
definitions, accept optional arbitrary direct loading values, reuse compact
schema ordinals, and preserve the existing all-string configured projection.
Stop before `cc_info.bzl` calls `cc_internal.create_header_info()`.

Clean `../zabel` commit `0795445f…` is architectural guidance only. Its
provider schema leaf distinguishes schemaless/schemaful ownership, while one
provider definition owns schema, initializer, publication owner and export
identity. Slug follows that owner/kind split through starlark-rust and retained
Buck2 utilities without copying Zig code or behavior. Bazel 9.2 remains sole
compatibility authority.

### M7 cc_common private bridge accepted; provider initializer selected (2026-08-26)

Commit `4d7a9bbb` adds the `.bzl`-only public `cc_common` projection, accepts
exactly zero-argument `internal_DO_NOT_USE()` from canonical `rules_cc+`
owners, returns a frozen opaque `cc_internal` token, and keeps BUILD and every
C++ method absent. Focused bridge and all 201 loading units pass. Broad loading
remains 30/31 only for the recorded stale `@external` diagnostic-order row;
locked core check, rebuilt CLI, formatting and archive hygiene retain their
accepted baselines. Independent review corrected root canonical diagnostic
spelling to Bazel's `//...` form, then returned `ACCEPT` at 92 production and
64 proof additions.

Source order now passes lazy `cc/private/paths.bzl` and reaches
`cc/common/cc_helper_internal.bzl`'s initialized `_ArtifactCategoryInfo`
provider. Pinned Bazel 9.2 requires a string-list schema plus callable `init`,
returns `(provider, raw_constructor)`, forwards original constructor arguments
through `init`, validates its dictionary against the schema, and makes the raw
constructor bypass the callback while rejecting positional arguments. Declared
fields remain optional. The rules_cc source immediately constructs and freezes
its fixed artifact-category instances, so declaration and instance loading form
one child-completing abstraction.

Run only `WP-4-7A-bazel-provider-initializer-loading`. Add a loading-only
initialized callable/raw/instance family beside the unchanged configured
string-provider representation. One assignment-bound provider identity owns
both constructors; retained closures, references and arbitrary freezeable
field values stay in the frozen module heap. Initialized instances remain
unsupported as rule-analysis results. Stop before later rules_cc loads or any
C++ provider, toolchain, action or analysis method.

Pinned Zabel `c7298478…` guides the single provider-definition owner,
normal-versus-raw split, and freeze/lifetime discipline visible in its
rules_cc-shaped initialized-provider regression. No Zig implementation,
representation or behavior is copied; Bazel 9.2 remains sole compatibility
authority. Existing starlark-rust `Value`/`FrozenValue`, `CompactString`,
`SmallMap`, `Dupe` and `Allocative` patterns satisfy the Buck2 utility review
without a new import or ledger row.

### M7 config-string descriptor accepted; cc_common private bridge selected (2026-08-26)

Commit `919ecfa5` completes the selected bazel_skylib common-settings child.
`.bzl` `config.string` now has Bazel's named-only `flag` and `allow_multiple`
booleans with false defaults and retains all four identities through rule
projection, recursive freeze and equality. BUILD keeps its existing
true/single-only constructor. Only true/single definitions may record and use
the admitted scalar configured consumer; non-flag and multi-value variants
fail before target recording.

Focused descriptor/ABI, supported-package and configured-cquery proof passes;
all 200 loading units pass. The broad integration remains 30/31 with only its
declared stale `@external` diagnostic-order row. Locked core check, rebuilt
CLI, formatting, hygiene and the known archive baseline pass. Growth is 41
production and 134 proof additions within caps. Independent terminal review
returned `ACCEPT`.

Source order returns to rules_rust 0.73.0
`rust/private/toolchain.bzl`, whose second child is rules_cc 0.2.17
`cc/common/cc_common.bzl`. That child enters the generated Bazel-9
compatibility proxy, then `cc/private/cc_common.bzl`, then
`cc/common/cc_helper_internal.bzl`. The first missing evaluated expression is
`cc_common.internal_DO_NOT_USE()` in `cc/private/cc_internal.bzl`; the prior
Skylib `paths` child contains only lazy functions, constants and the accepted
keyword-only `struct` construction.

Pinned Bazel 9.2 constructs public `cc_common` through builtins injection.
Its `internal_DO_NOT_USE` wrapper calls the private `cc_internal` checker with
the rules_cc allowlist; canonical repositories beginning `rules_cc+` are
accepted and other owners receive the private-API diagnostic. Run only
`WP-4-7A-bazel-cc-common-private-bridge-loading`: add a `.bzl`-only,
owner-checked bridge and return one frozen opaque internal token. BUILD must
remain without `cc_common`, and every internal member, provider, toolchain,
action and analysis operation remains deferred. Stop and re-audit before the
next rules_cc expression.

Pinned Zabel `c7298478…` is architectural guidance only. Its C++ builtins leaf
does not install a public global itself, exposes the internal token through a
private capability, and makes owner enforcement mandatory. Slug follows the
public/private phase split and fail-closed owner rule with its current complete
`.bzl` globals owner, but does not copy Zig code, methods, builtins execution
or C++ semantics. Pinned Bazel 9.2 remains sole behavior authority.

### M7 config-string-list false accepted; config-string descriptor selected (2026-08-26)

Commit `297c2286` completes `.bzl` StringList declaration identity. The compact
descriptor now retains `flag` beside `repeatable`, accepts every valid pair,
and preserves Bazel's exact false-flag/true-repeatable diagnostic. Omitted and
explicit false/false agree; all three valid identities discriminate. BUILD
remains without StringList, and all list target variants fail before recording.
All 198 loading units pass; the broad integration retains only its stale
`@external` diagnostic-order failure. Locked core check, rebuilt CLI, formatting
and hygiene pass. Final growth was 7 production and 97 proof additions after
the terminal reviewer requested explicit integer-type ABI rows and producer
export assertions, then returned `ACCEPT`.

The selected Skylib child next passes a lazy string implementation and the
already-admitted `config.string(flag=True)` declaration. Its final absent
expression is `config.string()` at line 172. Pinned Bazel 9.2 declares named-
only `flag` and `allow_multiple`, both false by default, and retains both on the
STRING build-setting descriptor. Slug currently owns only a unit String kind
and exposes no `allow_multiple` argument.

Run only `WP-4-7A-bazel-config-string-descriptor-loading`. Complete all four
descriptor identities but preserve the existing configured boundary: only
`flag=True, allow_multiple=False` may record and use Slug's admitted scalar root
string setting. Reject non-flag and multi-value rule invocation before package
recording. Preserve the existing BUILD true/single constructor without
broadening it. After the Skylib child finishes, audit the next loaded child of
`rust/private/toolchain.bzl` separately.

Pinned Zabel `c7298478…` is architecture guidance only. Its evaluator-free
definition keeps String kind, flag and allow-multiple together, supporting the
same producer/freeze owner but no behavior conclusion. Bazel 9.2 remains sole
behavior authority.

### M7 config-bool false accepted; config-string-list false selected (2026-08-26)

Commit `52d2c6f2` completes `.bzl` `config.bool` flag identity. Named true,
omitted and explicit false forms preserve their BOOLEAN kind and flag bit
through rule construction, recursive freeze and equality; omitted and explicit
false agree while true differs. BUILD remains without the constructor, and the
unchanged Boolean target rejection now lives beside integer rejection in the
small pre-recording helper. All 198 loading unit tests pass; the broad loading
integration retains only its recorded stale `@external` diagnostic-order
failure. Locked core check, rebuilt CLI, formatting and hygiene pass. Final
growth was 15 production and 76 proof additions; independent review returned
`ACCEPT`.

Source order then passes the admitted nonrepeatable and repeatable true-flag
StringList declarations at lines 107-129. The first absent expression is
`config.string_list()` at line 133 because Slug rejects a false/omitted flag and
retains only repeatability. Pinned Bazel 9.2 declares both arguments named-only
and false by default, retains both bits, and rejects `repeatable=True` unless
`flag=True`. The next absent expression after this declaration is
`config.string()` at line 172.

Run only `WP-4-7A-bazel-config-string-list-false-loading`. Retain `flag` beside
`repeatable` in the existing compact descriptor, accept the complete valid
matrix, preserve the pinned invalid-pair diagnostic, keep BUILD absence and
fail all list target invocation before recording. Do not add CLI accumulation,
configured values, transitions, providers, analysis or actions.

Pinned Zabel `c7298478…` remains architectural guidance only. Its one
evaluator-free build-setting definition keeps StringList kind, flag and
repeatability together, supporting Slug's existing producer/freeze owner. No
Zig behavior or code is adopted; Bazel 9.2 remains sole behavior authority.

### M7 config-int accepted; config-bool false identity selected (2026-08-26)

Commit `9685d9a7` admits `.bzl` `config.int` with named-only `flag` defaulting
to `False`. INTEGER kind and flag polarity now survive rule construction,
recursive freeze and equality. Omitted and explicit false descriptors agree;
true differs. The existing builtin-schema owner derives mandatory,
nonconfigurable Integer `build_setting_default` and optional string `help`.
BUILD retains no integer constructor and integer target invocation fails before
package recording. Focused proof, all 198 loading unit tests, locked core check,
rebuilt CLI, formatting and hygiene pass. The broad loading integration retains
only its recorded stale `@external` diagnostic-order failure. Final growth was
32 production and 108 proof additions within caps; independent terminal review
returned `ACCEPT`.

The accepted Skylib child then freezes `bool_flag` through the already-admitted
`config.bool(flag = True)` descriptor. Its next declaration reaches
`config.bool()` at line 100, which is the first absent evaluated expression:
Slug currently rejects false/omitted Boolean flags and retains no Boolean flag
bit. Pinned Bazel 9.2 declares the argument named-only with default `False`,
passes that bit into a BOOLEAN `BuildSetting`, and derives the same mandatory
Boolean default schema for both flag identities.

Run only `WP-4-7A-bazel-config-bool-false-loading`. Complete the existing
Boolean descriptor as `{ flag }`, accept named true, omitted and explicit
false, retain equality/discrimination through recursive freeze, keep BUILD
absence, and preserve the pre-recording invocation rejection. Do not add CLI,
configured, transition, provider, analysis or action behavior. After this
slice, source order stops at `config.string_list()` on line 133.

Pinned Zabel `c7298478…` is architectural guidance only. Its evaluator-free
`BuildSettingDefinition` owns Boolean kind and flag together, supporting the
same declaration/freeze phase split. No Zig code, layout, behavior, configured
capture or analysis algorithm is adopted; pinned Bazel 9.2 remains sole
behavior authority.

### M7 post-rustfmt audit accepts config-int loading (2026-08-26)

Commit `1e2759c2` selected recursive source-order authentication. The accepted
rules_rust archive finishes `rust/private/rustfmt.bzl`: its remaining two rule
declarations use already-admitted docs, label schemas and canonical toolchain
strings while their implementations stay lazy. Evaluation returns through the
alias-only rust-analyzer wrapper and reaches `rust/private/toolchain.bzl` via
`rust/rust_stdlib_filegroup.bzl`.

The first child is lawfully mapped to selected `bazel_skylib@1.8.2`. Its BCR
source JSON hashes to `34a3c8bc…`, its accepted archive hashes to
`6e78f0e5…`, and `rules/common_settings.bzl` hashes to `f3bcedef…`. Provider
and attribute declarations through line 69 are supported; the first absent
evaluated expression is `config.int(flag = True)` at line 71, followed by
`config.int()` at line 81.

Pinned Bazel 9.2 defines one named-only `flag` argument defaulting to `False`.
Both calls create an INTEGER build-setting descriptor whose flag bit is
retained; rule construction adds mandatory, nonconfigurable integer
`build_setting_default` plus optional string `help`. Accept named `True`,
omitted and explicit `False` in one loading packet because the selected source
requires both identities. Positional, nonboolean and unknown forms reject.
Integer target invocation, CLI parsing, configured values and analysis remain
deferred; the next source frontier is `config.bool()` at line 100.

Pinned Zabel `c7298478…` guides only the owner shape: its evaluator-free
`BuildSettingDefinition` keeps integer kind and flag together. Slug reuses its
existing compact `BuildSettingKind`, frozen rule schema and `Allocative`
values; no Zig code, layout, behavior, cache or configured consumer is adopted.
Bazel 9.2 remains sole behavior authority.

### M7 rustfmt test target attribute accepted; post-rustfmt audit selected (2026-08-26)

Commit `88304c2f` freezes the fixed `targets` label-list declaration with its
ordered `CrateInfo`/`TestCrateInfo` alternatives, complete exported
`_rustfmt_test_aspect`, and existing `platform_transition`. All producer
identities survive recursive module freeze. Target invocation fails before
configured loading could discard provider/aspect facts; application,
transition execution and provider matching remain deferred.

Focused proof, all 196 loading unit tests, unaffected loading integrations,
locked core check, rebuilt CLI, formatting and diff gates pass. The sole broad
integration failure remains the recorded stale `@external` diagnostic
expectation. Final growth is 66 production and 175 proof additions within all
caps. Independent review requested one duplicate-aspect rejection row and
returned `ACCEPT` after that bounded correction.

Source order continues through `rust/private/rustfmt.bzl:281-356`, then returns
to `rust/toolchain.bzl`. The remaining rustfmt toolchain declarations appear to
use accepted label schemas and toolchain-label conversion, but that must be
proved against the live loader. The next uncached wrapper reaches
`rust/private/toolchain.bzl`, whose first mapped child is
`@bazel_skylib//rules:common_settings.bzl`; its first candidate missing surface
is `config.int(flag = True)` at line 71. Run only the docs audit before adding
integer settings or assuming every preceding child is already supported.

Pinned Zabel `c7298478…` guides only the declaration owner: its typed
`BuildSettingKind.int` lives beside the other evaluator-free build-setting
kinds. Slug may reuse that phase split, but no Zig code, layout, behavior,
cache or configured semantics may be adopted. Pinned Bazel 9.2 remains the
sole behavior authority.

### M7 rustfmt test target-attribute audit accepted; loading selected (2026-08-26)

Commit `cb8df441` selected the declaration audit. Pinned Bazel 9.2 proves
`attr.label_list` builds one immutable factory containing trimmed docs,
normalized required-provider alternatives, an exported aspect list and the
custom transition factory. The enclosing rule only marks that it propagates
aspects and has a Starlark transition; implementations, aspects, provider
matching and transitions do not execute during declaration loading.

The exact `dict(LINT_TEST_COMMON_ATTRS, **{"targets": ...})` overlay uses
ordinary Starlark dictionary update semantics. Keyword entries replace an
existing value without moving its key; this fixed base has no `targets`, so
the descriptor is appended after the four already-frozen common attributes.
No Slug code is needed for that merge.

Slug can extend its existing transient/frozen `RuleAttributeSchemaGen` with
the fixed two singleton provider alternatives and one complete frozen aspect
object, while reusing its current frozen transition object. Documentation is
validated and discarded consistently with earlier admitted attributes.
Target invocation must fail before the loading `AttributeSchema` projection
can drop provider/aspect facts.

Pinned Zabel `c7298478…` guides only that owner and phase split: its single
`AttrDefinition` retains optional providers/aspects/cfg, and later configured
capture detaches their producer identities and transition provenance. Slug
uses its own `ProviderId`, frozen values and Arc/Option storage; no Zig code,
layout, behavior, evaluator, cache or analysis algorithm is copied. Bazel 9.2
remains sole behavior authority.

### M7 rustfmt test aspect accepted; target-attribute audit selected (2026-08-26)

Commit `50205fb3` freezes the third rustfmt aspect with exactly
`@@dep+//rust/private:rustfmt.bzl%RustfmtTestInfo` as its advertised provider.
The importer alias preserves the private aspect's first export, and complete
producer identities are proved for both recursively required aspects. Omitted
advertised-provider state remains empty; explicit empty, duplicate, wider,
unexported and non-provider forms fail closed. No implementation runs and no
aspect is applied.

Focused proofs, all 194 loading unit tests, all unaffected integrations,
locked core check, rebuilt CLI and formatting/diff gates pass. The one
full-suite failure remains the known baseline-stale `@external` diagnostic
ordering assertion. Archive hygiene reports only its three retained thoughts
paths. Final growth is 23 production and 101 proof additions, inside all
packet caps; independent correction review returned `ACCEPT`.

Source order next reaches `rustfmt_test = rule(...)` at lines 218-243. The
common lint attributes are already accepted, but `targets` uses a label-list
descriptor whose `doc`, two-alternative provider predicate, attached private
aspect and custom `platform_transition` exceed Slug's current label-list
constructor surface. Run only the docs audit before retaining or applying any
of those facts.

Pinned Zabel `c7298478…` remains architectural guidance only. Its one
declaration-owned dependency schema retains provider predicates, aspect
identities and transition provenance together, while configured capture
detaches those facts from evaluator values. The audit may use that ownership
split as guidance, but no Zig code, representation, behavior, cache or
analysis algorithm may be copied; pinned Bazel 9.2 remains sole behavior
authority.

### M7 rustfmt test-aspect provides audit accepted; loading selected (2026-08-26)

Commit `df654bfb` selected the advertised-provider audit. Pinned Bazel 9.2
proves `provides` is validated at aspect declaration: each value must be a
provider exported at top level, and its producer `Provider.Key` is retained in
an immutable set. `StarlarkDefinedAspect` includes that set in equality/hash
and only transfers it to advertised-provider enforcement during later
definition/application work.

The fixed singleton therefore needs no provider object retention or analysis
consumer. Slug can clone the already-accepted
`@@dep+//rust/private:rustfmt.bzl%RustfmtTestInfo` `ProviderId` into the
existing frozen aspect owner. Explicit empty, duplicate or wider lists remain
outside the admitted call; application and verification that the implementation
returns its advertised provider remain deferred.

Pinned Zabel `c7298478…` guides only this owner shape: its complete
`AspectDefinition` retains `provides` and follows it during module freeze
while keeping aspect export identity separate. Slug reuses its own
`ProviderId`, Arc slice and `Allocative`; no Zig code, behavior,
representation, cache or analysis algorithm is copied. Bazel 9.2 remains the
sole behavior authority.

Run only `WP-4-7A-rustfmt-test-aspect-provides-loading`. Exact compatibility
is limited to the fixed singleton exported provider and declaration freeze.
Rust storage and diagnostics are Slug-native.
Provider production/matching, application/propagation, configured
dependencies/fragments/toolchains, actions, the later rule, M8/M7B and exact
output identity remain unsupported/deferred.

### M7 second rustfmt aspect accepted; test-aspect provides audit selected (2026-08-26)

Commit `275e0b24` freezes `rustfmt_aspect` with the two fixed private Label
schemas and the complete required `rustfmt_srcs_aspect` producer object.
`_config` retains
`@@dep+//rust/settings:rustfmt.toml` plus single-file policy;
`_process_wrapper` retains
`@@dep+//util/process_wrapper:process_wrapper` plus exec/executable policy.
Both required-provider IDs remain owned by `providers.bzl`, both
implementations remain lazy, and no aspect is applied.

Focused proof, all 193 loading unit tests, all 37 unaffected integrations,
locked core check, rebuilt CLI and hygiene pass. The one full-suite failure is
the same baseline-identical stale `@external` assertion documented by the
predecessor packets. Final growth is 120 production and 93 proof additions,
within all caps. Independent terminal review returned `ACCEPT` after adding
explicit renamed and wider attribute-dictionary rejection cases.

Source order accepts the documented `RustfmtTestInfo` provider and string-list
constant, then skips two lazy implementation bodies. The next aspect's
implementation, three `attr_aspects`, single exported required edge and
documentation are accepted; its first missing argument is
`provides = [RustfmtTestInfo]` at line 214. Run only the docs audit before
implementation or provider matching.

Pinned Zabel `c7298478…` remains architectural guidance only: its complete
producer-owned aspect definition retains advertised provider values separately
from aspect export identity. The audit must determine whether Slug can reuse
its existing `ProviderId` and frozen aspect lifetime without a registry or
consumer rebinding. No Zig code, behavior, cache or analysis algorithm may be
copied; Bazel 9.2 remains the sole behavior authority.

### M7 second rustfmt aspect audit accepted; loading selected (2026-08-26)

Commit `d66059ac` selected the source-order audit. Pinned Bazel 9.2 proves
`aspect(attrs)` builds and retains implicit attributes after requiring their
defaults, while `requires` retains the required aspect object and derives its
class only during later definition construction. Duplicate/cycle path checks
belong to applied-aspect assembly, not this declaration-only slice.

The fixed `_config` and `_process_wrapper` descriptors are private Labels.
Their already-typed defaults remain owned by the rustfmt defining module;
single-file, exec-configuration and executable policy survive independently.
The required value is the already first-exported `rustfmt_srcs_aspect`, so the
consumer must freeze that complete producer object instead of reconstructing a
class key or importer identity.

Pinned Zabel `c7298478…` guides this ownership shape only: its complete
`AspectDefinition` retains named attributes and the required value, while a
separate `AspectExportIdentity` records producer module plus first exported
name and module freeze follows the required child. Slug will reuse its existing
frozen attribute schema and aspect value lifetime; no Zig code, behavior,
representation, cache or analysis algorithm is copied. Bazel 9.2 remains the
sole behavior authority.

Run only `WP-4-7A-rustfmt-second-aspect-loading`. Exact compatibility is
limited to the two fixed descriptors and one exported required producer edge.
Existing Arc/compact/frozen storage and public underscore names are
Slug-native. Public/wider attributes, multiple required aspects, cycle
observability, aspect class derivation, application/propagation, configured
dependencies/fragments, actions, later rustfmt declarations, M8/M7B and exact
output identity remain unsupported/deferred.

### M7 first rustfmt aspect requirements accepted; second aspect audit selected (2026-08-26)

Commit `d4d4d6dc` extends the existing frozen aspect owner with exactly two
singleton required-provider alternatives and the fixed `cpp` fragment. A
three-module recursive proof preserves
`@@dep+//rust/private:providers.bzl%CrateInfo` and `TestCrateInfo` through
`common.bzl`'s `rust_common` struct and the consuming rustfmt module, while the
aspect implementation remains lazy. Flat, wider, mixed, empty-inner,
unexported and non-provider predicates plus non-`cpp` fragments reject.

Focused proof, all 192 loading unit tests, all 37 unaffected integrations,
locked core check, rebuilt CLI and hygiene pass. The one full-suite failure is
the same baseline-identical stale `@external` assertion documented by the
predecessor packet. Final growth is 63 production and 96 proof lines, within
all caps. Independent terminal review returned `ACCEPT` after tightening the
converter from arbitrary nested predicates to the exact two-singleton shape.

Source order now skips the lazy implementation at lines 129-150 and reaches
the second rustfmt aspect at lines 152-192. Its first missing argument is the
fixed `attrs` dictionary; the same call later adds the first
`requires = [rustfmt_srcs_aspect]` edge. Run only the docs audit before
implementation. Pinned Bazel 9.2 remains sole behavior authority. Pinned Zabel
`c7298478…` guides only reuse of one complete producer-owned aspect definition,
distinct aspect export identity and imported provider/aspect ownership; no Zig
code, behavior, cache or analysis algorithm may be copied.

### M7 lint-test common attributes accepted; first rustfmt aspect selected (2026-08-26)

Commit `2cbdb148` accepts the fixed `attr.bool(doc = ...)` call through the
existing validation-only path and freezes both lint-test scalar label defaults.
The raw `@bazel_tools` string resolves through the defining module's immutable
mapping, while the typed no-colon runner Label remains
`@@dep+//rust/private/lint_test_runner:lint_test_runner`. Focused tests, the
remaining loading integrations, core check, rebuilt CLI and hygiene pass. The
one full loading-suite failure is baseline-identical at `5e9039fe`: an older
test expects a later rule/toolchain failure but now stops first on its absent
`@external` repository mapping. Independent terminal review returned `ACCEPT`.

Source order returns to `rust/private/rustfmt.bzl`. Its functions remain lazy;
the `RustfmtTargetInfo` provider already constructs. The first unsupported call
is `rustfmt_srcs_aspect = aspect(...)` at lines 119-127, specifically
`required_providers`, followed immediately by `fragments = ["cpp"]`. Pinned
Bazel 9.2 retains the nested provider predicate and immutable fragment set in
the aspect declaration without running its implementation.

Pinned Zabel `c7298478…` guides the architecture only: its complete
producer-owned aspect definition retains provider requirements and fragments
alongside, but distinct from, first-export aspect identity; imported provider
identities are not rebound by the consumer. Slug will reuse its existing
`ProviderId` and frozen aspect lifetime, with no copied Zig code, behavior,
cache or analysis rule. Bazel 9.2 remains sole behavior authority.

Run only `WP-4-7A-rustfmt-first-aspect-requirements-loading`. Exact
compatibility is limited to the fixed nested two-alternative predicate, the
fixed `cpp` fragment, producer provider identities, recursive freeze/export and
lazy implementation. Rust Arc/compact representation and diagnostics are
Slug-native. Flat/native/wider predicates, other fragments, aspect application,
provider matching, configured fragments, toolchains/actions, later rustfmt
declarations, M8/M7B and exact output identity remain unsupported/deferred.

### M7 post-rust-analyzer audit selects defining-module scalar label defaults (2026-08-26)

Commit `e71db43e` records the accepted detect-sysroot packet and selects the
docs-only recursive source-order audit. Slug computes external `.bzl` children
serially in resolved load order and returns on the first child failure. After
`rust/private/rust_analyzer.bzl:484` completes, `rust/toolchain.bzl:11-14`
selects `rust/private/rustfmt.bzl`; its first child `common.bzl` is already
complete from the accepted rust-analyzer closure, so its next new child is
`rust/private/lint_test.bzl`.

The transition at `lint_test.bzl:37-41` and documented `platform` label at
lines 46-48 already load. The first unsupported expression is the `doc`
argument on `attr.bool` at lines 49-52; Slug's bool descriptor lacks the
already-shared validation-only documentation parameter. Once admitted, the
next unsupported expression is the raw external string default at lines 53-55:
`@bazel_tools//tools/allowlists/function_transition_allowlist`. Slug currently
reduces label defaults to a package-only raw converter, which rejects `@` and
has discarded the defining repository mapping. Fixing only that string would
stop immediately at lines 56-60 because the adjacent `_runner` default is an
already-constructed Starlark `Label`, which the raw-value adapter also rejects.
The selected packet therefore admits that one fixed bool-documentation call
plus exactly these two scalar label forms and completes this module.

Pinned Bazel 9.2 `StarlarkAttrModule`, `Attribute.Builder`,
`BuildType.LabelType` and `LabelConverter.forBzlEvaluatingThread` establish the
fixed distinction: a string default is parsed with the innermost defining
`.bzl` package context and repository mapping, while a `Label` value is
retained unchanged. Focused rule-class and Bzl-load tests authenticate
declaration-time conversion, remote-string conversion and defining-module
mapping. Neither target lookup nor implementation execution occurs here.

Pinned Zabel `c7298478…` guides only the architecture. Its retained declared
label-default spelling and captured canonical Label paths reinforce one
producer-owned typed default: resolve/rebase strings at the defining module,
preserve canonical Label values, and do not defer repair to a consuming BUILD
package. No Zig code, representation, mapping behavior, evaluator rule or DICE
relation is copied; Bazel 9.2 remains sole behavior authority.

Run only `WP-4-7A-lint-test-label-default-loading-r3`. Reuse
`discard_attribute_doc` for the fixed bool descriptor plus the complete
`BzlModuleIdentity`, shared label resolver, `StarlarkLabel` and existing owned
`CoercedAttributeValue::Label`; add no map, cache, lookup, I/O, hash domain or
lifetime owner. Exact compatibility is limited to validation/acceptance of the
fixed bool doc, scalar label-default string and Label inputs, their
defining-module identity, canonical freeze/export and the fixed lint-test
dictionary. Existing Rust enum/Arc storage and diagnostics are Slug-native.
Documentation retention/extraction, label lists/dicts, computed or late-bound defaults, target
invocation, transition allowlist/application semantics, rustfmt aspects,
configured dependencies, providers, actions, M8/M7B and exact output identity
remain unsupported/deferred.

The first implementation attempt exposed a proof-harness boundary rather than
a missing production mapping. The selected loading fixture deliberately names
its synthetic root module `bazel_tools`; consequently that fixture maps the
apparent built-in name to the root. Renaming it activates the complete pinned
`@bazel_tools` MODULE dependency closure and first requests absent
`rules_license` registry evidence, while an explicit override reaches Slug's
existing unsupported `ExplicitBuiltinOverride` boundary. Do not grow the
fixture or alter mapping behavior for this packet. Compose the already-accepted
Bzlmod proof that a real selected non-root route resolves `bazel_tools` to the
built-in snapshot with a focused caller-aware loading context that freezes the
exact lint-test dictionary. Keep the recursive selected fixture for the
`rules_rust -> dep+` producer/Label path. This proof correction adds no code
owner and does not change the compatibility classification.

The corrected proof then exposed a second material contract error before any
Rust was retained. Pinned Bazel 9.2 `LabelValidator.parseAbsoluteLabel` and
`LabelParserTest.parserTable` prove that a no-colon absolute label uses the
whole post-`//` path as its package and the last path segment as its implicit
target. Therefore `Label("//rust/private/lint_test_runner")` is exactly
`@@dep+//rust/private/lint_test_runner:lint_test_runner`, not
`@@dep+//rust/private:lint_test_runner`. Pinned Zabel's separate retained
package-path/target-name projection reinforces that owner shape as
architectural guidance only. This second contract correction requires
`REPLAN`; the stopped `-r2` packet retained the same two-file implementation
boundary and caps.

The exact fixed-dictionary test for `-r2` then failed earlier than either label
default: Slug rejects `attr.bool(doc = ...)` as an extra named parameter.
Pinned Bazel 9.2 `StarlarkAttrModuleApi.boolAttribute` admits a string-or-None
`doc`, and `StarlarkAttrModule.boolAttribute` passes it into the common
attribute factory. The accepted rules_rust source supplies the fixed string at
lines 49-50. No Rust from the stopped attempt is retained. `REPLAN` to `-r3`:
validate/discard exactly this documentation through Slug's existing helper,
then perform the unchanged two label-default conversions under the same files
and caps. Pinned Zabel remains architectural guidance only and contributes no
documentation behavior.

### M7 detect-sysroot rule accepted; recursive frontier audit selected (2026-08-26)

Commit `129ff448` exposes the already-pure apparent-label resolver only within
`slug_loading_v2` and reuses it solely for raw single-`@` strings in
`rule(toolchains = ...)`. Canonical `@@...` and existing relative branches are
unchanged. No mapping, key, cache, lookup, I/O or lifetime owner was added.

The selected-registry proof recursively freezes
`rust_analyzer_detect_sysroot` with exactly
`@@dep+//rust:toolchain_type` followed by
`@@dep+//rust/rust_analyzer:toolchain_type`, while its failing implementation
remains lazy. The prior current-toolchain rule remains frozen, and missing or
ambiguous apparent mappings now reject through the raw rule-string path.

Focused proofs and all 256 loading tests pass. Locked core check, rebuilt CLI,
formatting and diff gates pass; archive status retains only its known three
thoughts paths. Growth is 7 production and 33 proof additions, 40 total, within
every cap. Independent terminal review returned `ACCEPT`.

Pinned Zabel `c7298478…` guided reuse of the immutable defining-module context
and pure thin canonical projection only. Its native BUILD `toolchain(...)`
resolver supplied no behavior or code; Bazel 9.2 remained sole authority.
Exact compatibility covers the fixed two string conversions, mandatory policy,
order, recursive freeze, doc value and export. Existing Arc representation and
diagnostics are Slug-native; invocation, `ctx.toolchains`, selection, provider/
path semantics, JSON action and returned `DefaultInfo` remain deferred.

The accepted file ends at line 484. Source order now returns to
`rust/toolchain.bzl`, whose next load is `//rust/private:rustfmt.bzl`; that
module recursively loads `common.bzl` and `lint_test.bzl` before its own
provider/aspect/rule declarations. Some children may already be memoized from
the accepted closure. Run only the docs audit to replay the actual recursive
manifest/source order, distinguish cached children from newly evaluated ones,
and name the first unsupported expression. Pinned source, not a guessed
rustfmt declaration, determines the next implementation packet.

### M7 current-toolchain rule accepted; detect-sysroot rule loading selected (2026-08-26)

Commit `61cb0ad0` carries the selected route's existing repository-mapping Arc
into every recursive external `BzlModuleIdentity`, its equality/hash and the
manifest fingerprint. The evaluator's existing typed native-call source first
and `DefInfo` fallback now select the complete defining identity. The shared
`.bzl` Label resolves only the admitted `@name//package:target` form through
that immutable mapping and fails closed on absent or conflicting entries.
`str(Label(...))` hands one canonical direct target to the existing frozen rule
requirement owner without changing raw apparent string behavior.

A selected-registry proof deliberately separates root apparent name
`dep_alias`, module-local self-name `rules_rust` and canonical repository
`dep+`. It recursively freezes the exact current-toolchain declaration with
one `@@dep+//rust/rust_analyzer:toolchain_type` requirement while its
implementation remains lazy. Mapping changes discriminate identity and
fingerprint, and an ambiguous mapping rejects.

Focused proofs, all 545 `slug_bzlmod_v2` unit tests and its integration suites,
and all 256 `slug_loading_v2` tests pass. Locked core check, rebuilt CLI,
formatting and diff gates pass; the archive audit retains only its known three
thoughts paths. Growth is 115 production and 85 proof additions, 200 total,
within every cap. Independent review rejected the first layout because a
touched test exceeded 150 lines; extraction reduced it to 142 lines and the
terminal re-review returned `ACCEPT`.

Pinned Zabel `c7298478…` guided only the immutable per-defining-module mapping,
currently executing module lookup and thin canonical declaration projection.
No Zig code, mapping behavior, representation, evaluator or DICE relation was
copied; Bazel 9.2 remains sole behavior authority. Exact compatibility is the
fixed selected-registry lookup, canonical handoff, mandatory requirement,
recursive freeze and export slice. Arc retention, complete-map
over-invalidation and fingerprint framing are Slug-native; every wider mapping,
toolchain, invocation and analysis surface remains unsupported/deferred.

Pinned-source order next traverses the lazy
`_rust_analyzer_detect_sysroot_impl` body at lines 431-473 without executing it,
then evaluates `rust_analyzer_detect_sysroot = rule(...)` at lines 475-484. Its
two distinct string requirements at lines 478-479 are raw apparent-self labels.
Pinned Bazel `LabelConverter.forBzlEvaluatingThread` and
`parseToolchainTypes` resolve both through the defining module's package
context, mark plain strings mandatory and preserve first-label order. Slug now
owns that exact mapping but its rule converter still rejects raw apparent
strings. Run only the selected packet to reuse the shared pure resolver for
these two strings and retain their ordered canonical requirements. The
implementation body, `ctx.toolchains`, fail paths, provider fields, path
operations, JSON action and returned `DefaultInfo` remain lazy and deferred.

Pinned Zabel guidance selects the existing immutable module context and pure
Label-resolution leaf rather than another mapping owner. Its native BUILD
`toolchain(...)` resolver is not a behavioral analogue for
`rule(toolchains = ...)`; only the explicit-input/thin-canonical-projection
shape applies.

### M7 current rust-analyzer toolchain-rule audit selects defining-module mapping (2026-08-26)

Pinned Bazel 9.2 `BazelModuleContext`,
`LabelConverter.forBzlEvaluatingThread`, `Label.parseWithPackageContext`,
`StarlarkRuleClassFunctions.parseToolchainTypes`, and focused Bzl-load/Label/
rule-toolchain tests establish the fixed call. The shared Label builtin uses
the innermost executing `.bzl` module's selected repository mapping, including
an explicit self-name entry; `str(Label(...))` produces canonical `@@...`
spelling. A plain string requirement is mandatory, and ordered first-label
deduplication does not change the fixed one-element list.

Slug's selected-registry route already owns the ordered apparent-to-canonical
mapping and includes it in route equality/hash. Recursive child routes already
select each child's own mapping. The gap is downstream: `BzlModuleIdentity`,
the recursive manifest and evaluator context retain only label/path, while the
bounded Label builtin rejects explicit repositories and the rule-toolchain
converter cannot accept the resulting canonical string.

Run only `WP-4-7A-current-rust-analyzer-toolchain-rule-loading`. Reuse the
route's existing mapping Arc in each frozen module identity, include it in
manifest fingerprinting, select the full defining identity at native-call
source/`DefInfo` resolution, admit only mapped `@name//package:target` Label
construction, and accept the canonical `str(Label(...))` handoff in the
existing frozen rule requirement owner. Missing/conflicting mappings fail
closed. Direct apparent rule-toolchain strings, wider Label forms, target
invocation, `ctx.toolchains`, selection, analysis and later declarations remain
deferred.

Exact compatibility covers the fixed selected-registry apparent-self lookup,
canonical handoff, one mandatory direct requirement, recursive freeze and
producer export identity. Arc storage, complete-mapping over-invalidation,
fingerprint framing and nonrequired diagnostics are Slug-native. Other mapping
producers and the wider toolchain API remain unsupported/deferred.

Pinned Zabel `c7298478…` guided only the architecture: retain immutable
canonical repository plus apparent mapping with the defining module, let a
shared Label builtin consult the currently executing module, and project one
canonical declaration result. Its native toolchain declaration is not treated
as the behavior analogue. No Zig code, representation, mapping rule, evaluator
or DICE relation is copied; Bazel 9.2 remains sole behavior authority.

### M7 rust-analyzer toolchain declaration accepted; apparent-self Label audit selected (2026-08-26)

Commit `eda81a4d` loads and recursively freezes the complete fixed
`rust_analyzer_toolchain = rule(...)` declaration. Label and string docs accept
omission, strings and `None` and are discarded outside the deferred
documentation-extraction surface. Executable and exec-transition policy are
distinct booleans in the existing descriptor/frozen rule-schema owner;
mandatory, single-file, typed defaults and custom transitions retain their
existing owners. Omitted and explicit-false executable values remain identical.

Rules carrying executable-true or exec-configured attributes reject before
`PackageRecorder` can record a target. Existing non-executable custom-transition
invocation remains accepted. Recursive proof discriminates exec with omitted
executable from executable with a retained custom transition. External default
coercion now consumes the accepted caller-aware canonical source projection
instead of reparsing an already-canonical repository label.

Focused tests and all 256 loading tests pass; locked core check, rebuilt CLI,
formatting and hygiene pass. The archive audit retains only its known three
thoughts paths. Growth is 96 production, 134 proof and 230 total, within every
cap; independent review returned `ACCEPT` after requiring the custom-transition
freeze discriminator.

Pinned Zabel `c7298478…` guided keeping executable, single-file and transition
policy in one declaration schema separated from target-local values; no Zig
layout, code, DICE relation or behavior was copied. Bazel 9.2 remains sole
authority. Exact compatibility is the fixed declaration call, retention,
freeze and export slice. Rust storage, discarded docs and fail-closed invocation
are Slug-native; configured exec dependencies and analysis remain deferred.

Source order next reaches `current_rust_analyzer_toolchain = rule(...)` at lines
423-429. Its implementation body at lines 404-421 remains lazy, but declaration
evaluation calls
`Label("@rules_rust//rust/rust_analyzer:toolchain_type")` and passes its string
at line 427 to the lines 426-428 `rule(toolchains = ...)` list. The bounded Label
surface rejects explicit
repositories and Slug's current rule-toolchain converter lacks a complete
defining-module repository mapping. Run only the docs audit
`WP-4-7A-current-rust-analyzer-toolchain-rule-audit` before changing that
identity boundary.

### M7 rust-analyzer toolchain-rule audit selects fail-closed declaration loading (2026-08-26)

Pinned Bazel 9.2 `StarlarkAttrModuleApi`, `StarlarkAttrModule.createAttribute`
and `convertCfg`, `StarlarkRuleClassFunctions.createRule`, and focused
`StarlarkRuleClassFunctionsTest` rows establish the complete fixed call. Attribute
`doc` is named `string | None`, trimmed and retained as nonsemantic documentation;
`executable = True` requires a non-`None` `cfg`; `cfg = "exec"` installs the
execution transition; `allow_single_file = True` independently retains the
single-artifact/file predicate; `mandatory = True` is declaration policy; and
the two string defaults are retained typed values. The exported rule remains
owned by its defining `.bzl` and implementation. Bazel performs these descriptor
validations during declaration construction, before any target is invoked.

Slug already retains mandatory, single-file, default, custom-transition and
rule export/freeze state. Its first unsupported argument is label-attribute
`doc`; after accepting that metadata shape, `cfg = "exec"` currently fails
because `cfg` accepts only a custom transition, and `executable` is absent.
The bounded implementation adds two booleans to the existing declaration-owned
schema—executable policy and an exec-transition marker—while preserving the
custom-transition owner. Omitted and explicit-false executable values are the
same retained false policy, including with exec or custom cfg; true requires
one admitted non-`None` cfg. Docs are validated and discarded consistently
with the accepted provider/rule-doc loading slices. Any target invocation of a
rule carrying true executable policy or the exec marker fails before
`PackageRecorder` records a target, so configured exec semantics cannot
silently degrade to target identity. Existing non-executable custom-transition
invocation remains unchanged.

This is exact only for the fixed definition call, validation, typed retained
schema/defaults, recursive freeze and producer export identity. Compact Rust
fields, fail-closed invocation and nonrequired diagnostics are Slug-native.
Documentation extraction, `cfg = "target"`/`None` and wider descriptor forms,
target invocation for either newly gated policy, executable prerequisite
validation, execution-platform configuration, analysis/actions, later
rust-analyzer declarations, M8/M7B and exact output bytes remain
unsupported/deferred.

Pinned Zabel `c7298478…` guided the architecture: its ordinary-dependency facts
keep executable, single-file and dependency-transition policy together in one
declaration-owned schema, distinct from target-local values; its declaration
owner and executable-module identity relations remain separate. Slug adopts
only that single-owner/thin-projection lesson. No Zig code, representation,
parser, evaluator, DICE relation or behavior is copied; Bazel 9.2 remains sole
behavior authority. The Buck2 utility audit selects no import because two
booleans extend an existing compact schema and add no allocation, collection,
interner or hash domain.

### M7 bounded Bazel `Label` loading accepted; toolchain-rule audit selected (2026-08-26)

Commit `84ddb6a3` adds `Label` only to complete `.bzl` globals and shares one
`CanonicalLabel`-owned Starlark value with module-extension evaluation. The
admitted constructor accepts `//...`, `:...` and Label idempotence, preserves
the existing narrow value surface, completes the fixed aspect toolchain
expression and keeps BUILD aliases rejected. Bare and explicit-repository
spellings, mapping and wider APIs remain deferred.

Recursive provenance uses the exact byte-preserving parser source-name
projection retained by `BzlLoadManifest`. The typed native call-expression
source takes precedence over a surviving outer `DefInfo`, so an imported
function inlined inside a non-inlined caller still resolves to its defining
module; non-inlined definitions retain the typed `DefInfo` fallback. Missing
or ambiguous mappings fail closed. The same source-name helper now owns Host,
external and legacy local parser naming.

Focused Label/aspect/runtime proof, all 254 loading tests, locked core check,
rebuilt CLI, formatting and diff gates pass. The archive checker retains only
its known three-path thoughts classification. Final growth is 295 production,
134 proof and 429 total, within every file/function/packet cap. Independent
review returned `ACCEPT` after requiring byte-preserving provenance and the
nested cross-package inlining discriminator.

Pinned Zabel `c7298478…` guided the single retained Label owner and thin
definition-context projection; no parser, mapping, runtime or behavior was
copied. Bazel 9.2 remains sole authority. Exact compatibility is limited to
the admitted constructor/value/fixed-aspect slice; Rust representation and
diagnostics are Slug-native; wider Label/aspect behavior is unsupported or
deferred.

The accepted source-order closure now reaches
`rust/private/rust_analyzer.bzl:359`, where
`rust_analyzer_toolchain = rule(...)` contains four label attributes using
`doc`, `cfg = "exec"`, `executable`, `allow_single_file` and `mandatory`, plus
two documented string attributes with defaults. Run only the docs audit
`WP-4-7A-rust-analyzer-toolchain-rule-audit`. Pinned Zabel's retained ordinary
dependency schema and executable-module/declaration-owner split are
architecture guidance only; the audit must authenticate behavior against
Bazel 9.2 and stop before implementation, target invocation or analysis.

### M7 `Label` audit accepted; bounded loading packet selected (2026-08-26)

The audit selected one typed implementation rather than outer-evaluator
guessing. The vendored Rust Starlark runtime already retains each `def`'s
definition `CodeMap`; expose only its filename to a directly called native
builtin. `BzlLoadManifest.reachable` already maps those exact logical source
paths to canonical module labels, so `BzlEvaluationContext` can resolve an
imported function to its defining `.bzl` while a direct alias at module scope
uses the outer top-level module. Missing provenance fails closed. BUILD has no
Bzl context and remains rejected.

Move the accepted module-extension Label wrapper to one shared loading-owned
module instead of duplicating canonical identity or its exact str/repr/hash/
equality and narrow property surface. Admit string `//...` and `:...` inputs
plus Label idempotence; defer bare, explicit-repository and wider APIs. The
fixed aspect adapter additionally accepts only the resulting canonical string
when it names the defining repository. No repository mapping is guessed.

Pinned Zabel `c7298478…` guided the retained-value/shared-builtin split and the
executing-definition context rule; its parser, mapping observer, runtime and
storage are not reused. Bazel 9.2 remains sole behavior authority. Run only
`WP-4-7A-bazel-label-global-loading`.

### M7 fixed aspect definition accepted; `Label` audit selected (2026-08-26)

Commit `840d28e7` adds `aspect` only to complete `.bzl` globals and retains the
admitted implementation lifetime, six ordered propagation attributes, one
canonical direct-string toolchain requirement, defining module and first
producer export name through recursive freeze/import. BUILD remains unable to
resolve or invoke the builtin, including through an imported factory. Native
callables, malformed fixed lists, unsupported parameters and false export
identity reject or remain absent.

Focused proof passes 3/3 and all 251 loading tests pass; locked core check,
rebuilt CLI, formatting, diff hygiene and the known archive baseline pass.
Final growth is 153 production, 120 proof and 273 total, within every cap.
Independent terminal review returned `ACCEPT` after requiring a true
user-defined Starlark function and direct inspection of the nested unexported
definition.

The live rules_rust expression now reaches
`str(Label("//rust:toolchain_type"))`. Run only docs packet
`WP-4-7A-bazel-label-global-audit`. It must authenticate `.bzl` placement,
innermost executing-function defining-module context, canonical repository
ownership, value stringification/identity, BUILD re-export rejection and the
exact boundary before apparent-repository mapping or wider Label APIs. It must
distinguish a top-level call, a direct builtin alias and an imported function
containing `Label`, and `REPLAN` if Slug has no typed frame provenance. Pinned Zabel
`c7298478…` is concept/test guidance for keeping retained canonical identity
with the value and resolving through the executing function's defining module
rather than the outer evaluator or builtin exporter; Bazel 9.2 remains sole
behavior authority.

### M7 repeatable StringList accepted; post-descriptor audit selected (2026-08-26)

Commit `573c25c7` exposes named-only `config.bool(flag = True)` only through
the complete `.bzl` config projection, keeps it absent from BUILD, and replaces
the prior string-only marker with a compact String/Boolean kind retained
through rule definition, freeze, equality and typed default-schema selection.
Boolean rule invocation fails before target recording. Both BUILD and `.bzl`
string projections share one private constructor, following the single-owner,
thin-projection architecture selected from pinned Zabel guidance.

Focused proof passes 3/3 and all 247 loading tests pass; core check, rebuilt
CLI, formatting and diff checks pass. The packet lands within its 120/110/230
caps at 116/110/226. Independent terminal review returned `ACCEPT` after
requiring the shared string constructor and exact named-only boolean ABI. The
archive audit preserves only its known three-path thoughts classification.

Pinned Bazel 9.2 defines `config.string_list` with named-only `flag` and
`repeatable`, both defaulting to `False`, creates a `STRING_LIST` descriptor,
and rejects repeatability without `flag = True`. The accepted rules_rust
archive reaches nonrepeatable uses at `rust/private/rustc.bzl:3093` and `:3108`
before the first `repeatable = True` use at `:3120`. Repeatability therefore
cannot be omitted from any widened semantic-identity claim.

Commit `6811fa84` accepts omitted/explicit-false nonrepeatable StringList only
in `.bzl`, retains it distinctly from String/Boolean through freeze/equality
and list schema, keeps BUILD string-only, and rejects every list target before
recording. Focused proof, all 248 loading tests, locked core check, rebuilt CLI,
hygiene and archive baseline pass. Public query/build retain their known
repository-session wrappers. Final growth is 34 production, 89 proof and 123
total; independent terminal review returns `ACCEPT` after adding explicit
`flag=False` rejection.

Commit `68e458b4` accepts `repeatable=True` by placing one boolean on the
existing evaluation descriptor and retained StringList variant. False/true
definitions compare unequal while sharing list schema; every list target still
fails before recording. Focused proof and all 248 loading tests pass, with
locked core check, rebuilt CLI, formatting, archive baseline and independent
terminal review. Final growth is 14 production, 23 proof and 37 total.

Fresh query/build retain the public repository-session wrappers, which do not
expose the next internal source-order stop. The source-order audit at
`a8e18278` authenticates `rust/private/rust_analyzer.bzl:207` as that stop:
its fixed `rust_analyzer_aspect = aspect(...)` follows the accepted recursive
children and precedes the file's later rules. Run only
`WP-4-7A-bazel-aspect-definition-loading`, retaining the fixed constructor
subset and first producer export identity but no `Label` or application
semantics. Pinned Zabel
`c7298478…` directly guides the complete declaration/export owner and thin
projection split; no code or behavior is copied. Bazel 9.2 remains sole
authority and M7A -> M8 -> M7B is unchanged.

### M7 Bazel `rule(doc=...)` support accepted; config-bool frontier active (2026-08-26)

Commit `6ab6f35d` accepts omitted, string and explicit `None` rule docs at the
existing call-shape adapter, rejects other values, and deliberately retains no
documentation. Frozen schema, capability and equality remain unchanged.
Focused tests pass 2/2, all 244 loading tests pass, locked core check, rebuilt
CLI, formatting and hygiene pass, and independent terminal review returned
`ACCEPT` within every packet cap.

Fresh disposable rules_rust query and build pass the documented
`rust_lto_flag` plus `error_format` string build-setting declarations. Their
public terminals remain the existing repository-session wrappers
(`query_error` exit 7 and `build_runtime_error` exit 2). Source order next
reaches `rust/private/rustc.bzl:3047-3055`, where
`always_enable_metadata_output_groups` uses `config.bool(flag = True)`; a
second boolean descriptor follows before the first `config.string_list` use.

Pinned Bazel 9.2 `StarlarkConfigApi`, `StarlarkConfig`, `BuildSetting`,
`RuleClass.Builder` and `ConfigSettingTest.buildsettings_convertedType`
establish a named-only boolean `flag`, a typed BOOLEAN descriptor and a
mandatory boolean `build_setting_default`. Slug currently retains only a
string-specific bit, so treating bool as string or as a second independent bit
would weaken equality. Bazel registers `ConfigBootstrap` for `.bzl` files, not
BUILD; Slug's current BUILD string-only config projection must not gain bool.

Run only `WP-4-7A-bazel-config-bool-loading`: replace the string-only bit with
one compact string/boolean kind across rule-definition freeze and equality,
derive the typed default schema, expose bool only through `.bzl` globals, and
reject boolean invocation before target recording. Exact compatibility is
limited to `.bzl` placement, BUILD absence, the live `flag=True` definition
load and typed schema/freeze. Rust enum/storage, fail-closed invocation error
and diagnostics are Slug-native; omitted/False descriptors, boolean
targets/analysis/CLI values, transitions/config matching, other config methods,
later rules_rust semantics, M8/M7B and exact output bytes remain
unsupported/deferred.

Pinned `../zabel` `c7298478…` guides the complete typed config owner and narrow
schema/string projections only. No Zabel code, representation, scheduler or
behavior is copied; Bazel 9.2 remains sole behavior authority. The Buck2
utility audit selects no import because the compact enum replaces one bool and
adds no collection, string, interner or allocation.

### M7 Bazel `provider(doc=...)` support accepted; rule-doc frontier active (2026-08-26)

Commit `a81b5823` accepts omitted, string and explicit `None` provider docs at
the existing global adapter, rejects other values, and deliberately retains no
documentation. Frozen provider schema plus source-label/exported-name identity
remain unchanged. Focused tests pass 2/2, all 242 loading tests pass, locked
core check, rebuilt CLI, formatting and hygiene pass, and independent review
returned `ACCEPT` after the diff was reduced within every packet cap.

Fresh disposable rules_rust query and build advance through all 18 provider
declarations. Their public terminals remain the existing repository-session
wrappers (`query_error` exit 7 and `build_runtime_error` exit 2). The accepted
source/load-order trace identifies the next internal declaration at
`rust/private/lto.bzl:40`: `rust_lto_flag = rule(doc = ...,
build_setting = config.string(flag = True), ...)`.

Pinned Bazel 9.2 `StarlarkRuleFunctionsApi.rule`,
`StarlarkRuleClassFunctions.createRule`,
`StarlarkRuleClassFunctionsTest.testRuleDoc`, `RuleClass` and
`RuleInfoExtractor` establish that named-only `doc` is `string | None`,
defaults to `None`, is trimmed and retained for separate documentation
extraction. Slug's frozen rule owner already contains every admitted
build-semantic field and has no documentation consumer.

Run only `WP-4-7A-bazel-rule-doc-loading`: consume and validate `doc` at the
existing rule adapter, preserve the frozen schema/capability, prove recursive
freeze and do not admit another rule parameter. Exact compatibility is call
acceptance/type rejection on the live loading route. Rust storage and
nonrequired diagnostics are Slug-native; doc retention/extraction, other rule
parameters, broader provider/rule analysis, toolchains/actions, M8/M7B and
exact output bytes remain unsupported/deferred.

Pinned Zabel `c7298478…` guides one complete call-shape owner projected to the
existing build-semantic frozen rule, without a metadata side store. It supplies
no rule behavior or representation; pinned Bazel 9.2 remains sole authority.

### M7 Bazel provider `doc` audit accepted (2026-08-26)

Pinned Bazel 9.2 `StarlarkRuleFunctionsApi.provider`,
`StarlarkRuleClassFunctions.provider`, `StarlarkProvider`, and focused
`StarlarkRuleClassFunctionsTest`/`StarlarkProviderTest` rows establish that
`doc` is named `string | None`, defaults to `None`, is trimmed and retained for
external documentation extraction. It is not a Starlark-visible provider
attribute, and exported callable equality/hash remain solely the `.bzl`
label/exported-name key.

The live rules_rust `rust/private/providers.bzl` declares 18 top-level
providers before the next load can complete. Every declaration supplies a
string `doc` plus dictionary `fields`; no `init`, list schema or provider
instance is used at this loading frontier. The completed module must only bind
and freeze the callables before parents store selected ones in `rust_common`.

Slug's existing provider global already validates dictionary field docs as
strings, reduces them to sorted semantic field names, and freezes
`UserProviderCallable` with structural source-label/exported-name identity.
Accept `doc: Option<&str>` at that adapter and deliberately do not add it to the
retained callable: Slug exposes no documentation extractor, Bazel excludes it
from provider identity, and retaining long rules_rust prose would add
nonsemantic graph memory. Exact compatibility is call acceptance/type checking
and unchanged freeze/export behavior for build/query loading. Bazel doc-string
trimming/storage, field documentation and Stardoc extraction remain explicitly
unsupported/deferred.

Pinned Zabel `c7298478…` guides preserving one complete globals owner and a
narrow semantic projection rather than adding a metadata side store. It
supplies no provider behavior; pinned Bazel 9.2 remains sole compatibility
authority.

### M7 Bazel `.bzl` `struct` support accepted; provider frontier active (2026-08-25)

Commit `1a527089` gives every audited `.bzl` evaluator one complete globals
value containing `Print` and retained `StructType`, while both direct BUILD
evaluation routes use the sibling Print-only value. Focused recursive export
and BUILD-exclusion tests pass, all 240 `slug_loading_v2` tests pass, locked
core check and rebuilt V2 CLI pass, and independent review returned `ACCEPT`.

Fresh rules_rust query and build both pass named struct construction, field
reads and recursive freeze/export. They now converge at
`rust/private/providers.bzl:17`, where `CrateInfo = provider(doc = ...,
fields = {...})` reaches Slug's retained provider builtin and rejects `doc` as
an extra named parameter. Public query/build errors retain their existing
typed wrappers.

Run only docs packet `WP-4-7A-bazel-provider-doc-audit`. Authenticate the
pinned Bazel declaration contract for `doc` and `fields`, inspect Slug's
current provider callable ownership, and trace the live rules_rust declarations
through export and first required use. Keep declaration-time callable creation
distinct from later provider instances and configured-analysis semantics.

Exact compatibility remains limited to the accepted `.bzl` environment and
live struct operations. Rust value storage and nonrequired diagnostics are
Slug-native. Broader struct behavior, unauthenticated provider parameters,
provider-instance/analysis breadth, toolchains/actions, M8/M7B and exact output
bytes remain unsupported/deferred.

Pinned Zabel `c7298478…` continues to guide one complete typed globals owner
projected to the correct consumers. It supplies no provider behavior or
representation; pinned Bazel 9.2 remains sole compatibility authority.

### M7 Bazel `.bzl` `struct` implementation selected (2026-08-25)

Pinned Bazel 9.2 `StarlarkGlobalsImpl` places `StructProvider.STRUCT` in fixed
`.bzl`, cquery and SCL globals, but not fixed BUILD, MODULE or REPO globals;
`BazelStarlarkEnvironmentTest` additionally proves BUILD-loaded and
MODULE-loaded `.bzl` files declare the same names. The live rules_rust load
needs named bool construction, `.std`/`.host_tools` field reads and freezing a
dictionary of structs across recursive module export.

Retained starlark-rust already implements that slice through
`LibraryExtension::StructType`, `register_struct`, `StructGen` and its derived
freeze. It diverges outside the selected surface: it orders structs, does not
implement Bazel struct concatenation/provider identity, and renders spacing
differently. Those rows remain unsupported/deferred rather than being promoted
by exposing the builtin.

The bounded successor keeps `package.rs` as the sole loading-global owner,
adds a distinct current BUILD environment, and makes the existing complete
loading environment the `.bzl` environment with only `Print` and `StructType`.
Only BUILD/package evaluations in `bzl_module.rs` switch to the BUILD value;
all `.bzl` routes share the other value. MODULE, REPO, cquery and preliminary
core evaluation are unchanged.

Pinned Zabel `c7298478…` guided the complete typed environment owner and
consumer projection rather than per-evaluator symbol reconstruction. No Zabel
code or behavior is copied; Bazel 9.2 remains sole compatibility authority.

### M7 Bazel keyword-only Starlark support accepted (2026-08-25)

Commit `54d28477` adds one retained Bazel dialect equal to Standard except for
keyword-only arguments and routes exactly the audited BUILD/`.bzl` parsers
through it. MODULE parsing and every unrelated syntax field remain unchanged.
Focused syntax, core and recursive external-Bzl tests pass, as do all 239
loading tests, locked core check, rebuilt V2 CLI, formatting and hygiene.

Fresh query and build both pass rules_rust's `_support(*, ...)` definition and
calls. They converge on the same next internal terminal at
`rust/platform/triple.bzl:28`: `Variable struct not found`; their public
terminals remain the existing typed query/build wrappers. Independent review
accepted the implementation and proof.

Exact compatibility covers the admitted Bazel 9.2 definition/lambda syntax,
parameter ordering, defaults and call binding. Rust storage, valid-Unicode
source ingestion and nonrequired diagnostic wording remain Slug-native;
positional-only parameters and unrelated syntax remain unsupported/deferred.

Pinned Zabel `c7298478…` guided the single complete dialect owner consumed by
all relevant evaluators. No Zabel code or behavior is copied; Bazel 9.2
remains sole compatibility authority.

### M7 Bazel keyword-only Starlark implementation selected (2026-08-25)

The read-only audit traces the rules_rust terminal to
`ExternalBzlModuleEvalKey` and inventories nine Stage 4 parse sites plus the
live preliminary root-BUILD evaluator. Every site currently supplies
`Dialect::Standard`; starlark-rust already parses, resolves, compiles and binds
required/defaulted keyword-only parameters once its single
`enable_keyword_only_arguments` field is set.

Pinned Bazel 9.2 `Resolver`, `StarlarkFunction`, `FunctionTest`,
`ResolverTest` and `ParserTest` authenticate bare `*`, `*args` followed by
keyword-only parameters, ordering failures and the same lambda parameter form.
The bounded successor adds one `Dialect::Bazel` constant equal to Standard
except for that field, then uses it at the audited BUILD/`.bzl` boundaries.
MODULE dialects, positional-only parameters, types, f-strings and top-level
forms remain unchanged or unsupported/deferred.

Pinned Zabel `c7298478…` guided the single complete dialect owner consumed by
all relevant evaluators instead of per-call reconstructed policy. No Zabel
code or behavior is copied; Bazel 9.2 remains sole syntax/call authority.

### M7 selected-BCR archive realization accepted (2026-08-25)

Commit `2f373248` streams the exact rules_rust 0.73.0 verified capture through
a raw bounded Rust gzip/GNU-tar realizer, independently verifies and replaces
the registry MODULE, and returns one complete immutable root through the sole
token-revalidated materializer. Cleanup, stale-drop, same-session reuse and
A/B/A association proofs pass; local archives remain unchanged.

Focused selected-BCR, HTTP and repository tests pass (106 repository rows,
one declared disposable-artifact audit ignored), as do locked core check and
the rebuilt V2 CLI. A direct Bazel/Slug comparison matches all 4,493 paths and
types plus every regular file byte/mode and archive mtime. Fresh query/build
requests advance beyond materialization and stop honestly at rules_rust's
`def _support(*, ...)`, which Bazel 9.2 accepts but Slug's current
starlark-rust `Dialect::Standard` rejects.

Exact compatibility covers the selected URL/SRI/order, archive regular bytes,
modes and mtimes, directory presence, registry MODULE bytes/nonexecutable
result and local archive behavior. Rust streaming ceilings/diagnostics,
valid-Unicode paths, directory metadata, MODULE mtime, source association and
scratch lifetime are Slug-native. Generic archives, PAX/links/specials,
strip/patch/overlay breadth, toolchains/actions and M8 remain deferred.

Pinned Zabel `c7298478…` guided the architecture: integrity-verified captures
stay private, realization builds a fresh owned complete root, semantic content
association is distinct from its physical path, and publication remains with
the existing owner. No Zabel code or behavior is copied; Bazel 9.2 remains the
sole compatibility authority.

### M7 selected-BCR archive realization selected (2026-08-25)

The audit accepts one bounded Rust-native implementation. The evidenced
artifact is 67,196,890 compressed bytes, 224,337,920 gzip bytes and 4,493
logical UTF-8 regular/directory entries; it needs GNU long names, 0644/0755
regular modes and no PAX, links, specials, absolute/parent or duplicate paths.
Its registry MODULE is an independent 4,481-byte SHA-256-SRI transfer after
extraction and before publication.

The verified archive capture stays callback-local and feeds one provisional
`TempDir`; the complete root alone reaches the existing post-callback token
check. A domain-separated Slug-native association covers both verified content
digests, never the temp path. The selected 256 MiB expansion/payload, 64 MiB
entry, 8,192-physical-header, 256-byte path, 32-component and 1 MiB MODULE
ceilings are admitted divergences. `flate2`/`tar` add exactly eight locked
packages with no existing-version drift.

Pinned `../zabel` commit `c7298478…` guided this ownership decision: its
selected repository source joins a producer-owned semantic view to completed
materialization, and its generated materialization retains the complete
immutable root in the physical payload. No Zabel behavior or representation
is copied; pinned Bazel 9.2 remains sole archive/MODULE behavior authority.

### M7 selected-BCR verified capture accepted; realization audit active (2026-08-25)

Commit `3bc02039` streams only the admitted selected-BCR plan through ordered
HTTPS direct HTTP/1 connections, verifies SHA-256 SRI in bounded command
scratch, explicitly deletes verified captures and publishes the honest
generation-scoped deferred-extraction materialization terminal. No task,
client, global provider, DICE I/O, retained capture/path/socket or root is
introduced. Independent lifecycle review accepted the stale cutoff,
first-success stop and ordinary peer-held-open disposal correction.

The nine transport proofs and ten archive/session proofs pass; the full core
suite is 298 pass with its one declared unrelated query assertion failure.
Fresh wildcard-removed rules_rust query/build replays preserve only the public
collapsed repository-session terminal. Current must re-derive gzip/tar,
executable-mode, registry-MODULE and immutable-root ownership before more Rust.
Pinned Zabel commit `c7298478…` remains architectural guidance for keeping the
semantic view separate from physical realization; Bazel 9.2 remains sole
behavior authority.

### M7 selected-BCR transport-entry audit accepts verified capture (2026-08-25)

The live callback runs synchronously after a completed DICE Need attempt, with
no transaction or materializer lock, and the existing current-thread runtime
can directly drive a raw HTTP/1 connection. The smallest bounded successor
streams ordered HTTPS responses into a capped temporary capture, verifies SRI,
deletes it, and advances the direct session from deferred transport to deferred
extraction without publishing physical state.

Exact dependency resolution adds only Ring-local `rustls`, native roots and
no-default-features `tokio-rustls`; workspace Tokio-Rustls is forbidden because
it enables AWS-LC. Pinned Zabel guides semantic-view/physical-realization
ownership and scratch lifetime only; Bazel 9.2 owns transport behavior. M7
remains partial and M7A -> M8 -> M7B is unchanged.

### M7 exact BCR plan/local archive split accepted (2026-08-25)

Commit `1807b1d4` moves the accepted local archive owner/proof behind a private
plan boundary and admits the produced Bazel 9.2 `tar.gz` shape without physical
work. Exact BCR fields produce a generation-scoped deferred `TransportError`;
malformed fields remain `SpecError`; the local byte/path/diagnostic surface is
unchanged. Independent review, focused proof, locked compile and hygiene pass.

The fresh wildcard-removed rules_rust replay reaches the repository-session
non-success terminal; the public wrapper collapses the inner message, while
direct session proof retains the exact deferred result. Current must re-derive
one bounded transport entry from the live split before more Rust. Pinned Zabel
guides semantic-view/physical-realization ownership only; Bazel 9.2 owns
behavior. M7 remains partial and M7A -> M8 -> M7B is unchanged.

### M7 exact BCR plan/local archive split active (2026-08-25)

The corrected bounded successor separates plan admission from physical work.
It preserves the local file/tar branch byte-for-byte, parses the producer's
complete BCR fields into a private immutable plan, and returns an honest
generation-scoped deferred `TransportError` without DNS, runtime, root or
archive effects. Malformed shapes remain stable `SpecError`.

This packet uses pinned `../zabel` only for semantic-view/physical-realization
separation; Bazel 9.2 and Slug's accepted producer own exact fields. M7 remains
partial and M7A -> M8 -> M7B is unchanged.

### M7 BCR producer/runtime correction active (2026-08-25)

The producer's direct proof and real source require `type = "tar.gz"` plus
explicit empty/zero structural fields. The packet incorrectly required absent
type. Its candidate also replaced the accepted local plan, used blocking raw
HTTP rather than the accepted existing-runtime direct-Hyper lifecycle, created
a root before SRI and missed ceilings/proof. It was removed; the tree is clean.

Current is docs-only. Preserve the accepted dependency closure and pinned
`../zabel` ownership guidance; Bazel 9.2 and Slug's accepted producer own the
exact fields/behavior. M7A -> M8 -> M7B is unchanged.

### M7 BCR dependency closure accepted; implementation active (2026-08-25)

The isolated accepted lock delta adds five direct core names and eight bounded
compression/archive packages in 77 lines. Every existing entry, including
`wasip2 1.0.4+wasi-0.2.12`, remains exact. The resolved graph is Ring-only;
AWS-LC and global provider installation remain forbidden.

Implement only current's eight-file boundary. Bazel 9.2 owns behavior; pinned
`../zabel` guides producer-owned semantic view versus physical-root ownership
only. M7 remains partial and M7A -> M8 -> M7B is unchanged.

### M7 BCR dependency closure correction active (2026-08-25)

The accepted direct Ring transport requires five new core dependency edges and
an eight-package `flate2`/`tar` closure. The implementation contract wrongly
marked `Cargo.lock` read-only, and its worker stopped and restored a clean tree.

Current is docs-only: freeze the isolated 77-line lock addition, retain existing
`wasip2`, prove the Ring-only graph and admit no other version drift. Bazel 9.2
still owns behavior; pinned `../zabel` still guides semantic/physical ownership
only. M7A -> M8 -> M7B is unchanged.

### M7 BCR HTTP lifecycle accepted; archive implementation active (2026-08-25)

The corrected transport has no legacy client, pool, executor or retained
socket. DNS completes on the synchronous command owner. Bounded runtime entries
poll one pinned direct HTTP/1 connection and yield body frames; capture writes,
hashing and extraction occur outside Tokio, and final shutdown is driven before
return. Registry remains untouched.

Implement only the exact BCR shape and accepted local archive in current.
Pinned Bazel 9.2 owns behavior; pinned `../zabel` guides producer-owned semantic
view versus physical-root ownership only. M7 remains partial and M7A -> M8 ->
M7B is unchanged.

### M7 native BCR HTTP lifecycle correction active (2026-08-25)

Independent review rejected the first implementation draft: Hyper's legacy
client spawns connection drivers and default DNS can spawn blocking work, even
though the draft claimed no task or shutdown duty. No Rust changed under it.
Current now designs an archive-only HTTP/1 connection whose resolver completes
on the synchronous command owner and whose connection future is driven and
joined inside each runtime entry. Registry remains untouched.

The accepted rules_rust archive shape remains exact and the local tar slice is
unchanged. Pinned Bazel 9.2 owns behavior. Pinned `../zabel` guides the
producer-owned semantic-view/physical-root separation only. M7 remains partial
and M7A -> M8 -> M7B is unchanged.

### M7 root selected external loading accepted; archive design active (2026-08-25)

The reviewed audit accepts the frozen eight-file route/load vertical against
its actual already-materialized selected-source surface. The direct transaction
proves structural route identity, ordered external-Bzl loading, recursive
producer views and lifecycle; broad Rust validation passes. The corrected real
command proof advances from the old Host-loader rejection to the exact
`rules_rust+` materialization request and drops the false downstream terminal.

The sole core materializer remains the next owner. Its local tar fixture slice
cannot consume Bazel's HTTPS/SRI/gzip-GNU-tar request or registry MODULE
replacement. Run only the docs design packet for a bounded private archive
owner and lawful async/session boundary. Pinned Bazel 9.2 owns behavior; pinned
`../zabel` guides semantic-view/physical-realization separation only. M7 stays
partial and M7A -> M8 -> M7B is unchanged.

### M7 root external-load proof REPLAN exposes native archive frontier (2026-08-25)

The retained eight-file candidate passes its focused and broad Rust proof and
keeps ordinary route callers closed. With only the parked wildcard registration
removed, real rules_rust now advances beyond the prior root Host-loader
rejection and demands the structural `rules_rust+` source.

The next command-visible terminal is earlier than the packet predicted: native
materialization rejects Bazel's standard BCR archive `RepoSpec`, including SRI,
empty remote patch/overlay maps and registry MODULE replacement facts. A local-
path disguise is also rejected and is not equivalent. The candidate therefore
remains unaccepted while a docs-only audit corrects its proof boundary and
selects one natural archive/materializer successor.

Pinned Bazel 9.2 owns archive behavior. Pinned `../zabel` guides only the
separation of semantic repository views from nonsemantic physical realization;
no Zig transport, archive, cache, path or behavior is adopted. M7 remains
partial and M7A -> M8 -> M7B remains unchanged.

### M7 selected-registry source oracle accepted; corrected owner active (2026-08-25)

The mandatory fixture audit accepts corrected growth from `51540963` through
`3ac0a85b` and resets hygiene there. Packet one adds exactly 46 regular files,
zero links, 152 text lines, one command and 20,480 artifact bytes. Pinned Bazel
9.2 generation plus two distinct fresh-root replays all return `status: ok`.
The row can succeed only through the selected owner's self and mapped views.

The corrected design keeps the canonical selected definition, `RepoSpec`,
source policy and ordered mapping as Bzlmod-owned structural inputs; loading
consumes that typed source fact and owns recursive evaluation. This follows
pinned `../zabel` architecture guidance without copying Zig implementation or
using it as output authority. Run only the seven-file implementation in current.
Actual rules_rust declarations, schemas/effects and upper consumers remain
unsupported/deferred; M7 remains partial.

### M7 selected-registry source owner accepted; frontier audit resumed (2026-08-25)

The corrected seven-file implementation retains the selected definition source
on both request constructors, projects a distinct structural selected route,
and switches every mapped recursive load to the child producer's retained
view. Loaded-definition projection and both pure reacquisitions share the same
owner while preserving root Need, typed outer, semantic-error and epoch
polarity.

Focused lifecycle proof, all 543 Bzlmod unit tests plus integrations, the full
loading suite, the dependent core check and diff hygiene pass. Conservative
isolated additions are 775, below even the 850 production cap; all seven files
fit their physical ceilings. Independent terminal review returns `ACCEPT`.

Pinned `../zabel` `c7298478…` guided only the producer-owned mapping,
nonsemantic physical realization and already-selected consumer view. Bazel 9.2
remains behavioral authority. Return only to the docs-only bootstrap-critical
frontier audit; actual rules_rust still stops at `repository_rule(doc=...)`,
and M7A -> M8 -> M7B remains.

### M7 frontier audit corrects the next command-visible owner (2026-08-25)

Direct replay shows the unchanged fixture first reaches the parked M8 wildcard
registration. With only that line removed in a disposable copy, root BUILD
evaluation stops on `@rules_rust//rust:defs.bzl`; it does not yet reach
`repository_rule(doc=...)`. Root loading is root-only, and the existing root
route owner still rejects registry-selected dependencies.

Independent review accepts one cross-stage design: Stage 5 must project the
accepted root mapping plus canonical selected definition into a structural
selected route; Stage 4 may then consume it for the external Bzl child. This
follows pinned Zabel's producer-owned resolved-view layering only; Bazel 9.2
remains behavior authority. Run only the design in current.

### Root package external-Bzl owner design accepted (2026-08-25)

The accepted cross-stage design adds an equality/hash-discriminated root-BUILD
admission mode to the existing route key. Only its original Unsupported result
may consult the accepted root mapping and canonical selected definition; only
SelectedRegistry projects the existing structural selected route. Ordinary
callers retain exact builtin/direct-local/unknown/error behavior and cannot
activate the selected graph.

Root package loading preserves root recursive resolution, but apparent direct
BUILD loads consume the admitted route and existing external-Bzl child in
source order before package evaluation. Child events remain child-owned.
Observed selected failures project exhaustively to path retry outers or typed
infrastructure terminals; neither is flattened into the other.

Pinned Zabel `c7298478…` guides the package-source/resolved-view ownership only;
Bazel 9.2 remains behavioral authority. Independent review returns `ACCEPT`.
Run only the eight-file implementation packet in current under 900 production,
1,050 proof and 1,950 aggregate caps. M7 remains partial.

### Rust-only semantic-compatibility reset (2026-08-08)

Explicit user direction permanently excludes JVM/Java integration or semantic
delegation. Rust Host observations and valid-Unicode regex/string behavior are
Slug-native; exact Bazel configuration/output-directory bytes remain M9 work.
Complete structural equality/invalidation remains mandatory for admitted
inputs, unmodeled inputs fail closed, and a namespaced display/path projection
never becomes the semantic key. REAPI/CAS, content, repository, and lockfile
digests remain exact and separate.

The query-regex contract is the locked Rust `regex` 1.13.1 engine over valid
Unicode strings: compile once, apply unanchored search to the exact candidate
strings selected by each query function, enforce explicit parser/NFA/DFA and
input limits, and report Slug-owned diagnostics. Lone UTF-16 surrogates,
Java-only `Pattern` constructs, Java diagnostic text, and UTF-16 offset parity
are unsupported rather than emulated.

Exact Bazel ActionKey work is no longer a wholesale M9 item. After Stage 6 owns
an immutable configured-action row and owner platform/properties, each admitted
action family may add a Rust-only exact projection that feeds SHA-256 the same
ordered byte stream as Bazel 9.2. Zabel commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is a reviewed donor for protobuf
no-tag fingerprint primitives, Bazel internal-string encoding, per-family
GUID/body order, deterministic FileWrite compression, and the common
ActionKeyComputer platform/property tail. Fresh Bazel 9.2 source and oracle
evidence remain the authority for every accepted family.

The identity domains stay firewalled: structural action identity owns equality
and invalidation; an exact Bazel ActionKey is an optional aquery/parity
projection; and the SHA-256 digest of the encoded REAPI Action is the remote
Action Cache key. Exact ActionKey reproduction does not by itself establish
local or remote cache interoperability.

Source-language parsing remains the vendored Buck2 `starlark-rust` parser and
evaluator with Bazel-owned dialect, globals, labels, effects, and diagnostics.
The remaining handwritten registry-snapshot MODULE directive parser is later
replacement debt; no new Slug Starlark parser is permitted. Bazel query syntax
is a separate language and keeps its own parser.

The direct-local external exported-source build activation is accepted in
`42f4a64b` at 259 production/186 test/445 total formatted net lines. It closes
the bounded source-only M1 build vertical; the remaining named M1 gaps are
terminal, so the current packet pivots to the root configured-target/cquery
boundary required by the canonical direction reset.

The bounded root Starlark-label cquery implementation is accepted in
`135b0567`; it directly consumes the existing configured-analysis key and
publishes exact success/missing/recovery bytes without exposing `first-build`.
An action-query audit then found no configuration-opaque Bazel formatter, so
`aquery` remains deferred rather than inventing action keys, configured paths,
platforms, or configuration text. The active oracle-only packet now pins the
first semantic configuration discriminator: a string build setting and a user
transition observed through provider values, never through a checksum. The
first evidence attempt proved that Bazel's invalid-transition diagnostic itself
prints the unavailable checksum and was discarded; the positive-only successor
keeps successful semantic configuration evidence separate from that blocked
failure envelope.
The positive successor is accepted in `b12774b9`: direct default and command
values, two distinct transitioned configurations of the same child, warm
reuse, transition edit/restoration, and default edit/restoration are exact and
checksum-free. The accepted design keeps configuration resolution and
recursive analysis in request/resolved modes of the existing root key family,
uses the effective compact string as the new semantic discriminator, and keeps
all public command observation unchanged. Its implementation stopped cleanly
at a pre-existing provider decoder invariant: Slug requires an explicitly
returned `DefaultInfo`, while Bazel accepts custom-only returns and observes an
implicit empty default. The active positive oracle now pins that normalization
as `d4e7e47e`; the accepted one-function decoder design now restores Bazel's
implicit empty default while preserving Slug's strict collection invariant.
That decoder is accepted in `7c6eeae5`, so the internal transition
implementation is accepted in `dfc1705e`. The positive-only toolchain oracle
is accepted in `ed4baf08`; it pins first-compatible execution-platform and
toolchain selection through provider markers without exposing configuration,
platform, toolchain, path, or action identity. Reserved review accepted a
serial prerequisite before native loading and resolution: retain only guarded
direct root registration labels in semantic MODULE evaluation order, preserve
the existing root `dev_dependency` policy, and expose them through the existing
Need-aware loading anchor without a digest or new DICE key. That bounded
registration-retention implementation is accepted in `4a3af8df`. The five
fixture-bounded native constraint, platform, toolchain-type, and toolchain
target declarations are accepted in `6a457406`, including fail-closed root and
external query boundaries. Frozen Starlark rule requirements and the
load-capable, invocation-unsupported `platform_common.ToolchainInfo` symbol are
accepted in `1d6106bd`. Commit `1533569f` integrates those serial values with
the root registration anchor in one real DICE selection/prepared-context
vertical, including builtin ToolchainInfo and the bounded `ctx.toolchains`
consumer. Commit `afd2a606` retains the configuration-opaque recursive action
closure in deterministic breadth-first order, with full configured-key
deduplication, direct child invalidation edges, shared analysis values, and
recursive CLI/REAPI iteration. The current packet adjudicates only the four
remaining identity owners required by an exact action-query handoff. That
adjudication returned `REPLAN`: configuration, configured paths, per-action
platform, and Bazel ActionKey require a serial prerequisite chain. The current
packet records isolated Bazel 9.2 source/oracle discriminators before the first
general configuration-substrate design. Commit `f00e99db` now pins those
discriminators: configuration owns the checksum/output root, platform and
content affect the FileWrite ActionKey, and output name does not. The current
packet designs only the complete shared target-configuration input chain.
Invalid transition and broader toolchain failure diagnostics remain deferred.

The retained Bazel 9.2 evidence pins default and explicit `label` output to the
same `//parent:parent (a7a71fd)` bytes. Pinned source and live audits prove even
the default checksum depends on seventeen native option fragments containing
341 cache-key options, host and platform inputs, plus Starlark options/scopes;
Slug's opaque `first-build`
cannot reproduce it in a bounded packet. That public output remains
unsupported. Reserved review selected a smaller first consumer whose Starlark
expression emits only the canonical configured label. Retained Bazel 9.2 now
pins its exact `@@//parent:parent\n` bytes, missing-target failure, and
same-server recovery; the accepted implementation retains the narrow command,
typed error, daemon-wire, and lifecycle boundary. Reserved review accepted its
direct existing-key route, exact eight-production/five-test allowlists, and
650/600/1,250 caps. That M4 slice is now preserved while semantic configuration
work resumes in M2.

The direct-local external exported-source build lifecycle evidence is accepted.
Pinned Bazel 9.2 proves present/edit/recreate success with no output, deletion
as an exact missing-input failure, and directory presence as success. The
bounded five-file implementation is current.

The direct-local external exported-source build activation design is accepted.
It reuses the existing route/load/source owners, adds no DICE key, and freezes
one retained completion class so observed root and external exported sources
can succeed without changing filegroup, package-all, rule, analysis, action, or
REAPI paths. Reserved Sol correction review returned `ACCEPT`; the current
packet records the one remaining Bazel 9.2 missing-source lifecycle
discriminator before Rust.

The direct-local public unsupported-cycle boundary is accepted in `ea2019f8`
at 210 production/493 test/703 total formatted net lines. It preserves typed
ordinary failures and projects only private cycle capability through both
query load consumers to the exact query-only Slug-owned unsupported terminal;
build and root loading remain unchanged. Independent Terra latest-text review
returned `ACCEPT`. The current packet is read-only design for the smallest
separately reserved external-build activation: one explicit exported source
target in a direct local override, with no configured analysis or action
breadth.

The external Restricted-visibility typed implementation is accepted in
`fc022925`. Native-Windows glob ordering reached `REPLAN`: no native runtime is
available and the Unix-only byte/Latin-1 carriers cannot preserve the required
UTF-16 identity. The direct-local handoff design is accepted: one private
callerless key composes the existing root route and Host source owners into an
unselected MODULE-file input without entering the legacy source-preparation or
registry graph. Its first implementation reached `REPLAN` because the frozen
test cap could not retain all lifecycle/error/reuse evidence plus exact
bootstrap/path/materialization Need forwarding. The corrected 100 production/
440 test/540 total retry reached `REPLAN` after a passing suite measured 472
test lines. A clean-HEAD reconstruction exposed that the measurement was not
evidence-complete: its activation path ended in a route error without capture
enabled, its version edit asserted only value equality, and it never completed
the external source through Present/edit/Absent/recreate states. The
100/480/580 cap is therefore revoked. The corrected evidence design is now
accepted at 100 production/545 tests/645 total: 525 mandatory test lines plus
20 lines of formatting/compaction-only slack. The corrected implementation is
accepted in `e5e2c55d`. Direct evaluation remains blocked because the raw
handoff uses Bazel's exact nonregistry identity: route module name plus empty
version, independent of both the root-requested and file-declared versions.
The parser-backed inspection projection is accepted in `8aae11d6`. The first
closure/evaluation design reached `REPLAN`: the live sparse path/retry owner is
already sufficient, but direct include reads lack route-aware external package
policy/preflight; the private evaluator also prepopulates declarations from the
expected key and rejects nonregistry print. The one-file
`HostRepositoryPathKey` prerequisite is accepted in `00e85153` at 168
production/350 test/518 total net lines. The atomic route policy and package
lookup is accepted in `42ef64cd` at 449 production/739 test/1188 total lines.
The public selected-BUILD source and atomic external loading migration is
accepted in `9b5246af` at 211 production/368 test/579 total lines. The
route-aware package horizon is accepted in `1d5edc7c` as one private
`source_preparation.rs` implementation at 298 production/647 test/945 total
net lines. It reuses the accepted inspection and external lookup, requests all
first-seen unique packages before interpreting results, and applies Bazel 9.2
source-order mixed terminal/Need precedence. The private support-gated closure
implementation is accepted in `f2b626f2` at 434 production/1320 test/1754 total
formatted net lines. Thirteen focused tests, all 46 source-preparation tests,
all 30 host-package tests, GNU-Windows no-run, archive, formatting, and diff
gates passed. The full library result was 265/266; the sole untouched
`records_exact_proxy_tag_and_innate_call_spans` expectation failure reproduced
at clean HEAD. Its opaque preparation owner retains every supported acyclic
occurrence in breadth-first order, validates the present root before any include
package/source activation, and keeps cycle capability metadata
outside the semantic closure. An active-ancestry repeat becomes a pending
capability candidate only after its whole horizon succeeds; the owner prunes
only that repeated occurrence's deterministic outgoing replay and continues
every remaining cycle-free reachable worklist path. Later-horizon Needs and
real failures therefore retain Bazel precedence, including failures beneath
siblings of the first cycle candidate. Only an otherwise-successful exhausted
worklist returns the private unsupported-cycle capability. Public build/query/
one-shot/daemon publication remains frozen pending explicit user approval of
that product-visible limitation. The route-plus-requests package preflight
refactor is accepted in `34a2340e` at 9 production/4 test/13 total net lines.
The accepted two-packet successor first corrected the private trusted evaluator
in `module_eval.rs`. Packet 1 is accepted in `c683c239` at 190 production/208
test/398 total formatted net lines. Its focused result was 17/18 and the full
library result was 270/271, both failing only the known clean-HEAD
`records_exact_proxy_tag_and_innate_call_spans` baseline; GNU-Windows no-run,
formatting, archive, and diff gates passed, and independent latest-diff review
returned `ACCEPT`. Packet 2 is accepted in `3cf0e441` as an exact one-file
`source_preparation.rs` change at 193 production/577 test/770 total net lines.
Focused evaluation-owner tests passed 4/4 and all 50 source-preparation tests
passed. The full library result was 274/275, failing only the known clean-HEAD
`records_exact_proxy_tag_and_innate_call_spans` baseline. GNU-Windows no-run,
formatting, archive, diff, and scope gates passed, and independent latest-diff
review returned `ACCEPT`. Both private serial packets are complete. The user
has now explicitly approved a Slug-owned public unsupported-cycle limitation;
the accepted design freezes a query-only selected package-source gate. Sol
correction review and independent Terra latest-text review returned `ACCEPT`;
the bounded implementation packet is active.

The external package-policy design is accepted as three serial implementation
packets. First, the accepted private one-file `HostRepositoryPathKey` owns
route materialization plus resolved path state only (168 production/350 test/
518 total in `00e85153`). Second, the accepted atomic four-file route policy and
lookup in `42ef64cd` owns
canonical global deletion, route-local `REPO.bazel` and `.bazelignore`, and
`BUILD.bazel`-before-`BUILD` selection without marker bytes (449 production/
739 test/1188 total). Third, the accepted four-file public selected-BUILD source
and loading migration in `9b5246af` consumes
that lookup before reading the selected BUILD file (211 production/368 test/
579 total). The existing
path/retry substrate remains accepted; these packets add no oracle. Package
horizon, occurrence-preserving closure, and corrected evaluation/event
ownership remain serial after them.

The external query package-identity implementation is accepted in five files.
One private request-local Arc owner retains full canonical package identity
plus the first apparent repository route, uses allocation-free canonical-only
equality/hash/order, and dispatches external graph/package provenance only
after route-to-canonical verification. Focused query, retained-daemon,
lifecycle, output, route-remap, and real-path Private/Restricted evidence
passed; independent latest-diff rereview accepted. The 17-row fixture remains
frozen while the external Bzl owner proceeds as a separate design.

The dormant external Bzl-module owner implementation is accepted in exactly
two loading production files plus one same-module test file. Its private
route-derived label/key, typed complete errors, Host logical source path,
canonical manifest/frozen-lifetime representation, evaluation-only local
event metadata, and isolated third cycle family passed 104 loading tests, the
downstream query non-activation guard, native checks, and both GNU-Windows
no-run gates. DICE `Reused` activations carry no evaluation data and therefore
prove reuse without recapture rather than retained-batch exposure. Freeze
coverage is structural-only because every value in the current loading globals
implements `Freeze`. Independent correction rereview accepted the final
three-file `+1205/-8` diff. `RepositoryPackageLoadKey::LoadsUnsupported`
remains unchanged and no production caller reaches the private key, so
macro-produced native targets and query provenance remain dormant pending the
separate activation design.

The external Bzl package/query activation audit reached `REPLAN` before Rust.
The loading, lifetime, error, event, and query seams are bounded, but neither
the frozen 17-row fixture nor the accepted ad-hoc custom-rule probes prove the
automatically reachable case where a `.bzl` macro creates a native
`filegroup`. That audit scheduled a minimal Bazel 9.2 oracle addition in an
isolated `dep/macro` subpackage; all existing rows and the existing dependency
BUILD file remain protected.

The external test-base closure audit reached `REPLAN`: its direct
unconfigured implicit edges are finite and source-pinned, but their transitive
packages require the built-in installed `@bazel_tools` repository, contextual
rules_shell/rules_java/platforms mappings, and an extension-generated remote
coverage repository that the current direct-local route cannot own. The
next packet designed only that DICE-owned repository-closure prerequisite;
external test rules and suites remained frozen.

The repository-closure ownership audit also reached terminal `REPLAN`.
Installed tools bytes can be source-pinned, but the exact selected-module,
registry, contextual mapping, extension-generated repository, and complete
package/query semantics have no bounded Rust owner; the exact Host registry
byte surface already has an accepted JVM/process-state impossibility result.
The external test-base/tools branch is therefore unsupported under the current
architecture. The next attempted existing-owner vertical slice was
same-package external package-group visibility for `visible()` only.

That `visible()`-only package-group visibility design also reached `REPLAN`.
Repository-relative matching, caller identity, include traversal, and DICE
invalidation can reuse the existing route/package/graph owners, but admitting
the Restricted target to their shared graph also exposes Bazel's raw
`visibility` attribute and effective `VisibilityNodep` edge. Omitting those
surfaces makes `labels(visibility)`, dependency/reverse/path traversal, and
graph output observably partial; adding them violates the completed packet's
explicit other-consumer stop. The superseding design packet was read-only and
covered the complete already-enabled generic-query consumer surface, with raw
and effective visibility kept distinct.

The complete external Restricted-visibility consumer design is accepted. It
limits the protected target to one native `filegroup`, reuses the existing
route/package/graph owners, route-remaps raw declared visibility separately
from effective top-level `VisibilityNodep` edges, preserves group includes as
their own edges, and keeps both NODEP/implicit query flags deferred. The
current evidence-first packet creates one isolated seven-row fixture; the
existing 20-row `module-local-override` fixture remains frozen.

The isolated external Restricted-visibility oracle is accepted at seven new
files, five workspace assets, seven exact rows, zero links, and 278 lines.
Bazel 9.2 generation and distinct-root replay passed; the first six rows are
Slug acceptance evidence and the final `--nonodep_deps` row is Bazel-only
edge-kind evidence. The current packet implements only the accepted four-file
projection and does not add a dependency-filter flag.

The attempted four-file Restricted-visibility implementation reached its caps
and passed focused/full Rust tests, but terminal review required REPLAN. The
accepted pure projection forbids parsing, while `CanonicalLabel` has no typed
repository-rebind API inside the four-file allowlist. The saturated test
boundary also omitted dedicated warm, visibility/include edit, route-remap,
different-external caller, and pre-synthesis sentinel discriminators. No Rust
from that attempt was retained. The successor design adds one narrow typed
identity rebind, clears stale mapping provenance, expands the exact boundary
to five files and 820 net lines, and requires every missing lifecycle/caller/
ordering discriminator. Independent latest-text review accepted the typed API,
mapping-provenance policy, caps, evidence matrix, oracle comparison, and stops.

The external Bzl macro-query oracle is accepted in the exact four-path
`+112`-line boundary. Bazel 9.2 generation and a distinct-root replay passed
all 20 rows; the 17 protected records remained JSON-deep-equal, and the three
new rows pin macro-created native filegroup kind, Bzl-only `loadfiles()`, and
external BUILD-first `buildfiles()` output. The full 107-test oracle harness,
archive, structural, and diff gates passed, and independent latest-diff review
accepted. No Rust, Cargo, tool, daemon, lifecycle, or activation surface
changed. The external Bzl package/query activation and exact bare-
`--noshow_progress` compatibility prerequisite are accepted together in eight
paths at `+829/-21`. All three macro rows now pass Slug exactly; only the
pre-existing unrelated external-build row remains red. The dependency-free
external Starlark-rule projection is accepted at five files and `+529/-0`
without new oracle growth. The current packet activates only the bounded
external Restricted-visibility query projection; it does not add a dependency-
filter flag, a test rule, or a suite.

Latest M1 accepted evidence: the corrected Host RegistryFunction oracle passed
one pinned Bazel 9.2 generation and two distinct fresh-root replays for each of
its two fixtures. Nine yanked-policy rows prove the cold-cache
`1,1,1,1`→`1,2,2,1` Off recorded-absence transition, selected-yanked reuse,
SHA-before-yanked precedence, digest restoration, and Refresh refetch. Twelve
transport rows prove exact ordered default, per-registry/later-wins,
explicit-empty, and exit-2 unknown-registry mirror projections; successful
rows explicitly empty embedded BCR mirrors and assert the comma-inclusive
formatter output without claiming archive attempts. The exact four-path diff
has 29 regular files, zero links, and 1,659 lines, growing by 507 lines within
the accepted cap. Source/parity, native implementation/evidence, and
architecture/orchestration terminal latest-diff reviews all returned
`ACCEPT`; no Rust, Cargo, dependency, API, consumer, or activation changed.

The private Host registry-input owners design is also accepted. Its exact
three-file, 900-added-line boundary separates the normalized command-registry
set, complete mirror map, vendor-only package-policy projection, and opaque
Refresh token. It preserves the slash-retaining implicit BCR default,
structured post-converter inputs, order-insensitive set/map equality,
old-value retention on equal reinjection, explicit-empty map identity, exact
unsubstituted lookup spelling, vendor fatal-read deferral, and strict
one-hour token lifecycle. All three terminal latest-text reviews returned
`ACCEPT`; no implementation work was started.

The dormant Host registry-input prerequisite is accepted in exactly three
paths with 899 additions and eight deletions. Four focused tests and the full
190-unit/184-integration crate surface passed with zero failures and zero
doctests; GNU-Windows built all twelve test executables. The owners preserve
separate semantic identities, retained equality/pruning, vendor-only
projection, and request-generation-independent Refresh state without public
or production wiring. Missing injected inputs are non-replayable
activation-order invariant diagnostics: later production activation must
atomically preinject every required value before exposing any consumer. All
three terminal latest-diff reviews returned `ACCEPT`.

The pure root-free Host RegistryFunction owner is accepted in exactly three
paths with 1,543 additions and no deletions. Eleven focused tests and the full
197-unit/184-integration crate surface passed with zero failures and zero
doctests; GNU-Windows built all twelve test executables. The owner preserves
pinned mode/vendor/Refresh/visible/mirror construction order, exact original
and resolved URI spellings, Java URI construction semantics, compact
hash/yanked-only lockfile equality, complete-only Needs/errors, retained
recomputation and pruning, and the exclusion of root, IO, request-generation,
mapping, source-preparation, write, and activation edges. Both terminal
latest-diff reviews returned `ACCEPT`.

The one-file Host Registry IO bridge design is accepted. It freezes a private
closed remote execution plan, exact Host hash-mode/expectation matrix, typed
remote/local failures, and generation-before/after-IO ordering while keeping
all active legacy wrappers and legacy Off behavior byte-for-byte. Host remote
Ignore is a typed routing error; legacy Off selects unverified fetch directly.
All three terminal latest-text reviews returned `ACCEPT`; no Rust, Cargo,
public API, dependency, consumer, or activation changed.

The dormant one-file Host Registry IO bridge is accepted with 833 additions
and 96 deletions. Four inline tests, the five-test registry-sensitive
source-preparation slice, the full 201-unit/184-integration bzlmod surface,
54 loading tests, 115 core tests, all doctests, and all 20 corresponding
GNU-Windows test executables passed. Exact scope/growth, formatting, diff,
archive, credential, public-API, call-site, and forbidden-edge gates passed.
The bridge preserves every active legacy wrapper and legacy Off behavior,
implements the exact Host matrix and generation ordering, and adds no public
item, production key, dependency, consumer, or activation. Both terminal
latest-diff rereviews returned `ACCEPT` after one evidence-only correction.

The first Host registry-file vendor oracle design ended in `REPLAN` after its
one focused correction. Terminal review proved that Refresh with
`vendor-missing` and disabled caches must issue an extra checksum-present yyy
MODULE request, and that the draft's broad yyy-request stop gate contradicts
its intended RepoSpec `source.json` request. No fixture or Rust changed.

The corrected Host registry-file vendor oracle design is accepted. It moves
the misleading aaa asset and Refresh to `vendor-hit`, preserving the exact
4→5/5/5/6 yyy MODULE sequence while checksum-empty aaa bypasses present vendor
bytes, and narrows the stop gate to yyy MODULE requests. All three terminal
latest-text reviews returned `ACCEPT`; the accepted scope remains six paths,
four assets, fourteen commands, and no harness, registry-byte, or Rust edit.

The Host registry-file vendor oracle is accepted. Pinned generation and two
absolute distinct-root replays prove exact vendored hit/fatal/fallback/
restoration and checksum-empty Refresh behavior across fourteen commands.
The fixture is 22 files, zero links, and 1,340 lines; the full fixture tree is
1,301 files, 14 links, and 36,603 lines. Parser, validator, source, archive,
credential, host-path, scope, growth, and diff gates passed, and all three
terminal latest-diff reviews returned `ACCEPT`.

The Host registry-file owner pre-implementation audit ended in `REPLAN`
before Rust. The accepted local bridge ignores its native path argument and
the runtime URL-only capability re-derives local paths without exact decoding,
so Host resolution cannot yet control encoded, non-UTF-8, or Windows local
registry reads. No Rust or fixture changed.

The two-file local native-path Registry IO bridge correction design is
accepted. Its defaulted capability method preserves every existing scripted
implementation and remote/legacy path, while the production override reads
the supplied native `Path` without formatting or reparsing. All three
terminal latest-text reviews returned `ACCEPT`.

The native-path bridge correction is accepted at +113/−2. Bzlmod 201+184,
loading 54, core 104+13, all doctests, all 20 GNU-Windows executables, and
all auxiliary gates passed; all terminal latest-diff reviews returned
`ACCEPT`.

The private Host registry-file owner redesign ended in `REPLAN` before Rust.
The corrected two-file draft captured checksum-mode identity, exact local
recordability and JDK path conversion, DICE-owned vendor lifecycle, and
`Path.isFile` fallback/selection semantics, but terminal source review found
that Bazel serves local `file:` directories as listing bytes while the
accepted native-path runtime bridge returns a directory read error. No Rust
or fixture changed.

### Replanned semantic-error/evidence contract (preserved for correction)

Run only
`WP-5-m1-operational-path-resolution-semantic-error-evidence-design-correction`.

Perform a read-only correction of the accepted resolver contract before any
more Rust. Preserve the independently validated explicit parent/target frame
machine, exact route splitting, portable roots, raw provenance, marker
asymmetry, fail-fast DICE boundary, and two-file implementation scope.

Freeze one noncontradictory error/equality model for the operational resolver
and semantic byte projection. In particular, decide how `WrongKind` retains an
OS-native diagnostic path and how observation, inconsistent-state, cycle, and
expansion errors remain fully typed without leaking namespace, materialization
instance, physical root/path, or operational route through byte-value
equality. Specify exact public variants and field-by-field operational and
semantic comparisons rather than relying on derived equality.

Freeze an executable test harness before retrying implementation. It must use
observation-backed `ResolvedPathKey` computations for exact self, A→B→A,
prefixed-cycle, relative/absolute descendant-expansion, successful ancestor
marker, ancestor-symlink, transitive-link, dangling-target, raw non-UTF-8, and
parent provenance/marker cases. It must also use stable test-only selector and
downstream counter keys on one retained DICE engine to prove resolver
recomputation and byte-consumer pruning across symlink retarget,
materialization-instance, real-root, metadata, and route changes, plus exact
A→B→Missing→typed-error→A invalidation and restoration.

Name the exact schemas, selector/counter key topology, epochs, expected
`path_to`/`chain`/route/provenance values, and staged assertions. Stop on a new
production owner, dependency, file, runtime/consumer migration, or any attempt
to replace the missing integrated evidence with direct helper/equality calls.

### Prior accepted implementation contract (blocked on the correction above)

Run only `WP-5-m1-operational-path-resolution-byte-projection`.

Add new `app/slug_workspace_v2/src/path_resolution.rs` and only its public
reexports in `src/lib.rs`; add no dependency and do not edit the accepted
observation owner. Implement the exact operational `ResolvedPathKey` and
semantic `PathFileBytesKey` schemas/equality from the accepted owner plan.
Cycle, infinite-expansion, and ancestor-marker values each retain separate
ordered shared `path_to` and `chain`; byte wrong-kind is a dedicated
projection error, not a fabricated observation failure.

Implement one private iterative `ResolutionMachine` over a `Vec` of
independent resolver frames. Each frame owns one requested path, its
Begin/parent-wait/route-replay/lstat/terminal-link/readlink phase, ordered and
sorted logical chains, physical raw-link provenance, and first ancestor
marker. The pure transition seam returns PushParent, exact Observe, or
Complete; the async adapter alone services DICE observations and must hold no
frame borrow across await.

A nonroot caller suspends immediately below a fresh parent frame. On parent
completion, append its raw-link provenance, derive
`parent.real_path + basename`, never copy its ancestor marker, and propagate
errors unchanged. Missing or non-directory parent means caller-local Missing
at that derived path with no route admission or child lstat. Directory parent
routes replay entry-by-entry with the basename through the caller's chain
before real-child lstat. Root check-and-admits and lstats itself and follows a
synthetic root symlink rather than short-circuiting.

After symlink lstat, demand ReadLink; Missing is inconsistent and Error retains
the exact demand. Retain the physical link/raw OS target, normalize an absolute
target from its filesystem root or a relative target from the physical link
parent, run check-only, then resolve target ancestors in the same outer frame;
only its parent gets a fresh frame. Repeat without a cap. Preserve Windows
Prefix plus RootDir and Unix RootDir anchors.

Maintain one unique sorted vector beside insertion order. Exact repeat splits
the old route before its first equal entry without appending the repeat.
Strict descendant of the predecessor splits `old + candidate` before that
predecessor and terminates. A successor strictly below the candidate records
the first nonterminal split from `old + candidate` and continues. Check-only
never inserts; admission inserts at the known binary-search position and
appends once. Ordinary `/a/b/c` must be only `[/a/b/c]`; `/link -> /x/y`
with `/x -> /z` must be `[/link, /x/y, /z/y]`.

Only an actual `Ok(PathOutcome::Need(_))` may propagate as Need. Pass every
resolver-level `ctx.compute` through a private track-caller fail-fast
invariant helper; never stringify/cache an infrastructure error or invent a
demand. A requirement for recoverable DICE errors is a stop and replan.

Proceed test-first in three checkpoints. First prove pure roots, split logic,
frame push/pop, ordinary and suppressed chains/full suffixes, parent
provenance/marker asymmetry, target-parent replay, and root symlink. Then wire
observations and prove cumulative exact demand order; every relative,
absolute, ancestor, leaf, transitive, escaped, and root-clamped link; dangling
versus races; typed errors; raw non-UTF-8; Need validity; exact cycle and both
expansion shapes; and fail-fast infrastructure evidence. Finally add byte
projection plus same-engine A→B→Missing→typed-error→A, symlink retarget
A→B→A, and materialization-instance/real-root operational-unequal versus
semantic-equal pruning.

Use mutable `Vec` state, one incrementally maintained sorted `Vec`, shared
frozen slices, honest `Dupe`, and `Allocative`. Run full
`slug_workspace_v2` tests/doctests, format, diff, exact two-file allowlist, and
archive guards. Stop on a flattened suffix loop, recursion through
`ResolvedPathKey`, parent-marker copying, raw-provenance loss, fabricated
Need/wrong-kind observation, dependency/file/owner expansion, direct IO,
canonicalization/lossy identity, weakened tests, or any consumer/runtime/
repository/retry/publication work.

### Accepted transport evidence

The implementation must:

1. carry primitive ordered registry strings through both one-shot and daemon
   build/query paths without serializing semantic Rust types;
2. normalizes exactly once into `RegistryUrls` before the sole request commit,
   with Bazel's default BCR behavior and fail-closed diagnostics;
3. restores A→B→A request-local values without leaking between build and query;
4. keeps the already accepted `RegistryFileKey`, generation, IO capability,
   root graph, and loading owners unchanged; and
5. names a narrow implementation allowlist and exact CLI/server/core tests.

1. Add an ordered raw `Vec<String>` registry field to build/query command
   requests and a `#[serde(default)]` primitive registry list to
   `BzlmodRequestInputs`. Empty means unspecified. This packet supports
   repeatable `--registry=URL`; generic `--registry URL` parsing is not
   expanded.
2. `slug_commands_v2` only collects required nonempty values in encounter
   order. Ordinary `query` accepts `registry` in its existing flag validator
   and continues rejecting the other currently unsupported bzlmod flags.
   CLI, JSON, and daemon code do not trim, deduplicate, substitute, validate,
   or carry `RegistryUrls`.
3. Both one-shot and daemon paths pass the primitive list through the existing
   explicit bzlmod methods. The common retained-runtime injection helper calls
   one fallible `RegistryUrls::from_request(workspace, raw)` before allocating
   the request generation or scheduling any `changed_to`.
4. `from_request` supplies only `https://bcr.bazel.build/` when the raw list is
   empty; a nonempty list fully replaces that default. It removes every
   trailing slash and first-occurrence-deduplicates in raw encounter order,
   then performs `%workspace%` substitution and URI validation for each
   surviving entry. Validation accepts only exact lowercase `http`, `https`,
   and `file` schemes with a non-null hierarchical path, preserving host-only
   HTTP(S) and Bazel's factory diagnostic shapes. The stored compact
   `RegistryUrls` are the resolved effective URLs; no later layer repeats
   normalization or substitution.
5. The existing `RootModuleRegistryUrlsKey`, request generation,
   `RegistryPolicyKey`, `RegistryFileKey`, IO capability, root graph, and
   loading ownership remain unchanged. Malformed input fails before the sole
   commit and does not consume a generation.
6. Command tests pin default/override ordering, duplicate raw values, missing
   values, and query acceptance. Server tests pin omitted-field compatibility,
   primitive JSON round trips, malformed recovery, and build/query
   default→override→default isolation. Core tests inspect injected registry
   URLs and generation across the same A→B→A sequence. CLI tests exercise
   both one-shot and daemon equality-form transport.

The implementation allowlist is
root `Cargo.toml`,
root `Cargo.lock`,
`app/slug_bzlmod_v2/src/registry.rs`,
`app/slug_bzlmod_v2/Cargo.toml` for the already-locked `url` parser,
`app/slug_commands_v2/src/common.rs`,
`app/slug_commands_v2/src/build.rs`,
`app/slug_commands_v2/src/query.rs`,
`app/slug_commands_v2/tests/commands.rs`,
`app/slug_cli_v2/src/commands/build.rs`,
`app/slug_cli_v2/src/commands/query.rs`,
`app/slug_cli_v2/tests/cli.rs`,
`app/slug_server_v2/src/server.rs`,
`app/slug_server_v2/src/lib.rs`,
`app/slug_server_v2/src/tests.rs`,
`app/slug_core_v2/src/runtime/mod.rs`,
`app/slug_core_v2/src/runtime/dice.rs`, and
`app/slug_core_v2/tests/runtime.rs`.

Do not edit Rust, add discovery/fallback, fetch registry content, expand rc
handling, or design MVS/yanked/final-hash/writer behavior in this packet.

The rejected regex candidate does not authorize a UTF-16 engine fork.
`filter`, `attr`, and regex-based `kind` remain deferred; any V2-owned engine
requires its own UTF-16, diagnostic, resource, allocation, and
differential-corpus gate.

## Adopted Cross-Stage Improvement Overlay (2026-08-12)

The [Zabel-derived adoption roadmap](./slug-v2-subplans/zabel-adoption-roadmap.md)
records accepted planning, oracle, runtime, action-ownership, repository,
execution, progress, explain, watch, complexity, and performance follow-ups.
The [plan-authoring guide](./slug-v2-plan-authoring-guide.md) is the readiness
contract for new and materially revised packets.

The first private core source-observation consumer is accepted in `53152727`,
so the fixed **source-consumer cutover** has occurred without package, loading,
command, or public migration. The post-cutover DICE audit and focused design
selected one loading-source/output-base-lock oracle as the smallest
prerequisite before request-revision Rust. Neither decision widens M1 into the
unrelated Wave A catalog.

After the source-consumer cutover, schedule the remaining work as bounded
packets in this order:

1. generate and replay only the accepted M1 loading-source/output-base-lock
   oracle prerequisite; the applicable DICE audit is already accepted;
2. implement the smallest M1 request-revision/source-certificate vertical with
   final reobservation and atomic compatible publication;
3. add each Bazel-derived Starlark/provider/action/aquery/toolchain oracle
   subset just before the semantic owner it discriminates, rather than making
   one monolithic oracle wave block M1;
4. install Stage 6 immutable action-owner context before broader action
   registration;
5. complete **M7A**, only the repository, rules_rust, toolchain, action/input
   tree, aquery, and Stage 7 REAPI breadth required by the bootstrap closure;
6. run M8 Stage 10.3 analysis and Stage 10.4 fixed-point bootstrap as soon as
   M7A is accepted; then
7. resume **M7B** run/test/BEP, unrelated public-ruleset and command breadth,
   followed by repository-output caching, progress, explain, and watch only
   after their named semantic owners and lifecycle prerequisites exist.

M7A and M7B are scheduling gates within M7, not new compatibility milestones.
M9 exact Bazel configuration/output-identity work remains after the functional
bootstrap path; exact ActionKey projections move with each admitted action
family.

Zabel remains a pinned donor of design lessons and fixture themes, never the
compatibility oracle. Exact claims still require Bazel 9.2 source or generated
oracle evidence.

## Operating Decision

Use the existing repository for continuity, but restart the implementation
shape:

1. Preserve V1 through a tag and archive branch before root-level replacement.
2. Keep V1 source as extraction/reference material, not as the default build
   graph for V2.
3. Build V2 around Bazel 9 semantics, Bazel source/test oracle fixtures, DICE,
   starlark-rust, and REAPI-first execution.
4. Import V1 code only after a small oracle fixture or focused regression proves
   the behavior matches the V2 boundary.

### Future branding TODO

Consider renaming the project to **Rubin**, after Red Rubin basil. The name is
concise, retains the basil theme, and its “red” and initial “R” associations
subtly signal the Rust implementation. Treat this as a future branding decision,
not an implementation milestone or current-packet dependency.

## 2026-07-22 Direction Reset

The immediate goal is not broader build execution. It is one trustworthy,
incremental semantic graph that can reproduce Bazel 9 analysis and expose that
graph through `query`, `cquery`, and `aquery` in increasing order of depth.

The governing order is:

1. Pin all new oracle work to Bazel 9.2.0 at
   `8220c6198837d5c13d53fea211cf3282aa12408a`. The sibling `../bazel`
   checkout may move to Bazel 10 or later; use the tag/commit, not its current
   `HEAD`, for parity evidence.
2. Replace split one-shot evaluation and fallback workspace scanning with one
   daemon-owned DICE graph whose injected inputs cover files, directory
   listings, environment and command policy, repository mapping, loading,
   configured targets, and action declarations.
3. Make configured-target analysis real: recursively analyze dependencies,
   execute rule implementations with prepared Bazel-shaped contexts, consume
   returned providers, and retain declared actions without executing them.
4. Implement full unconfigured `query` over the loading graph, then `cquery`
   over configured targets, then exact `aquery` over the same action graph
   Stage 6 produces.
5. Treat matching `aquery` output as the execution handoff. Only after this
   gate should new execution/cache breadth, `run`, `test`, or broad ruleset
   conformance control the next milestone.
6. Maintain a Bazel 9 build graph for Slug itself so Bazel plus BuildBuddy can
   accelerate development. After analysis, action graph, and execution are
   correct, prove a Bazel-built Slug can build Slug and then reach a Slug-built
   fixed point.

The already-landed first-build and NativeLink-backed REAPI fixtures remain
valuable regression tests. They prove a narrow vertical slice; they do not
prove the DICE ownership, configured-target graph, query surface, or bootstrap
architecture described above.

### Integration-first freeze

- Do not expand Stage 5 with more standalone parser/key/value substrate unless
  the packet is required by the analysis/query/aquery path.
- Do not expand Stage 7 cache, materializer, or backend breadth until the
  `aquery` gate is accepted, except to preserve an already-landed regression or
  to enable the Bazel/BuildBuddy developer build.
- Do not use a real-world build as structural acceptance evidence. Convert each
  discovered gap into a focused Bazel 9 oracle first.
- Historical checkpoint sections remain evidence of what landed. The latest
  priority/gate section in this plan and each owning subplan is authoritative
  when older checkpoint prose says `pending`, `next`, or `first`.

Do not physically move the whole V1 tree into `v1-archive/` unless the tag and
branch archive is not enough. A full in-tree archive makes search, codegraph
indexing, and new-agent orientation worse. If a physical archive is required,
exclude it from active build metadata and codegraph indexing.

## 2026-06-29 Branch Review And Remediation Gate

Review of `codex/slugv2` found that the clean-restart archive sequence has not
actually been completed in this checkout:

- `scripts/v2_archive_status.sh` fails because `slug-v1-archive` and
  `v1-archive` are missing, even though Stage 0 docs recorded them.
- `codex/slugv2` adds V2 scaffolding on top of the full V1 root instead of
  resetting the active tree into a clean V2 root. Relative to `main`, the branch
  adds hundreds of files and no root cleanup.
- `Cargo.toml` still includes the V1 `app/slug*` workspace members beside the
  new `app/slug_*_v2` crates, and the active tree still tracks V1-heavy paths
  such as `app/`, `buck2/`, `prelude/`, and `tests/`.
- A focused V2 compile check passed for the new crates, so the branch is useful
  as a prototype and selective patch source, but it is not the V2 trunk shape.

Do not merge or promote the current `codex/slugv2` branch wholesale as the clean
restart. Before implementation proceeds as V2 trunk, do this sequence:

1. Freeze new feature work on the mixed-root branch.
2. Pick the V1 preservation commit from the live checkout, verify the worktree
   state, then create and validate the `slug-v1-archive` tag and `v1-archive`
   branch.
3. Start the active V2 line from a clean root worktree: keep root pointers,
   V2 plans, and intentionally retained infrastructure; remove V1-only source,
   tests, Buck-shaped metadata, and V1 workspace members from the active build.
4. Re-import from `codex/slugv2` one bounded stage at a time. Each import needs
   an owner subplan, an oracle fixture or Bazel source citation, focused
   validation, and a Stage 9 extraction-ledger entry when it came from V1 or
   from the mixed-root prototype.
5. Run `scripts/v2_archive_status.sh`, `git diff --check`, and the touched
   stage validation before calling the root clean.

Plan-following sessions use
`.codex/skills/slug-agent-orchestration/SKILL.md`.

2026-06-29 execution update: the missing local archive refs have been repaired;
`slug-v1-archive^{commit}` and `v1-archive` now both resolve to
`e218054d4c796655939b968d90208b185decb352`. Cargo root metadata now exposes only
V2 app crates as active `app/slug_*` workspace members/dependencies, with V1
app crates removed from that surface.

2026-06-29 clean-root remediation update: the active clean-root branch is
`codex/slugv2-clean-root-remediation`. It removes tracked V1 source/test trees,
root Bazel/Buck metadata, old CI, old docs, old V1 plans, and the unwrapped
`remote_execution` source candidate from the active tree. The retained tracked
root is orientation docs, V2 plans/prompt, Stage 1 oracle harness, V2 crates,
repo-local V2 skills, `docs/developers/dice.md`, and the explicitly retained
infrastructure crates listed in `V1_ARCHIVE.md`. V1 and rejected mixed-root
surfaces remain available through `slug-v1-archive`, `v1-archive`, and
`codex/slugv2` for staged extraction only.

2026-07-22 live-checkout correction: the annotated `slug-v1-archive` tag still
resolves to `e218054d4c796655939b968d90208b185decb352`, but the local
`v1-archive` branch is absent and the archive checker allowlist predates
`app/slug_server_v2`. Stage 0 is therefore not green in the live checkout; its
owner plan records the bounded repair before M0 acceptance.

2026-07-23 baseline-repair acceptance: after a clean read-only ref audit, local
branch `v1-archive` was restored directly at the recorded commit without
moving or replacing any ref. Commit `9897e940` added only the exact
`slug_server_v2`, `slug-agent-orchestration`, and current root-prompt
allowlists. The normal checker, two negative ref-override probes, and
`V2_ARCHIVE_STATUS_REQUIRE_CLEAN=1 scripts/v2_archive_status.sh` all passed;
Sol-low returned `ACCEPT`. M0 is green.

## Non-Negotiables

- Bazel 9 only. No pre-Bazel-9 behavior, no WORKSPACE support, and no legacy
  toolchain-resolution compatibility.
- Bazel source and Bazel tests are the compliance oracle. A parity claim needs
  a local Bazel source citation or an oracle fixture result.
- DICE owns semantic build state. Do not hide semantic discovery inside
  synchronous Starlark-visible APIs.
- REAPI is the execution boundary. BuildBuddy is the primary scaled remote
  development/CI lane; sibling `../actiond` is the preferred hermetic local
  conformance backend; NativeLink remains a useful regression backend. All sit
  behind the same REAPI boundary.
- Bazel invocations may use ordinary RC discovery and consume the user's
  `~/.bazelrc` for BuildBuddy authentication. Agents and inspection tools must
  never read or copy its contents, and credentials or derived secret material
  must never enter this checkout, logs intended for commit, or Git history.
- Slug-local sandbox implementation is deferred until after analysis, exact
  `aquery`, remote execution, and cache correctness. Backend isolation supplied
  by BuildBuddy or actiond does not count as a Slug sandbox implementation.
- Progress is demonstrated by a vertical Bazel-shaped build, not by independent
  identity, parser, DICE-shaped, action, or REAPI data models. A wrapper trait
  or stable-serialization helper is scaffolding until the owner fixture drives
  it through the real runtime boundary.
- V2 output layout targets Bazel-shaped paths. Any deliberate Slug-specific
  divergence must be explicitly documented as an extension, not assumed.
- V1 plans and code are evidence and extraction sources, not the V2 source of
  truth.
- New packets and replans follow
  [slug-v2-plan-authoring-guide.md](./slug-v2-plan-authoring-guide.md): name
  learned facts, decisions and non-decisions, exact/Slug-native/deferred
  classification, natural producer ownership, request/revision behavior,
  memory lifetime, upstream tests, fallback deletion, scope, and stops.

## V1 Material Worth Keeping

Preserve and mine these V1 surfaces:

- DICE-owned bzlmod/replay implementation and tests in `app/slug_bzlmod` and
  `tests/core/bzlmod/test_plan61_guardrails.py`.
- REAPI/NativeLink smoke tests, what-ran evidence, upload/materialization
  checks, and remote action-cache tests from Plans 31 and 34.
- Bazel Starlark API work: `rule(implementation=...)`, `attr.*`, providers,
  depset probes, `ctx.actions`, and selected `cc_common` or `proto_common`
  compatibility surfaces.
- Repository-rule and module-extension lessons, especially lockfile replay,
  repo mapping, watched inputs, and materialization guardrails.
- Plan docs as a bug database for known semantic traps.

Do not import these V1 surfaces without redesign:

- Buck cell identity and fallback cell graph machinery.
- `buck-out` or Buck-shaped output-root assumptions.
- Direct-local executor shortcuts used as compatibility proof.
- Process-global semantic registries, hidden bridges, or fallback scanners that
  bypass DICE ownership.
- BXL or other Buck-derived user surfaces unless deliberately scoped as Slug
  extensions after Bazel compatibility is stable.

## Stage Map

| Stage | Owner Plan | Parallelism | Checkpoint |
|-------|------------|-------------|------------|
| 0 | [00-v1-archive-and-clean-root.md](./slug-v2-subplans/00-v1-archive-and-clean-root.md) | Serial | V1 is tagged/branched, V2 root docs and metadata are active, archive policy is clear. |
| 1 | [01-compliance-oracle-harness.md](./slug-v2-subplans/01-compliance-oracle-harness.md) | Parallel | A fixture runner compares Java Bazel and Slug V2 for exit status, outputs, events, and selected diagnostics. |
| 2 | [02-rust-skeleton-and-runtime-substrate.md](./slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md) | Parallel | Minimal Rust CLI/server skeleton uses actual Buck2 runtime crates without exposing Buck semantics. |
| 3 | [03-bazel-identity-and-layout.md](./slug-v2-subplans/03-bazel-identity-and-layout.md) | Parallel after Stage 2 starts | Labels, repositories, packages, target ids, execroot, and output paths are Bazel-shaped. |
| 4 | [04-starlark-loading-and-build-packages.md](./slug-v2-subplans/04-starlark-loading-and-build-packages.md) | Parallel after Stage 3 basics | `BUILD.bazel` and `.bzl` loading work for small packages with Bazel globals. |
| 5 | [05-bzlmod-and-repository-graph.md](./slug-v2-subplans/05-bzlmod-and-repository-graph.md) | Parallel after Stage 3 basics | Starlark-evaluated `MODULE.bazel`, registry, repo mapping, extensions, repo specs, and lockfile policy are DICE-owned. |
| 6 | [06-analysis-toolchains-and-actions.md](./slug-v2-subplans/06-analysis-toolchains-and-actions.md) | Parallel after Stages 4/5 | Configured-target analysis, toolchains, providers, depsets, and action declarations pass focused oracle fixtures. |
| 7 | [07-reapi-native-execution.md](./slug-v2-subplans/07-reapi-native-execution.md) | Parallel with synthetic actions, then after Stage 6 | Shell and ruleset actions execute through REAPI with upload, AC, materialization, and zero direct-local proof. |
| 8 | [08-ruleset-and-command-conformance.md](./slug-v2-subplans/08-ruleset-and-command-conformance.md) | Query after loading/analysis; execution commands after aquery | `query`, `cquery`, and exact `aquery` pass before ruleset, run, test, and BEP breadth. |
| 9 | [09-v1-extraction-ledger.md](./slug-v2-subplans/09-v1-extraction-ledger.md) | Continuous | Every V1 or Buck2-derived extraction has an owner, oracle proof, and cleanup decision. |
| 10 | [10-bazel-build-and-bootstrap.md](./slug-v2-subplans/10-bazel-build-and-bootstrap.md) | Bazel developer graph may start now; self-hosting follows exact aquery and execution | Bazel 9 builds/tests Slug through BuildBuddy, then Slug reaches a stage1/stage2 self-build fixed point. |

## Current Milestone Overlay

The numbered stage files are ownership boundaries, not the implementation
order. Use this overlay for scheduling new packets:

| Milestone | Required result | Owning stages | Exit gate |
|-----------|-----------------|---------------|-----------|
| M0: archive and baseline health | V1 refs and clean-root checker are truthful; Bazel/Buck2/actiond sources are pinned | 0, 1, 9 | Archive status is green and every new fixture carries immutable provenance. |
| M1: one semantic spine | One daemon-owned DICE instance, immutable request overlays, lazy typed observations, and source certificates serve loading, bzlmod, analysis, and commands | 2, 4, 5 | Two overlapping requests, mutation during computation, final reobservation, atomic retry/publication, create/edit/delete/recreate, and compatible warm reuse pass without a fallback scanner or fresh per-request graph. |
| M2: analysis graph | Recursive configured targets return real providers and deterministic declared actions without execution | 3, 4, 5, 6 | Admitted inputs have complete structural identity/invalidation; named Slug-native ID bytes are explicit. |
| M3: `query` | Bazel 9 unconfigured query semantics evaluate the loading graph | 8, 9 | Admitted non-regex semantics are exact; named regex functions follow the reviewed Slug-native valid-Unicode contract. |
| M4: `cquery` | Configured query reads the same configured-target graph as analysis | 6, 8 | Transitions/providers/graph semantics match; provisional configuration ID bytes are explicitly Slug-native. |
| M5: `aquery` | Action query reads the same Stage 6 action graph and implements Bazel 9.2.0's formatter shapes | 6, 8 | Graph/content/platform relationships match; new family activations include an exact ActionKey projection, with the accepted FileWrite follow-on explicitly queued, while configuration/path tokens remain Slug-native. |
| M6: execution and caching | Stage 6 actions execute and replay only through REAPI | 7 | BuildBuddy and local actiond evidence prove upload, execute, AC, and materialization with zero direct-local actions. |
| M7A: bootstrap-critical command/ruleset breadth | The exact repository sources, rules_rust/provider/toolchain semantics, action kinds/input trees, aquery shapes, and REAPI behavior needed by the Slug bootstrap closure use the accepted graph and executor | 4, 5, 6, 7, 8 | Focused bootstrap-closure fixtures match and Stage 10.3 can compare the ordinary Slug graph without a bootstrap-only path. |
| M8: bootstrap | Bazel-built Slug builds Slug and reaches a self-hosted fixed point | 10 | Stage1 and stage2 action graphs and declared outputs match after only admitted normalization. |
| M7B: remaining command/ruleset breadth | `run`, `test`, BEP, unrelated public rulesets, and command formats not required by the bootstrap closure use the accepted graph and executor | 8 | Focused public fixtures match; stress projects remain supplemental. |
| M9: exact Bazel configuration/output identity bytes | Reproduce Bazel configuration and configured-output byte algorithms in Rust; finish only residual ActionKey families not admitted earlier | 6, 8 | Existing four-domain evidence and new source audits prove exact bytes without JVM production code. |

M3 progress: implementation commit `61ca25db` lands the first accepted
DICE-backed loading-query thin vertical over the root repository, with
Buck2-derived parser/evaluator/traversal seams and retained-daemon execution.
It passes the Bazel 9.2 `query-parser-and-sets` and
`query-loading-thin-vertical` oracle fixtures through Slug. M3 remains open for
the remaining functions, repositories and patterns, ordering modes, and
formatters; this checkpoint must not be described as full query parity.
Oracle commit `5b7806d7` now pins the next accepted behavior packet for
root-repository subtree patterns, `rdeps`, and
`same_pkg_direct_rdeps`. Implementation commit `cdc5af41` passes that oracle
through the retained daemon with prefix-local package enumeration and
Buck2-derived reverse traversal. M3 remains open for the other 13 loading
functions, repository/pattern breadth, ordering modes, and formatters.
Oracle commit `2b73c08d` now pins the next 43-command packet for `allpaths`
and `somepath`, including bounded arbitrary shortest paths and Bazel's
source-backed root-node `somepath` AUTO-order exception. Implementation commit
`7d851ce9` passes that oracle with direct unbounded reverse-traversal reuse,
Buck2-derived compact BFS/parent reconstruction, exact DICE transitions, and
retained-daemon execution. M3 remains open for the other 11 loading functions,
repository/pattern breadth, ordering modes, and formatters.
Oracle commit `e8e1d9ef` now pins the next 42-command ordinary-query packet for
`some` and the shared signed Java-`int` boundary used by `deps`/`rdeps`.
Implementation commit `b25c8aff` now passes that packet through the retained
daemon: `some` selects from the existing insertion-ordered `TargetSet`, while
the shared FULL renderer deterministically topologically orders the final
selected portion of the request-local evaluation graph. The siblings packet
below replaced the initial semantic selected-induced approximation with
recorded evaluation edges while preserving all `some` oracle rows. It also
carries signed `i32` depth/count values through
`deps`/`rdeps` and renders bare-negative syntax safely for UTF-8 input. Worker
and root each passed the six-crate 82-test suite and all five accepted query
fixtures (133/133 rows). M3 remains open for the other ten loading functions,
repository/pattern/order/formatter breadth; `filter` stays deferred until an
exact Java `Pattern` substrate exists.

The siblings/BUILD-file vertical is now landed: fixture base `8c28877b`,
attribute correction `20f88c05`, FULL-provenance oracle `1a3dec16`, and
implementation `d19a9b29`. `QueryNodeKind::BuildFile` uses the exact active
loaded basename, coalesces an exported active BUILD target, and remains a
zero-edge non-rule node. `siblings` evaluates once and deduplicates packages;
request-local `u32`/`Vec`/`SmallMap` evaluation edges follow Bazel
`BlazeQueryEnvironment` and the Buck2 graph pattern, while FULL renders only
those recorded edges with no render-time DICE read. Exact retained-DICE and
daemon transition coverage passed without adding a key, cache, protocol,
filesystem, lock, or global boundary.

The corrected Bazel update/no-update/root runs were `034446-589899`,
`034516-592708`, and `034623-595736`; FULL-provenance discovery/anchored
update/no-update/root runs were `035638-609525`, `035734-612675`,
`035759-615627`, and `035853-619234`. The Slug gate passed 91/91 and all six
query fixtures passed 176/176: worker `040407-626548`, `040411-626572`,
`040414-626601`, `040418-626692`, `040423-626782`, `040427-626870`; root
`040534-628098`, `040540-628123`, `040546-628189`, `040549-628247`,
`040554-628339`, `040558-628428`. M3 now has nine deferred functions;
`filter` remains deferred pending exact Java `Pattern` parity. `buildfiles`
and `loadfiles` remain separate transitive-load/fake-target work.

Gate A of `WP-4-8-m3-build-load-files` is now accepted in `791e26b2`.
The crate-private `app/slug_query_v2/src/provenance.rs` plus its one-line
module declaration provide symmetric real/fake request-local identity in a
checked-`u32` `Vec`/`SmallMap` arena. Each callback delivery is one nonempty
`Arc`-ID batch with a label-first representative; union preserves batches,
intersection retains the left representative, and label-materialized `except`
is symmetric. `siblings` scans every batch for ownership and delayed output
deduplicates labels. Fake `evaluation_graph_label` is `None`; fake nodes remain
printable and zero-edge for later activation. The module is disconnected: no
evaluator, graph, registry, DICE, or function activation changed, so Gate B and
all nine ordinary functions remain deferred. Worker and root independently ran
`CARGO_BUILD_JOBS=1 cargo test -p slug_query_v2` (32 total: 10 provenance, 16
loading-query, 6 parser/registry); Sol-low final review returned `ACCEPT`.

## Two-Tier Work-Packet Contract

`.codex/skills/slug-agent-orchestration/SKILL.md` is the sole operating
contract. It owns routing, packet/reviewer templates, validation ownership, and
bounded logging. The plan-authoring guide owns readiness for a new or materially
revised packet. This plan owns only milestone state and acceptance evidence.

## Retained First Real Bazel Build Integration Gate

This was the first integrated implementation proof after the Stage 2 skeleton.
It is owned here because it crosses the Stage 1-7 boundaries; implementation
and detailed evidence remain in their stage owners. As of the 2026-07-22
direction reset it is a retained regression gate, not the current scheduling
gate. The Current Milestone Overlay controls new work.

The gate is:

1. `slug build` opens a real DICE transaction and evaluates a root
   `MODULE.bazel` and `BUILD.bazel` through starlark-rust.
2. A small package resolves a typed label, evaluates one custom rule, and
   produces a provider plus a shared-DAG depset and declared action.
3. The action becomes serialized REAPI `Command`, `Directory`, and `Action`
   protobufs; it uploads, executes through NativeLink, and materializes the
   declared output.
4. The matching Stage 1 fixture has a checked-in Bazel oracle, proves
   `reapi_actions=1` and `direct_local_actions=0`, and compares the declared
   output digest.
5. Once the daemon exists, an edit to the loaded `.bzl` reruns the affected
   computation in the same daemon for named DICE dependencies.

`simple-rule-action`, `shell-action-reapi`, and `load-invalidation` are the
initial fixture chain. A missing-module probe is separate: Bazel 9 creates an
empty `MODULE.bazel` with a warning, so V2 must not treat a missing module file
as a generic WORKSPACE-only failure.

Do not use this narrow build as proof that Stages 5-8 are structurally accepted.
Stage 9 records the concrete V1/Buck2 reuse that made each segment real, and the
analysis/query/aquery overlay now determines what may advance next.

This integration gate is not one implementation packet. Each packet names the
single numbered gate clause and owner stage it advances; detailed evidence
stays in that stage's plan. Cross-stage interface choices require pre-review.
After the contributing packets are accepted, a final integration packet runs
the complete fixture chain and receives Sol review before this gate is marked
complete. Passing substrate-only tests or one stage's isolated fixture cannot
substitute for that integration review.

### Gate status — 2026-07-16

All five clauses have contributing packets accepted:
1. `simple-rule-action` (clause 4, write action via REAPI) — pass
2. `shell-action-reapi` (clause 4, run_shell via REAPI) — pass
3. `bare-remote-executor-reapi` (clause 4, bare executor) — pass
4. `platform-exec-properties-reapi` (clause 4, platform properties) — pass
5. `load-invalidation` (clause 5, same-daemon DICE invalidation) — pass

The fixture chain (`simple-rule-action`, `shell-action-reapi`,
`load-invalidation`) passes end-to-end through the oracle harness with
NativeLink-backed REAPI execution and the `slug_server_v2` daemon. A final
integration review by Sol is required before the gate is marked complete.

## First Commit Scope

The first V2 implementation commit is documentation and ownership only:

- mark this plan as canonical;
- preserve the V1 roadmap as archive/reference;
- create the V2 subplans;
- update `AGENTS.md` so future workers read this plan first;
- avoid moving source code until the V1 archive tag/branch and V2 root policy
  are explicit.

Do not mix source movement, root reset, or implementation code into this commit.

## Validation

For documentation-only changes:

```bash
git diff --check -- AGENTS.md README.md thoughts/shared/plans
```

For the first real implementation slice, use the validation command in that
slice's subplan and record compact evidence in the owning V2 plan.

## Plan Execution

Plan-following sessions use
`.codex/skills/slug-agent-orchestration/SKILL.md`; Live Status above owns
scheduling. Packet creation and `REPLAN` also apply
[slug-v2-plan-authoring-guide.md](./slug-v2-plan-authoring-guide.md); cross-stage
Zabel-derived work is tracked in
[zabel-adoption-roadmap.md](./slug-v2-subplans/zabel-adoption-roadmap.md).

## Reviewed Next M3 Direction: Build and Load Files (2026-07-23)

Status: Gate A and Gate B are accepted. B1.5 landed exact load diagnostics in
`4428df22`, recoverable DICE load-cycle handling in `237e7cac`, and exhaustive
non-graph CLI/retained-daemon evidence in `d25bc8c0`. B2 landed the reviewed
formatter/protocol boundary in `cb514747`; all 64 rows of the shared Bazel 9.2
fixture are now accepted under Slug.

M3 began with nine deferred ordinary loading-query functions. The reviewed
parent
packet is `WP-4-8-m3-build-load-files`, but it is deliberately split into two
commit gates: (A) `load-provenance-fake-target-substrate`, then (B) activation
of `buildfiles()` and `loadfiles()` only after A is accepted. One combined,
immutable Bazel 9.2 oracle fixture must be generated before either code gate.
The B1 core now activates only those two functions, leaving seven ordinary
functions deferred; `filter` stays
blocked on exact Java `Pattern` compatibility, and attribute/kind/label,
test, visibility, and executable functions remain blocked on their missing
metadata surfaces.

This is a loading-only, root-repository packet. It must model Bazel's full
transitive load graph and its `FakeLoadTarget` behavior, not a source-file
approximation: a fake target prints its `.bzl` label but belongs, for query
operations such as `siblings`, to the package that first consumed it. Uniquing
is label-based within each load-function invocation, while real targets, fake
targets, query-graph nodes, and set operations can meet through separate
paths. Request-local state must preserve enough `(printed label, consuming
package, real/fake)` provenance for the oracle-observed winner; it must not
collapse this to a request-global first-owner rule before both operand orders
and two-consumer cases are generated and reviewed.

Stage 4 owns a compact immutable manifest: each node has a canonical root
label/path, direct children, and transitive fingerprint in shared `Arc`
slices; `LoadedPackage` exposes its BUILD's direct roots/reachable closure
while retaining the corresponding `FrozenModule` lifetime separately. Stage
8 owns request-local fake-node/provenance state; it does not change global
`QueryLabel` identity. `LoadedPackage` semantic equality must include its
direct roots and transitive manifest identity/fingerprint, while frozen-module
pointer/lifetime storage remains excluded. The packet may use the existing DICE
`BzlParseKey`, `BzlModuleEvalKey`, load-label resolution, `PackageLoadKey`,
`PackageListing`, and workspace observations. Any new DICE key requires Sol
pre-review.

`buildfiles` must emit the selected package's active BUILD plus every
transitive load label and the active BUILD companion of every load-label
package; `loadfiles` emits only the transitive load labels. Companion basename
discovery is tracked but parse-independent and must not require a successful
`PackageLoad` for that package. The request-local projection retains only
operand-evaluation edges for FULL output: fake nodes never enter package
graphs, `:all`, or recursive patterns, and neither fake nor synthetic edges
may be added merely to render the result. A function-produced fake target is
zero-edge, so `deps(fake)` returns only itself.

Stop and replan on external-repository mapping, a requirement to silently omit
`.scl`, direct filesystem discovery, a global `QueryLabel` identity rewrite,
whole-workspace scanning, a new DICE key without review, or a claim that a
`.bzl` load cycle succeeds. A loaded label's containing-package BUILD may have
broken syntax or a broken `load()` and must still contribute its discovered
companion basename without a successful `PackageLoad` value; missing selected
loads and `.bzl` cycles are explicit failure-oracle cases.

Oracle evidence now ends at `e8014b25` (`test: isolate fake target set
algebra`): `query-build-load-files-provenance` has 64 Bazel 9.2 commands.
The base 58-row evidence is `8f6f02b3`; the correction adds a singleton
package loading only `//shared:two.bzl`. Update `051423-694832`, Terra clean
`051521-700085`, and root clean `051644-705470` passed; Sol-low returned final
`ACCEPT`. At that oracle checkpoint, nine functions remained deferred and
neither implementation gate had landed. Gate A subsequently landed in
`791e26b2`, B1 core activation landed in `ba457999`, and B2 completed Gate B
in `cb514747`. The oracle proves selected active
BUILD/transitive-load/active-companion `buildfiles`, loads-only `loadfiles`,
fallback/dual/diamond/multi-package/empty/idempotent/deps/failure cases, and
broken companion discovery without package loading.

The source basis is `BinaryOperatorExpression`'s `evalPlus`, `evalMinus`, and
`evalIntersect`, `QueryUtil`'s `TargetKeyExtractor`-keyed set,
`TargetKeyExtractor`, and `SiblingsFunction`: intersection retains the left
representative; equal printed-label `except` removes in both directions; and
union streams both provenance callback batches to `siblings`. The older
fake-left `except` real-`one.bzl` row remains nonempty only for unmatched
transitive `two.bzl`, not asymmetric equality. Stage 8 uses symmetric
label removal and explicit callback batches, never an asymmetric `Eq` or
operator rule.

Within one invocation `seenBzlLabels` label-deduplicates; across separately
evaluated functions one printed fake label can have different consuming
packages. Gate A retains `(printed label, consuming package, real/fake)`.
B1 applies the corrected label-keyed set/batch semantics through a crate-private
generic evaluator with associated `E::Set`: the loading environment owns one
request-local candidate arena and evaluates IDs in callback-preserving batches.
Its `seenPackages` key is the printed candidate package, while `PackageLoad`
and load visitation use the retained owner package; `.bzl` uniqueness and
final-output uniqueness are separate sets. Companion discovery receives the
workspace-root absolute package path and remains DICE-only.

Fake candidates have no dependencies, `siblings` scans every callback batch,
and FULL output selects the first label representative before projecting only
recorded real edges. The change activates exactly `buildfiles` and `loadfiles`,
removes unused public evaluator reexports, and adds no DICE key, global label
identity, filesystem seam, or change outside `slug_query_v2`.
Factored FULL uses `--output=graph --graph:factored`: fake nodes are zero-edge,
direct `buildfiles` omits the selected real BUILD unless another graph observer
materializes it, `deps(buildfiles(...))` includes result nodes, and no
synthetic projection edge is allowed.

Stage 4 half evidence landed in `b0670e33` (`feat: retain load provenance
manifests`), and Stage 8 completes Gate A in `791e26b2` (`feat: add fake target
provenance algebra`). B1 core landed in `ba457999`; B2 completed Gate B in
`cb514747`, and seven ordinary functions remain deferred. Public
`BzlLoadManifest`/`BzlModuleIdentity` retain canonical
label/normalized path, source-order label-first direct IDs, first-seen closure,
and `[u8; 32]` SHA-256 fingerprint. `LoadedPackage` equality now includes
direct roots/reachable closure/fingerprint: BUILD comment/format edits remain
equal, but leaf/direct/transitive edge create-delete-recreate changes then
restores the value. Aligned `FrozenBzlLifetimeEntry` retains every transitive
`FrozenModule` outside equality; identity/path are `Allocative`-accounted and
the opaque frozen module is skipped.

The public companion helper uses only `WorkspaceDirectoryKey`, primary before
fallback, regular or symlink entries, `None` for missing, explicit read errors,
and shared normalized-path validation; it is parse-independent and adds no
key/cache/lock/filesystem/package-load boundary. Worker/root loading tests had
27 integrations (the worker reported 26 by omitting pre-existing
`native_removed`); root also passed 11 `slug_analysis_v2` and 22
`slug_query_v2` integrations. Sol-low accepted corrections for symlinks,
shared validation, non-truncating alignment, edge lifecycle/BUILD
non-over-invalidation, and memory accounting.

For B1, the Terra-high worker and root independently passed
`CARGO_BUILD_JOBS=1 cargo test -p slug_query_v2`: 34 tests (10 unit, 18 loading,
6 registry/parser). Root also passed the serial downstream
`slug_commands_v2`, `slug_server_v2`, and `slug_cli_v2` suite: 11 command,
12 server, and 14 CLI tests, with zero doc tests. Sol-low final review returned
`ACCEPT`. Root removed one transient candidate-package `String` allocation
before the final tests.

`4428df22` gives missing loads Bazel's
`cannot load '<label>': no such file` diagnostic and appends
`compilation of module '<path>' failed` to malformed `.bzl` errors.
`237e7cac` adapts Buck2's lazy cycle-detector pattern into a request-scoped
DICE user detector for `BzlModuleEvalKey`. Its typed result retains both the
acyclic BUILD-to-cycle path and the cycle, renders Bazel's multi-node and
self-edge diagram, poisons the cycle computation so a repair invalidates it,
and proves same-DICE recovery plus a non-cycle diamond. Sol-low required the
blocking path-to-cycle result and returned `ACCEPT`.

`d25bc8c0` accepts B1.5: one CLI regression matches all 57 non-graph oracle
rows exactly, including exit/stdout/stderr behavior, and retained-daemon tests
cover leaf edits, direct/transitive edge switch-delete-recreate, and companion
BUILD priority without over-invalidating `loadfiles`. The full CLI suite passed
14 integration plus 1 unit test; the server suite passed 14 tests; Sol-low
returned `ACCEPT`.

`cb514747` accepts B2 and the complete 64-row fixture. `QueryOutput` retains a
request-local structural selected graph from the evaluation that produced the
labels; one-shot and retained-daemon presentation format that value without
reevaluation or a DICE read. The command/protocol surface supports Bazel's
default factored graph mode, explicit true/false and negated factoring, and
the fixed 512-node label limit. Factoring uses exact predecessor and successor
sets, quotient-edge deduplication, Bazel's lexicographical member-sequence
class comparator, reverse-postorder graph visitation, and minimal
always-quoted DOT labels. A dedicated regression distinguishes member-sequence
ordering from the incorrect joined-label ordering at a literal `\\n`
boundary.

Root passed `cargo fmt --all -- --check`, the four focused graph formatter
tests, the exact seven-row CLI graph matrix plus unfactored coverage, and the
serialized `slug_commands_v2`/`slug_query_v2`/`slug_server_v2`/`slug_cli_v2`
suite: 12 command, 14 query unit, 18 loading-query, 6 parser/registry, 15
server, 14 existing CLI integration, 2 graph integration, and 1 CLI unit
tests. Sol-low accepted the final comparator correction. Gate B is complete;
the next M3 packet must address one of the seven still-deferred ordinary query
functions rather than extending this formatter.

## Authoritative Next M3 Packet: Labels Metadata Foundation (2026-07-23)

`WP-4-8-m3-labels-metadata-foundation` is next. It supersedes tentative
`filter()`: Bazel `RegexFilterExpression` uses Java `Pattern.compile` and
`Matcher.find`, and no exact implementation/reusable dependency is known.
Finite oracle or `fancy-regex`/Rust `regex` agreement is not parity, so filter
remains blocked.

The packet has three serial commits: immutable Bazel oracle, Stage 4 metadata
substrate with no activation, then Stage 8 `labels` activation. Stage 4 replaces
`RuleDefinitionGen::has_deps` with ordered immutable, `Allocative` schema and
coerced-value structures. They retain exact attribute kind/name, query spelling
(`_implicit` becomes `$implicit`), mandatory/default/configurability state,
`Explicit | Default | Implicit` provenance, scalar/list labels, non-label
values, and unevaluated `select()` branches/default/concatenation. Canonical
labels are coerced during package construction; values are not flattened to
the aggregate dependency edge list. Output/output-list attributes retain their
exact label form and create Bazel-shaped generated targets owned by the
declaring rule before query activation. All semantic state participates in
`LoadedPackage` equality.

Stage 8 adds a separate compact attribute projection to `QueryNode` and then
activates only `labels`: rule prerequisites resolve through the existing
demand-loaded package graph, absent/non-label attributes and non-rules are
empty, and label uniqueness follows the query set. Authority is Bazel 9.2
`LabelsFunction`, `BlazeTargetAccessor#getPrerequisites`,
`AggregatingAttributeMapper#getReachableLabels`, and
`AbstractQueryTest#testLabelsOperator` at `8220c619…`. The oracle covers
scalar/list, explicit/default/implicit, missing/non-label, every configurable
branch and default, accepted concatenation, source and generated output labels,
cross-package resolution, order/dedup, compositions, and missing prerequisites.
The attribute projection and generated nodes participate in
`QueryNode`/`UnconfiguredPackageGraph` equality. Same-daemon edits cover each
semantic form while semantically equal/non-semantic formatting reuses values.

Own `slug_loading_v2/{attrs,package}.rs`, then query
`{expr,evaluator,graph}.rs`; add no key, scan, global identity, guessed
configuration, visibility, executable, or tests surface. The only generated
surface admitted is the exact output/output-list target representation required
by `labels`; its ownership, kind, and graph edges must be oracle-backed. Stop
before activation for any missing reachable-label form, output-target
ambiguity, coercion/provenance ambiguity, or query-time Starlark/filesystem
work. Reuse only Buck2 compact utility and traversal shapes; V1/Buck2 `labels`
is unimplemented and reference-only.

Oracle Gate 1 landed in `8dfae99c`: 31 generated Bazel 9.2 rows cover all
seven default public label-bearing attrs; experimental documented-false dormant
attrs are excluded. Select keys are false; valid dedup, two output producers,
generated kind/output→own-generator edges, and fail-fast missing/mandatory
errors are pinned. Worker `…/20260723-071512-784968-bazel` and root
`…/20260723-071641-791259-bazel` passed fixture-list, command-set, staged
diff/provenance/generated/credential-pattern checks; pytest unavailable; Sol
`ACCEPT`. This is Bazel evidence only: 29 rows are eventual Slug CLI gate and
two `label_kind` rows require focused `QueryNodeKind::GeneratedFile` assertions.

Stage 4 Gate A is accepted in `1b7c179c` (`feat: retain loading attribute
metadata`) with no `labels` activation: ordered immutable `Allocative`
seven-label-kind-plus-String schema/values retain defaults, configurability,
provenance/select structure, canonical generated identity/owner, outputs
outside ordinary deps, and semantic equality. Same-DICE tracker proves
`BzlModuleEval` → `PackageLoad` → consumer/observer; a preactivation guard
prevents leakage. Root passed fmt/diff, loading 35/query 39/analysis 11. Sol
corrected six initial blockers and rereviewed `ACCEPT`; root added nested
repeated-prefix ordering regression. Next is Stage 8: 29 CLI plus two
generated-kind assertions, never Slug 31/31 prematurely.

Prerequisite `f3e8ad48` (`feat: load config setting values`) is accepted:
the immutable labels fixture required native `config_setting` keys. The narrow
load-only representation retains sorted compact `values`, gives
`config_setting rule` correct zero edges, and has semantic reorder/change
tests; it performs no configuration evaluation and unsupported attrs fail
closed. Sol `ACCEPT`. Define/flag/constraint/common attrs and matching remain
deferred. Stage 8 `labels` now resumes unchanged at 29 CLI plus two
generated-kind assertions.

Stage 8 `8fec2696` activates exactly `labels(attr, expr)`; six ordinary
functions remain deferred. 29 non-label-kind CLI rows, including two complete
graph stdout rows, are exact; two Bazel-only label-kind rows remain formatter
constraints. QueryNode has compact immutable `Allocative` attrs separate from
deps; selectors retain all branches/default, exclude keys, and generated files
only output→own-generator edges. Package-load QueryError alone adds Bazel
`Evaluation of query`, preserving syntax/unrelated diagnostics one-shot/daemon.
Same-DICE/reuse and schema/value/select/default/output daemon transitions pass:
loading 37, query 42, CLI 21 (1 unit/17 CLI/3 graph), server 15, analysis 11,
fmt/diff. Sol corrected global suffix/fragment graph then selected-graph order;
final `ACCEPT` requires structural classification, exact graph rows,
generated-only ordering, ordinary factored/unfactored regression. M3 stays
open: never claim 31/31. This implementation reused the checked-in oracle and
needed no Bazel invocation; no agent or tool accessed `~/.bazelrc`. Future
Bazel commands may consume it through ordinary RC discovery without inspection.
Archive-status baseline failures (v1-archive/stale allowlists) are unrelated.

## Accepted M3 Packet: Executable Rule Capability (2026-07-23)

`WP-4-8-m3-executables-rule-capability` superseded the labels packet and is now
accepted. Oracle commit `c8e469f5`, Stage 4 substrate `c86fc656`, and Stage 8
activation `69565a29` complete the vertical. Bazel authority is
`ExecutablesFunction`, `BlazeTargetAccessor#isExecutableNonTestRule`, and
`TargetUtils#isExecutableNonTestRule` at `8220c619…`: the predicate is the
per-target `Rule.isExecutable()` / `$is_executable` capability *and* a retained
rule-class name not ending in `_test`. It is never inferred from a BUILD target
name or from a frozen implementation identity.

The generated Bazel 9.2 fixture has 40 commands: 32 semantic
`executables()`/composition/order/graph/diagnostic rows and eight Bazel-only
`label_kind` representation rows pinning five exported Starlark and three
supported native rule-class names. The latter are not Stage 8 formatter
acceptance. Terra update `085202-880190`, clean `085213-881221`, and root clean
`085303-889108` passed; Sol returned `ACCEPT`. The
`test=true, executable=false` row proves accepted syntax and `_test` exclusion,
not capability by itself; pinned `StarlarkRuleClassFunctions#createRule` and
`getTestBaseRule` establish that test still implies executable capability.
Ordinary Bazel RC discovery was allowed, but no agent or tool inspected or
persisted `~/.bazelrc` or BuildBuddy credentials.

Stage 4 retains immutable, `Allocative` `RuleCapability { rule_class:
CompactString, executable: bool }` in each Starlark rule instance and in all
semantic equality paths. `RuleDefinitionGen` must retain the exact exported
`.bzl` rule name through `StarlarkValue::export_as`, following the bounded
Buck2 rule shape and the existing V2 provider `OnceCell`/freeze pattern; the
exported rule name, not a target name, is the class. Gate A proves that export
validation requires test classes to end `_test` and non-test classes not to,
test implies executable, and an executable test is excluded. Supported native
`filegroup`, `alias`, and `config_setting` receive exact class names and
`executable=false`; alias never inherits; source/BUILD/generated nodes are
non-rules. Do not add `test_suite` while its global is absent. Native `genrule`
executable true/false is a separate
oracle/substrate gate: the current-loadable-graph boundary must be stated, and
the packet stops if full native-positive coverage is required rather than
inferring it.

Stage 8 evaluates its sole operand once, filters existing selected rules by
that projection, and adds no edges. It adds no DICE key, filesystem scan,
global classification, configured analysis, provider, regex, visibility, or
tests activation. Oracle and retained-daemon rows cover non-rules, executable
and non-executable rules, executable `_test` exclusion, native negatives,
composition/order/graph/diagnostics, false→true executable, false→true test,
export rename, target rename crossing `_test` without classification change,
formatting reuse, and delete/recreate. Root validation passed 45 query tests,
50 downstream CLI/commands/server tests, formatting, diff checks, and a clean
`slug_cli_v2` build; Sol-low returned final `ACCEPT`. M3 remains open with five
ordinary functions deferred. `WP-0-baseline-repair` subsequently passed; the
Live Status table now owns scheduling.
### M7 repository source-input owner accepted; source-path consumer audit next (2026-08-13)

Independent review accepts `e4292de7`: the private core owner computes only the
accepted root-apparent route carrier, forwards Need, retains the exact completed
predecessor, validates full source association, and constructs the accepted
Bzlmod input certificate once. Focused tests pass; core remains 192/193 only on
the accepted unrelated external-visibility diagnostic baseline. Run only
four-ledger docs packet
`WP-4-5-6-host-repository-source-path-consumer-owner-audit` under
40/300/240/240/820. Audit the exact path/source/materialization/Builtin/loading
call graph and choose one dependency-safe successor or prerequisite REPLAN.
Authorize no Rust, key/store, consumer migration, path/result/source/package/
materialization/I/O, public/command/server, reverse-edge, or JVM work.

### M7 source-path audit selects shared relative-path prerequisite (2026-08-13)

The accepted audit proves an already-projected source-input certificate cannot
preserve legacy invalid-path-before-request-projection ordering, while immediate
path-key migration would also change demand metadata. Run only four-ledger docs
packet `WP-4-5-host-repository-relative-path-owner-design` under
40/240/200/200/680. Freeze a hidden computation-free Bzlmod value over the sole
existing relative-path checker; future Rust is only `source_preparation.rs` and
hidden `lib.rs` exports under 100/240/340 and 11,540/380. Authorize no Rust yet,
key/store, consumer migration, source-input/request construction,
materialization/source/package/I/O, core/loading/command/public, or JVM work.

### M7 repository relative-path owner implementation activated (2026-08-13)

Independent review accepts design `4d96d094`. Implement only Bzlmod
`source_preparation.rs`, hidden `lib.rs` exports, and completion ledgers under
100/240/340 and 11,540/380. Preserve the exact pure value/error/accessor ABI,
sole existing checker, one post-validation Arc allocation, proof, and every
no-caller/key/source/materialization/I/O/core/loading/command/public/JVM stop.

### M7 relative path accepted; path-first core owner designed next (2026-08-13)

Independent review accepts `b46c2c63`; all 359 Bzlmod tests pass. Run only
four-ledger docs packet
`WP-4-5-6-host-root-apparent-repository-source-path-input-owner-design` under
40/300/240/240/820. Freeze a private core key that validates through the pure
owner before any await, then computes only the accepted source-input key and
retains exact path/predecessor identity. Future Rust is three core files under
340/700/1,040 with 840/850 ceilings. No result/source/loading/command/I/O/JVM
behavior is authorized.

### M7 root-apparent source-path input owner implementation activated (2026-08-13)

Independent review accepts design `68349398`. Implement exactly the new core
source-path-input module, minimal source-input sibling seam, private mod line,
and ledgers under 340/700/1,040 with 840/850 ceilings. Preserve path-before-
await ordering, exact path/predecessor ownership, ABI/proof, and every
no-result/source/loading/command/public/I/O/JVM stop.

### M7 source-path input accepted; source observation ownership audited next (2026-08-13)

Independent review accepts `bd337622`: the private core path-first owner
validates before its sole await, forwards Need, retains exact path/predecessor/
request identity, and adds no source or materialization behavior. Run only
four-ledger docs packet
`WP-4-5-6-host-root-repository-source-observation-consumer-owner-audit` under
40/320/240/240/840. Map Builtin catalog-byte ownership and request-backed
materialization/path/file observation, legacy module-name demand scope, exact
Need/error/lifetime boundaries, and choose one smallest dependency-safe
successor or prerequisite REPLAN. Authorize no Rust, key/store, consumer,
materialization/source/package/loading/command/public/I/O/reverse-edge/JVM work.

### M7 source-observation audit selects hidden Bzlmod owner design (2026-08-13)

Independent review accepts audit `b6a90390`. Run only four-ledger docs packet
`WP-4-5-host-repository-source-observation-owner-design` under
40/360/260/260/920. Freeze one hidden Bzlmod key over the accepted source input
and validated path: Builtin delegates once to the pinned catalog owner; Request
delegates once to the private materialization-result owner then existing file
observation. Preserve branch-specific output identity, exact Need/errors, and
no legacy demand metadata. Future Rust is only `source_preparation.rs` and
hidden `lib.rs` under mandatory 420/800/1,220 and 12,250/430. Authorize no
Rust, core/loading/command migration, second result lookup, public API, new I/O,
reverse edge, or JVM work.

### M7 repository source-observation owner implementation activated (2026-08-13)

Independent review accepts design `7ef0c353`. Implement only Bzlmod
`source_preparation.rs`, hidden `lib.rs` exports, and completion ledgers under
420/800/1,220 with 12,250/430 ceilings. Preserve exact hidden ABI, complete
Hash/Eq identity, one Builtin or request-result dependency, branch-specific
values/errors, proof, and all no-caller/core/loading/command/legacy-demand/
second-result/new-I/O/public/reverse-edge/JVM stops.

### M1 private root-host request revision accepted; loading/public audit next (2026-08-13)

Independent ownership and cleanup review accepts `207fe438`. One retained
`Arc<Dice>` now has a private one-file Host request family with immutable
semantic/presentation overlay separation, exact source certificates, final
reobservation, atomic revision/observation commits, stale-terminal suppression,
bounded retry, and shared-work cancellation. The same async nonreentrant owner
closes all five live production commit sites without spanning DICE compute,
Starlark, repository work, or event formatting.

Focused proof passes 7/7, the full crate passes 210 unit and 12 integration
tests with two independently reproduced out-of-packet baseline failures
skipped, and accounting closes at 456/560 production, 648/700 test, and
1,104/1,520 total lines. Strict Clippy and targeted Bazel-Rust validation are
blocked in unchanged `allocative_derive` and the absent `rules_rust`
toolchain; local Clippy has no new-module warning.

Run only docs packet `WP-2A-m1-loading-public-migration-audit` under
40/220/220/480. Map the exact public/daemon-to-loading call chains, including
the accepted direct root exported-source/filegroup source terminal, and
compare that path with root module, BUILD, `.bzl`, and loading-query
candidates. Select one one-file Host migration or prerequisite `REPLAN`.
Authorize no Rust,
snapshot replacement, public activation, new key/store, repository/
materialization, oracle generation, or JVM work.

### M1 audit selects a native-demand revision-publication bridge design (2026-08-13)

The live audit selects the explicit root exported-source branch as the uniquely
smallest public source consumer. After root anchor, package load, target lookup,
and exported-file kind selection, it issues exactly one contained Host
FileBytes demand and already has accepted public terminal evidence. Root module,
BUILD discovery, `.bzl` recursion, query, and external-repository candidates
all require broader source or session ownership.

The source certificate can stay private to core, but the callerless one-entry
publisher cannot replace the native command's full selected path epoch. Event
and demand selection also seals the provisional attempt before final
validation. Run only docs packet
`WP-2A-m1-native-demand-revision-publication-bridge-design` under
40/260/240/540. Freeze full-epoch merge, branch-only revision consumption,
selection-before-lock, atomic current-check/reobserve/selected commit,
sealed-terminal retry suppression, initialization, cleanup, and a future
three-core-file implementation packet. Authorize no Rust, public output/overlap,
lease or repository/materializer change, loading-key migration, oracle
generation, or JVM work.

### M1 native-demand revision-publication bridge implementation activated (2026-08-13)

Independent design review accepts a same-crate three-file bridge. The explicit
single root exported-source branch retains the exact certificate for both
success and source error after existing anchor/package/lookup/kind ordering.
The first native attempt atomically injects the initial revision with its full
native epoch. Selection and updater preparation remain outside the owner;
under it, unchanged source commits the full selected updater plus successor
revision, while changed source commits a one-entry replacement inside the full
command epoch and retries through a reversible sealed-terminal token.

Implement only `runtime/request_revision.rs`, `runtime/dice.rs`, and
`runtime/events.rs` plus completion ledgers under 600 production, 750 test,
and 1,350 total added Rust lines, with a separate 260-ledger-line cap. Preserve
all public bytes and the existing lease/repository lifecycle. No CLI/server,
loading/snapshot,
root-module/BUILD/`.bzl`, external repository, public overlap, new key/store,
oracle, or JVM work is authorized.

### M1 native root-source revision publication accepted; next audit active (2026-08-13)

Commit `f0849151` accepts the private three-file native bridge. Exactly one
syntactically sole-root exported-source success or completed source error
retains an exact certificate after existing anchor/package/lookup/kind
ordering. The first native attempt atomically initializes revision with its
full path epoch. Unchanged finalization publishes the already-prepared full
selected updater; changed source publishes a one-entry replacement inside the
full command epoch and retries through a reversible selected-terminal token.
Multi-target, rule, filegroup, query, external, and loading paths remain
certificate-free.

Focused revision, bridge, multi-target, and terminal-token proof passes. The
bounded full crate passes 220 library and 12 integration tests with the two
independently reproduced inherited failures skipped. Strict Clippy stops first
in unchanged `allocative_derive`; targeted Bazel Rust reaches analysis and
stops on six unchanged missing `slug_bzlmod_v2` `include_bytes!` inputs.
Formatting, diff/artifact hygiene, and independent ownership/event/cleanup
review pass. Conservative accounting closes at 555/600 production, 383/750
test, and 938/1,350 total net Rust lines.

Run only docs packet `WP-2A-m1-next-source-certificate-consumer-audit`.
Select one complete bounded Host source frontier or record its prerequisite.
Authorize no Rust, public overlap, repository/materialization, oracle, or JVM.

### M1 next-consumer audit requires a loading-frontier certificate design (2026-08-13)

The audit activated in `ea36fdcc` finds no second bounded one-observation
consumer after `f0849151`. Selected BUILD loading first resolves package
roots and `BUILD.bazel`/BUILD precedence, then reads bytes, and may recursively
load `.bzl` children. Root MODULE expands an include horizon; one `.bzl`
expands its load closure; direct-local external source also depends on route,
repository result, materialization, package discovery, and source observations.
A selected-file certificate would therefore be partial and stale by design.

The current core-private certificate cannot be produced across loading/Bzlmod
crate boundaries, while moving it without an ownership design risks a reverse
dependency or generic public framework. Record `REPLAN`: define one
app-internal complete frontier representation, its one-way visibility and
carrier, and compute-free batch final validation before another consumer.

Run only docs packet `WP-2A-m1-loading-frontier-certificate-design` under
40/300/260/600. Select one representation owner and one future bounded
consumer. Authorize no Rust, public API/output/overlap, reverse core edge, new
graph/key/store, partial certificate, repository/materializer activation,
oracle generation, watcher, historical Host reads, or JVM work.

### M1 loading-frontier design selects an observed-path key prerequisite (2026-08-13)

The design activated in `c1d875ad` confirms that no package/public terminal
can yet retain a complete frontier. Root package lookup also consumes policy
and repository-ignore sources; root package load first consumes the mutable
MODULE anchor; successful BUILD evaluation may expand through `.bzl` and
glob dependencies.

The lowest missing contract is earlier. `ResolvedPathKey` discards the exact
Lstat/ReadLink arcs used by its state machine, and `HostFileBytesKey` discards
both that prefix and its final FileBytes result. Reconstructing them above
workspace would duplicate the resolver; changing legacy values would widen all
current callers. The active packet forbids selecting new keys, so it records
`REPLAN`.

Run only docs packet `WP-2A-m1-observed-path-frontier-key-design` under
40/260/220/520. Design exactly one doc-hidden workspace observed-resolution
sibling and one Bzlmod-private observed-Host-file sibling, sharing the existing
resolution machine and `PathObservationEpoch`. Authorize no Rust, third key,
legacy migration, loading/core/public caller, repository/module/BUILD/`.bzl`/
glob activation, Cargo/oracle change, watcher, historical Host read, or JVM.

### M1 observed-path frontier sibling-key implementation activated (2026-08-13)

Independent design selects a callerless lower chain without changing legacy
keys. A doc-hidden workspace `ResolvedPathObservationKey` shares the existing
resolution machine and returns complete semantic result/error plus every exact
Lstat/ReadLink observation. A Bzlmod-private
`HostFileBytesObservationKey` consumes it and adds the exact final FileBytes
observation. Need and cancellation publish no carrier.

`PathObservationEpoch` remains the sole retained deterministic
`Arc<SortedMap<...Arc<Result>>>`. Its new shared-pairs API preserves exact
Arcs, coalesces structurally equal duplicate demands, and returns a typed outer
frontier error for conflicting results or operation mismatch. That error is
never a panic or a legacy semantic error.

Implement only workspace `path_observation.rs`, `path_resolution.rs`,
`lib.rs`, and Bzlmod `host_file.rs` under the corrected 380 production,
650 test, and 1,030 total added Rust lines plus 200 completion-ledger lines.
The single cap-only correction is consumed by discriminating proof. Preserve every legacy
key/value/caller and all public behavior. No Cargo/BUILD, third key,
loading/core/public activation, repository/module/BUILD/`.bzl`/glob work,
request finalization, direct/historical Host read, oracle, watcher, or JVM is
authorized.

### M1 observed-path frontier accepted; hierarchical audit active (2026-08-14)

Commit `308b409a` accepts the callerless observed-resolution and observed
Host-file sibling chain. Stable shared-Arc epoch union, complete success/error
prefixes, typed conflict/mismatch outcomes, exact final FileBytes retention,
Need/cancellation suppression, A/B/A, and zero legacy-key activation are
independently accepted. Formatted accounting is 352 production, 394 test, and
746 total net Rust lines; workspace 43, Bzlmod 367 plus integrations, and
downstream core check pass. Strict Clippy and archive status retain only their
named inherited baselines.

Run docs-only packet
`WP-2A-m1-host-loading-frontier-composition-audit` under
40/320/280/640 ledger lines. Starting with repository-ignore and root-module
predecessors before package markers, map complete mutable Host-source closures
and select one bounded private successor or `REPLAN`. Authorize no Rust,
partial frontier, loading/core/public activation, reverse dependency, new
retained container/graph/store, repository/materializer activation,
historical Host read, watcher, oracle, or JVM.
