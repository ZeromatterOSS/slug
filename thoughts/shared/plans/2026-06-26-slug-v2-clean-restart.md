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
| M1: one semantic spine | **partial; observed terminal epoch-association correction design active** | Host loading observations, typed command/event ownership, direct local-override external query loading, the query-only unsupported-cycle boundary in `ea2019f8`, direct-local exported-source build completion in `42f4a64b`, the first private core repository source-observation consumer in `53152727`, the pinned in-flight loading/source-lock oracle in `2ffad088`, the private request-revision/source-certificate vertical in `207fe438`, the sole-root native publication bridge in `f0849151`, the exact callerless observed-path/Host-file frontier in `308b409a`, the accepted observed root REPO, repository-ignore, package-marker, root-module, anchor, root-package source, and recursive Host `.bzl` frontiers in `f2c7305f`, `43adf74b`, `0875728b`, `2640d1c0`, `c6e61d60`, `2225cf99`, and `b9fda97d`; Host-glob listing/boundary, segment, traversal, complete root-package loading, singleton root-package-all publication, observed configured analysis, neutral singleton-root `Single`, public cquery publication, observed external repository routing, routed Host path/source, routed REPO/ignore policy, external package-marker lookup, and direct-local MODULE file, inspection, include-package horizon, recursive preparation, evaluation, repository-package source, external-Bzl evaluation, repository-package-load, loading-query publication, and epoch-shaped source-certificate acceptance are accepted through `bd4fb8db`, `dc6f6e02`, `2bccb48e`, `daf5eef9`, `31a8b1d3`, `69d37ddb`, `941db0d0`, `03f2db3e`, `e4555dca`, `e4ee0a8e`, `2a8dd968`, `33717f27`, `99d78875`, `a61de5d4`, `79248832`, `cc34e31d`, `1815c019`, `ac7b8bdf`, `93f43264`, `a9270586`, `2e1c1334`, and `a4dd40d6` | the reused observed terminal carrier must become the exact command-epoch Arc authority before selection | freeze and accept only the terminal epoch-association correction before retry 6 or the next M1 audit |
| M2: analysis graph | **accepted (Slug-native identity)** | recursive configured analysis, bounded root cquery in `135b0567`, transitions, toolchain context, recursive action closure, and the reviewed complete Rust-native default structural vertical | exact Bazel configuration/output/ActionKey bytes remain deferred to M9 | preserve the accepted structural and digest-domain boundaries |
| M3: `query` | **accepted** | all 16 default functions; default/explicit `label`, graph, `label_kind`, and `package` output; the 18-lane/165-pair Bazel 9.2 `attr()` oracle in `4ea8f6c7`; complete retained descriptors in `83fe6037`; and runtime activation in `ed38f82a` | Sky Query-only functions and non-text formats remain later breadth, not M3 gates | preserve the accepted loading-query graph |
| M4: `cquery` | **accepted** | the same provider/action/edge-bearing configured analysis result, full structural/null Target/Exec identity, transitions, toolchain/delegation topology, forward/reverse graph semantics, admitted formatters, Need/error ordering, and one-shot/daemon recovery | none; remaining expression and topology shapes are later breadth | preserve the accepted configured-query graph |
| M5: `aquery` | **accepted (bounded FileWrite; Slug-native identity/order)** | recursive action ownership, complete structural configuration identity, closure-resolved toolchain-backed FileWrite semantics, exact literal owner order/framing, bounded aspect-free `deps()` owner membership, stable-daemon A/B/A restoration, and sole-candidate selected-implementation action platforms | broader action kinds, expressions, formats, ordinary zero-toolchain owners, and multi-platform choice are later breadth | preserve the admitted FileWrite boundary |
| M6: execution and caching | **accepted (bounded FileWrite)** | the resolved semantic view is the sole FileWrite executor input; canonical inline Directory/Command/Action SHA-256 identity, selected-platform properties, raw-path rejection, one-shot and stable-daemon A/B/A, and zero direct-local actions are accepted | broader actions, input trees, backends, cache/materializer policy, and transport breadth remain later Stage 7 work | preserve the accepted FileWrite handoff |
| M7: command/ruleset breadth | **partial; source-consumer cutover accepted and later breadth parked behind M1** | `53152727` accepts the first private core caller, split compute-terminal ownership, retained predecessor/observation identity, and no public migration | bootstrap-critical repository, rules_rust, action/input-tree, aquery, and REAPI closure remains M7A after the M1 request-revision vertical | resume only after M1 and its just-in-time evidence, then follow M7A -> M8 -> M7B |
| M8: bootstrap | **developer graph accepted; parked behind M7A only** | exact 33-package CLI boundary plus accepted Gates A-B; the 43-test BuildBuddy developer gate is `PROVED_CACHE_ONLY` and `PROVED_RBE` with clean lifecycle; CI explicitly not admitted | the bootstrap closure still needs its repository sources, rules_rust/provider/toolchain semantics, action kinds/input trees, normalized aquery, and REAPI execution/materialization; accepted bounded M2/M5/M6 are no longer the named blocker | begin Stage 10.3/10.4 as soon as the bootstrap-critical M7A closure is accepted; do not wait for run/test/BEP or unrelated public-ruleset breadth |
| M9: exact Bazel identity bytes | deferred | four-domain C0/C1/P0/P1/content/path evidence in `f00e99db` | in-depth Rust-only analysis and reproduction of Bazel configuration checksum, output-directory identity, and ActionKey algorithms | begin only after the functional semantic graph/bootstrap path |

### Current packet

[WP-2A-m1-observed-terminal-epoch-association-correction-design](./slug-v2-subplans/current-packet.md).

### Rust-only semantic-compatibility reset (2026-08-08)

Explicit user direction permanently excludes JVM/Java integration or semantic
delegation. Rust Host observations and valid-Unicode regex/string behavior are
Slug-native; exact Bazel configuration/output-directory/ActionKey bytes move to
M9. Complete structural equality/invalidation remains mandatory for admitted
inputs, unmodeled inputs fail closed, and a namespaced display/path projection
never becomes the semantic key. REAPI/CAS, content, repository, and lockfile
digests remain exact and separate.

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
M9 exact Bazel identity-byte work remains after the functional bootstrap path.

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
| M5: `aquery` | Action query reads the same Stage 6 action graph and implements Bazel 9.2.0's formatter shapes | 6, 8 | Graph/content/platform relationships match after only configuration/path/ActionKey opaque-token normalization. |
| M6: execution and caching | Stage 6 actions execute and replay only through REAPI | 7 | BuildBuddy and local actiond evidence prove upload, execute, AC, and materialization with zero direct-local actions. |
| M7A: bootstrap-critical command/ruleset breadth | The exact repository sources, rules_rust/provider/toolchain semantics, action kinds/input trees, aquery shapes, and REAPI behavior needed by the Slug bootstrap closure use the accepted graph and executor | 4, 5, 6, 7, 8 | Focused bootstrap-closure fixtures match and Stage 10.3 can compare the ordinary Slug graph without a bootstrap-only path. |
| M8: bootstrap | Bazel-built Slug builds Slug and reaches a self-hosted fixed point | 10 | Stage1 and stage2 action graphs and declared outputs match after only admitted normalization. |
| M7B: remaining command/ruleset breadth | `run`, `test`, BEP, unrelated public rulesets, and command formats not required by the bootstrap closure use the accepted graph and executor | 8 | Focused public fixtures match; stress projects remain supplemental. |
| M9: exact Bazel identity bytes | Reproduce Bazel configuration, configured-output, and ActionKey byte algorithms in Rust | 6, 8 | Existing four-domain evidence and new source audits prove exact bytes without JVM production code. |

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
