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
| M7: command/ruleset breadth | **partial; repository context, symbolic-macro lifecycle, subrule loading, FDO command configuration, selected-toolchain context, configured hidden dependencies, direct subrule calls, the first generic fragment category, and dense retained depsets accepted** | accepted M7A loading/repository closure through `c83e70f0f`; symbolic macros/providers and configured namespaces in `e34cfdc7a`/`541fcfaf2`; corrected subrule architecture/loading in `4900ce46b`/`965cfde5e`; lawful FDO command/DICE producer in `4425d3bfb`; generic selected-toolchain context in `ebd19e3b1`; configured hidden dependencies in `2bf86bfa8`; direct invocation/value materialization in `da6865a3b`; real starlark-rust `set` in `cb71a302d`; generic fragment projection in `683538254`; dense depset/action-input owner in `7b0db03e1` | the authentic rules_cc FDO route now stops at generic Args/run/symlink action construction, including effective default action-environment ownership | implement the accepted configured-action-environment prerequisite |
| M8: bootstrap | **developer graph accepted; parked behind M7A only** | exact 33-package CLI boundary plus accepted Gates A-B; the 43-test BuildBuddy developer gate is `PROVED_CACHE_ONLY` and `PROVED_RBE` with clean lifecycle; CI explicitly not admitted | the bootstrap closure still needs its repository sources, rules_rust/provider/toolchain semantics, action kinds/input trees, normalized aquery, and REAPI execution/materialization; accepted bounded M2/M5/M6 are no longer the named blocker | begin Stage 10.3/10.4 as soon as the bootstrap-critical M7A closure is accepted; do not wait for run/test/BEP or unrelated public-ruleset breadth |
| M9: exact Bazel configuration/output identity bytes | deferred | four-domain C0/C1/P0/P1/content/path evidence in `f00e99db` | in-depth Rust-only analysis and reproduction of Bazel configuration checksum and output-directory identity; only residual unadmitted ActionKey families remain here | begin only after the functional semantic graph/bootstrap path |

### Current packet

[WP-6-7A-generic-args-spawn-symlink-category-architecture-r2](./slug-v2-subplans/current-packet.md).

Commit `683538254` terminally accepts generic configured fragment projection.
The successor audit found that authentic rules_cc 0.2.17 forwards a nonempty
transitive `all_files` depset through action tools, activating the retained
depset gate already named by Stages 6 and 9 before generic action construction.
Commit `7b0db03e1` terminally accepts one dense retained topology and typed
File/action-input import seam after whole-local-DAG, performance,
supported-depth equality and both external-owner lifecycle corrections. The
current docs-only packet freezes the complete non-callback
Args/run/run_shell/artifact-symlink category before selecting its bounded FDO
discriminator successor. Initial review returned `REPLAN` for generated-path
classification, effective default action-environment ownership, semantic map
identity and normalized executable/absolute-symlink paths. The corrected
candidate schedules a bounded configured-action-environment prerequisite before
the FDO action successor, classifies generated File bytes Slug-native and uses
canonical maps plus typed Bazel-path normalization. Bazel 9.2 remains semantic
authority. Focused correction rereview and a final narrowed-command-surface
rereview both returned `ACCEPT`; implement only that prerequisite next. Zabel
remains peer design guidance, and
`cc_common`/`cc_internal` remain ordinary BCR Starlark consumers rather than
Rust-native or parser special cases.

### Accepted architecture and previous packet record

Commit `da6865a3b` terminally accepts the complete generic direct-call/value/
lifetime category on top of configured hidden dependencies: compact tagged
Target identity, all admitted hidden shapes, nested authorization, call-token
invalidation, shared action ownership, generic provider lowering, and ordered
implicit/tool edge publication. Parser, `set`, C++ and ruleset-specific behavior
remain untouched.

R1 received `REPLAN`: Bazel's Exec transition copies
`host_compilation_mode` into `compilation_mode`, while Slug preserved target
mode, and the proposed proof did not discriminate every branch/entry of Bazel's
default private API allowlist. R2 received `REPLAN` because its audit used a
newer sibling Bazel checkout for two Rust allowlist entries instead of the
pinned 9.2 tree. R3 received `REPLAN` because starlark-rust cannot raise a
custom error from per-instance dynamic attribute lookup. The corrected R4
freezes the exact 18 main/11 external 9.2 entries plus `_builtins`, separately
inventories all 12 active fragment names, and uses distinct root/subrule
facades. Root uses a fallible static `cpp` field; subrules retain one dynamic,
future-extensible declaration-set facade and explicitly defer only the
unknown-versus-undeclared diagnostic/`hasattr` distinction. Its implemented
candidate projects one shared typed `cpp` value from the sole structural
configuration owner into separately
authorized, cached rule and subrule fragment collections. The bounded category
contains all six FDO-facing methods read by authentic rules_cc 0.2.17 before its
first action call, typed long-form target/host compilation-mode producers, the
bounded host-to-compilation Exec rewrite, and Bazel 9.2's fully inventoried default
private caller restriction. Absolute-path FDO producers, other Exec rewrites,
other fragments and generic action builtins remain explicit later boundaries. `cc_common` and
`cc_internal` remain ordinary BCR Starlark consumers/discriminators, never
parser or Rust-native C++ rule branches; Zabel remains peer guidance only.

Commit `c83e70f0f` terminally accepts the repository-context attribute
implementation. One authenticated immutable input serves ordinary and innate
generated repositories; all thirteen kinds use one normalized recursive
projector with exact explicit/default/implicit precedence, canonical label
objects and order. Full loading/Bzlmod validation, rebuilt CLI replays, caps,
and corrected independent terminal review pass.

Commit `e34cfdc7a` terminally accepts the first category successor. Both `.bzl`
routes now expose default non-finalizer `macro` and nonconstructible
`PackageSpecificationInfo`; BUILD retains only universe-owned `set` among the
six audited names. Macro expansion uses fresh evaluators over the existing
package owner, shares the package print sink, and retains compact definition,
origin, visibility, and namespace-violation identity. Full loading and
analysis-lib validation passed, and independent correction rereview returned
`ACCEPT` after typed-label rule coercion, implementation-type, and inherited-
`**kwargs` gaps were closed.

Commit `541fcfaf2` completes the scheduled configured-analysis successor. It
consumes only the retained namespace-violation fact immediately after target
lookup, rejects before configured semantic publication with Bazel's naming
message, and passes A/B/A plus named cquery/build dependents. The 18-production/
64-proof staged candidate excluded all parked analysis hunks; independent
terminal review returned `ACCEPT`.

Two fresh authenticated rules_rust 0.73.0 replays contain no missing-`macro`
failure and both stop at `subrule` in generated `bazel_features_globals`.
The source audit shows that the first authentic rules_cc subrule is not a token-
only declaration: it owns eight `configuration_field` label defaults, `cpp`
fragment access, hidden rule attachment, and later actions. Authenticated
`cc_toolchain.bzl` additionally requires the annotated `cpp` fields `libc_top`
and `zipper` on ordinary rule attributes plus
`platform_common.TemplateVariableInfo`; the first implementation replay caught
that the original eight-row frozen ledger omitted them. Architecture R1
was replanned because it incorrectly included defining-module identity in the
late-bound carrier, conflated ordered lifting with set-semantic authorization,
left the first successor unbounded, and implied complete FDO invocation. R2
uses typed fragment-class/field/tools-repository identity, separate ordered
hidden rows and set identities, freezes the first loading successor, and names
`args`/`run`/`symlink`/`cc_common.absolute_symlink` as later natural action
families. `cc_common`/`cc_internal` remain downstream generic BCR Starlark
consumers, never parser or Rust C++ rule targets. Bazel 9.2 remains sole
semantic authority; clean Zabel remains peer concept and optimization guidance
only.

Commit `4900ce46b` accepts that corrected architecture after focused
independent rereview. Implement only its frozen first successor: loading
declaration/export, typed FDO `configuration_field` carriers, rule attachment,
ordered sparse hidden rows and set-semantic identity. The deterministic stop is
configured hidden late-bound resolution before invocation or action
publication.

Implementation replay `REPLAN` keeps that architecture and corrects only its
authenticated source ledger: admit ten finite typed `cpp` fields, retain the
two ordinary defaults in one sparse shared rule-owned slice, and load
`TemplateVariableInfo`. Do not resolve any configured late-bound dependency or
claim FDO/C++ invocation.

Terminal implementation review then returned `REPLAN`: the first candidate
flattened away direct attachment roots, retained no addressable frozen callable
route, projected provider predicates with order-sensitive/empty-alternative
semantics, leaked deferred defaults through repository/tag consumers, made
pre-export subrule equality non-reflexive, and omitted persistent
`TemplateVariableInfo` proof. The bounded correction retains separate direct
and transitive semantic sets plus frozen callable routes, canonicalizes the
shared provider identity at both set levels, rejects those consumer leaks, and
adds the missing equality/source regressions without entering configured
resolution or invocation.

The corrected candidate passes 454 library tests, every loading integration
group, an index-only repeat, formatting, cap/isolation checks, and the rebuilt
CLI replay to the unchanged later positional-`module_extension` frontier.
Focused independent rereview returns `ACCEPT`; commit this loading-only packet,
then activate configured hidden-dependency resolution.

Commit `965cfde5e` terminally accepts that loading successor. Configured packet
R2 correctly moved dependency keys, provider/file validation, configured edges,
and loading-query facts before invocation and left override rejection plus
ConfiguredTarget/Artifact/FilesToRun values with the direct-call successor. Its
independent review nevertheless returned `REPLAN`: Slug had no authorized
producer for the non-default native states required to prove the ten exact
`cpp` projections, and a raw mutator would bypass command/DICE semantics.

Commit `4425d3bfb` accepts the thirteen-option FDO closure after full
configuration/command/analysis/server validation and independent terminal
review. R3 plan review corrected three overclaims: a pre-call error cannot
publish configured edges, Bazel file targets are executable, and only raw
`//` is a valid admitted `fdo_optimize` label. R4 consumes the lawful
structural producer, resolves and validates generic target/selected-Exec
children, publishes root loading-query facts, and leaves successful configured
edges/value materialization with the direct-call successor. Bazel 9.2 is
authoritative; Zabel remains peer guidance only.

### M7 complete repository definitions built-in package accepted (2026-08-30)

The first replay after `f747507f6` stopped at missing immutable content
`tools/build_defs/repo/utils.bzl`. Bazel's embedded-tools filegroup and archive
transform establish one complete direct package: `BUILD.repo` is renamed to
`BUILD` and all eight direct `.bzl` files retain their paths. Commit
`3023718a0` imports all nine members verbatim at mode 0644, changes no evaluator
or repository semantics, and updates the sole catalog manifest plus exact
directory/package proofs. Clean Zabel's authenticated repository-relative
embedded source shape informed the design only; Bazel 9.2 remained the content
authority.

### M7 generated-repository route and definition-host owner accepted (2026-08-30)

The reviewed Root/Canonical projection and loading handoff compile, and the
targeted Bzlmod route projection passes. The selected-registry integration
proof resolves `compatibility_repo` to the exact generated canonical name and
reaches the existing generated-repository owner. It fails before definition
source access because the owner-scoped Bzlmod input projection retains a
root-only guard inherited from the older aggregate execution path.

Bazel's canonical `ModuleExtensionId`, per-usage-owner mappings, and
definition-host generated-repository mapping establish the corrected split.
Independent R2 review rejected its use of Slug's pre-override mapping: Bazel
starts from the host's fully substituted mapping before overlaying sibling
generated repositories and current-extension overrides. R3 corrects that
ordering and adds an overridden-import regression. Clean Zabel independently
uses the same conceptual grouping but is not an authority. R3 then passed
focused and full Bzlmod/loading validation plus independent terminal review and
landed as `f747507f6`. Two fresh real replays prove the route/owner objective and
identify the complete direct `tools/build_defs/repo` embedded package as the
next unrelated exact content boundary.

### M7 innate owner certificate accepted; Host capabilities active (2026-08-29)

Commit `7bcac3da3` accepts explicit ordinary/innate classification, singular
owner-relative resolution, authenticated repository-rule certificates,
ordered retained calls, actual `RepoRuleId` versus synthetic MODULE label
conversion, and unchanged generic validation/canonical source routing. Exact
winsdk and direct rules_cc proofs retain one `@@rules_cc+//...` defining label;
the full Bzlmod and loading suites pass under the reviewed caps.

Run only
`WP-4-5-7A-canonical-repository-rule-host-capability-implementation` against
the accepted certificate and its corrected current base blobs. Do not
duplicate innate authentication, add a winsdk special case, or resume
selected-context Rust. Bazel 9.2 remains sole semantic authority; clean Zabel
is concept/optimization guidance only. BCR Starlark owns all rule logic,
including `cc_internal`, and `cc_common` stays a generic Host/provider ABI.

### M7 repository Host capabilities accepted; registration closure active (2026-08-29)

Commit `26a68d61c` accepts generic authenticated `repository_ctx.os`, immutable
full `os.environ`, dynamic `getenv`, staged `file`, sorted per-name Host DICE
dependencies/effect identity, typed retry/discard, and the exact pinned BCR
non-Windows winsdk output. Full loading and direct dependent validation pass;
independent corrected terminal review returned `ACCEPT`.

Run only `WP-4-5-7A-registered-toolchain-generated-repository-proof` under its
proof-only four-file live-blob allowlist and 900-line cap. Prove the actual four
`@bazel_tools` registration rows in source order, empty non-Windows row 3 with
no demanded `UnsupportedCatalog`, and unchanged retained selected
implementation, `ctx.toolchains`, action context and REAPI projection. BCR
Starlark remains the rule owner; Zabel remains guidance, not authority.

### M7 registration proof REPLAN; exec-configured loading design active (2026-08-29)

The proof draft authenticates all four exact `@bazel_tools` registrations and
passes generic non-Windows winsdk row-3 realization, warm reuse and Host-
platform transition. The first real REAPI dependent then reaches row 1 and
fails while loading verbatim launcher BUILD: Slug drops the already-parsed
`cfg="exec"`/executable attribute flags from immutable `AttributeSchema` and
rejects the target before recording it.

Do not widen the proof-only packet. Design only
`WP-4-5-7A-exec-configured-label-attribute-loading-design`: retain Target/Exec/
Starlark dependency configuration plus executable identity in loading, admit
package inventory, and fail closed only if configured analysis reaches the
still-unsupported attribute. Preserve BCR bytes, the passing proof draft and
the selected-context candidate. Bazel 9.2 is authority; Zabel is guidance.

### M7 exec-configured loading design accepted; implementation active (2026-08-29)

Commit `291b84c2b` records independent design `ACCEPT` after canonical scheduling
correction. Implement only the frozen five-file loading/analysis prerequisite:
retain Target/Exec/Starlark dependency configuration and the executable bit in
the immutable loading schema, admit package inventory, and fail closed before
configured analysis consumes an unsupported exec/executable edge. Preserve the
parked registration proof and selected-context candidate outside named hunks.

### M7 ordered BCR transform identity accepted; archive realization active (2026-08-30)

Commit `831e574e6` terminally accepts the generic loading prerequisite. Full
serial loading and analysis suites pass, the four-row inventory proof remains
green, and the real REAPI dependent now clears launcher row 1. Row 2 reaches
the authentic `rules_shell@0.6.1` selected BCR RepoSpec and exposes the next
generic repository boundary: absent archive type, nonempty strip prefix,
source-ordered authenticated patch, patch strip 1 and 0664/0775 tar modes.

The proof-only registration packet cannot change repository materialization.
Commit `2a7c9436e` freezes the producer-owned patch order, a narrow RepoSpec
publication identity that adds ordered `remote_patches` keys only for exact
http_archive/git_repository while leaving ordinary map equality unchanged, its
propagation through both route families and the actual materialization-request
cutoff, the complete phase-scratch transform plan, verified overlay/patch/
MODULE order, transform-aware source identity and private-root lifecycle.
Bazel 9.2/BCR remain behavior authority. Clean Zabel informs ordered scratch/
realization ownership only; no rules_shell special case, Rust rule
implementation or `cc_common` semantic owner is permitted.

Commit `01f2802f0` terminally accepts producer-retained source order and the
narrow structural RepoSpec publication identity. Focused route/request/DICE,
full serial Bzlmod and loading gates pass; independent terminal review is
`ACCEPT`. The full core gate's three separately dirty route/event failures do
not cross the selected-patch surface and remain owned by their parked packet.

R1's hermetic archive rows and producer/request/registration dependents pass,
but the fresh-root real REAPI path exposes a leading global PAX header in the
authenticated rules_shell archive. Its only record is the semantic-inert
`comment` key; Bazel's generic tar reader consumes it, while R1's explicitly
PAX-free subset rejects it. Independent terminal review returned `REPLAN`.

Commit `1599d730c` terminally accepts the complete bounded archive owner. Its
focused archive suite passes 32 rows with one intentional disposable-artifact
ignore; the full core run has only the three unchanged dirty route/event
failures. Two fresh-root real REAPI replays now consume authentic rules_shell
PAX/prefix/patch/MODULE bytes and stop identically at absent
`coverage_common`. Independent terminal review is `ACCEPT` and classifies that
symbol as a separate Starlark Host-ABI boundary.

R1 review returned `REPLAN`: Slug's `provides` carrier is user-only
`ProviderId`, its proposed provider tokens conflated callable and noncallable
Bazel providers, it overclaimed stable member-method pointer identity, and it
failed to exclude `@_builtins` `.bzl` from registered bootstrap globals.

Independent correction review accepts R4's addition of the fourth clean-test
user-provider rendering assertion with unchanged production scope and caps.
Implement only `WP-6-7A-testing-bootstrap-loading-implementation-r4` under the
manifest's frozen 500/700/1,200 boundary. Do not add
`ProviderIdentity: Display`, a rules_shell-only token, universal-environment or
parser/`set` changes, a configured coverage stub, or a `cc_common` special case.

### M7 effective repository Host inputs accepted; canonical capabilities active (2026-08-28)

Commit `64878a1be` accepts one immutable effective environment across every
active command lane, canonical no-fallback daemon transport, lower shared per-
name/platform DICE keys, typed cold-name retry, accepted-frontier lifecycle and
cache-safe rejection/cancellation restoration. Terminal review first found
request/argv redaction and restoration/platform proof gaps; the corrected
packet passes all focused owner, cross-crate and lifecycle gates and is
`ACCEPT`.

Run only
`WP-4-5-7A-canonical-repository-rule-host-capability-implementation` under its
exact loading-only allowlist and 650/900/1,550 caps. Reuse canonical `.bzl`
loading, expose generic `repository_ctx.os`/`getenv`/staged `file`, retain
observed Host identity in effect equality, and realize only the exact pinned
non-Windows winsdk branch. Registration proof, Windows realization and the
dirty selected-context candidate remain stopped.

### M7 repository declaration metadata accepted; Host-input architecture active (2026-08-28)

Commit `10e6f1a8b` retains `local`, `configure` and first-occurrence-
deduplicated/set-equal `environ` through the generic definition, frozen export,
call, instantiation and A-B-A DICE identity. All 412 loading tests plus direct
integrations pass; independent terminal review is `ACCEPT`. It performs no
Host read and adds no DICE key.

Run only `WP-4-5-7A-effective-repository-host-input-architecture-r5` at zero
Rust. Freeze a single client-captured effective environment, ProcessHost-owned
OS/architecture, per-variable injected DICE keys, accepted/restored request
lifetime, existing canonical `.bzl` route reuse, staged context effects, and
separate implementation/effect/proof manifests. Bazel 9.2 remains authority;
clean Zabel supplies concept/test guidance only. BCR Starlark owns every rule
including `cc_internal`; `cc_common` remains a generic Host/provider ABI.

Independent reviews REPLAN R1-R4 for missing retained Host identity, successor
blob overlap, compatibility/rc overclaims, cold-absent injected-key discovery,
rollback cache authorization and a core/loading dependency cycle. R5 freezes a
typed monotone environment Need, an accepted name-only frontier, per-name
`Unauthorized | Observed(Some/None)` equality, and doc-hidden shared Bzlmod key
ABIs with core-only production injection and loading-only consumption. R5
terminal review is `ACCEPT`; activate only the frozen Host-input implementation
packet before repository evaluation changes.

### M7 effective repository Host-input architecture accepted; implementation active (2026-08-28)

Commit `3dbd937a4` freezes one client-captured immutable environment, lower
shared per-name/platform InjectedKey ABIs, typed cold-name Needs, an accepted
name-only frontier and cache-safe `Unauthorized | Observed(Some/None)` rollback.
Core alone injects/owns lifecycle; loading later consumes. Independent R5
review is `ACCEPT` after four rejected designs; Bazel 9.2 remains authority and
clean Zabel concept/test guidance only.

Run only `WP-4-5-7A-effective-repository-host-input-implementation` under its
exact shared/commands/CLI/server/core blobs and 1,750/1,900/3,650 caps. Add no
repository evaluator capability or output. Preserve the dirty selected-context
candidate byte-for-byte outside packet deltas.

### M7 exact registered-toolchain catalog accepted; declaration metadata active (2026-08-28)

Commit `87d332cf6` imports the exact pinned launcher, resource, C++ wrapper and
source-launcher catalog closure, corrects four upstream archive modes, and
proves every byte, SHA-256, executable bit, direct directory inventory,
BUILD.tools precedence, negative boundary and byte/mode-sensitive manifest
identity. Full Bzlmod passes 580 unit tests plus all integration binaries;
independent terminal correction review is `ACCEPT`. Clean Zabel informed only
catalog ownership and immutable representation; Bazel 9.2 remained sole byte
and behavior authority.

Run only
`WP-4-5-7A-repository-rule-declaration-metadata-implementation`. Retain
`local`, `configure` and first-occurrence-deduplicated `environ` through the
generic frozen definition, exported projection, call record and instantiated
repository identity. Do not read Host state or execute an effect. BCR Starlark
continues to own all rules including `cc_internal`; `cc_common` remains a
generic host/provider-ABI client.

### M7 selected-context R2 terminal REPLAN; built-in closure design active (2026-08-28)

R2 canonicalizes evaluator scratch by actual toolchain type and proves pointer
identity for two requested aliases; it also restores the full configured
identity assertion and lawfully provisions the local `platforms` proof source.
The focused corrections pass. The REAPI proof then fails at registration row
1, `@@bazel_tools//tools/launcher`, because Slug's exact built-in catalog does
not contain the upstream package.

Pinned `src/MODULE.tools` registers launcher, test, generated
`local_config_winsdk`, and resource-compiler rows in that order. Complete
source tracing shows that exact catalog imports alone are insufficient:
resolution also loads the source-launcher package and ordinary `rules_cc`/
`rules_shell` BCR closures, while non-Windows `local_config_winsdk` needs
generic canonical `use_repo_rule` execution plus declared-environment and
host-OS observations. The current repository effect accepts root-defined rules
and `repository_ctx.file` only. No filtering, stub or ruleset-specific
exception is admissible.

Run only
`WP-4-5-7A-builtin-bazel-tools-registered-toolchain-closure-design`. Freeze
the exact file/hash/mode inventory, four-row DICE/selection trace and bounded
packet sequence. The selected-context candidate and local platforms fixture
remain unchanged. Pinned Bazel 9.2 is sole compatibility authority; clean
Zabel is peer ownership/capability guidance only. BCR Starlark owns all rules
including `cc_internal`; `cc_common` remains a generic host/provider ABI.

### M7 selected-toolchain context R1/R2 terminal REPLAN; R3 accepted (2026-08-30)

The R1 candidate implements the retained ordered context, exact Exec-selected
child analysis through the existing guarded configured-analysis family,
shared publication equality, definition-owned repository mapping and generic
`ctx.toolchains` payload adapter. Full analysis passes; query passes; full core
reproduces only the two documented base failures. Formatting, Rust scope, caps
and archive-baseline checks pass.

R1's one focused correction admits structural Target requester configuration
for top-level rules and structural Exec requester configuration for nested
selected rules. Terminal review then finds a second exactness miss: distinct
requested aliases converging on one actual type are separately materialized in
evaluator scratch, whereas Bazel exposes one `ToolchainInfo` object for that
actual type. The required REAPI suite also reaches exact
`@bazel_tools//tools:build_defs.bzl` but its inline workspace lacks the local
`platforms` module used by existing hermetic configured tests, and an existing
complete configured-identity assertion was weakened.

R2 canonicalized scratch materialization by actual toolchain type, proved
pointer identity for two distinct aliases, and restored the complete structural
stable-serialization assertion. Its one-shot REAPI proof advances beyond the
base terminal but then stops first at rules_shell `attr.label_list(flags=...)`
and, with a disposable
local rules_shell experiment, at embedded `tools/res` recursive glob. Those are
independent later BCR/loading surfaces, so terminal review returns `REPLAN`.

Commit `ebd19e3b1` accepts R3. The retained selected context, nested
Exec-configured children, structural requester identities, shared
`ToolchainInfo` occurrences and evaluator projection pass all owner/direct
dependents from the exact index. A doc-hidden wrapper proves the unchanged
REAPI planner directly without claiming the deferred BCR command closure. Full
single-thread core retains only five base-reproduced failures. Pinned Bazel 9.2
remains authority; clean Zabel remains peer guidance only, BCR Starlark owns all
rule flow including `cc_internal`, and `cc_common` remains a generic
host/provider-ABI client.

### M7 recursive analysis evaluator adapter accepted; category-6 implementation active (2026-08-28)

Terminal review accepts R3. The category-5 implementation now retains one
heap-independent recursive analysis graph, uses shared fresh/frozen/
rematerialized provider and depset classes, preserves complete configured
identity and publication equality, authenticates builtin and user providers,
and exactly canonicalizes a sole different-order depset successor by sharing
the child's physical successor array. The unrelated vendored JSON assertion is
restored byte-for-byte. Focused build-api, loading and analysis suites pass;
the only focused vendored failure and full-core failures reproduce unchanged
at the clean packet base. Formatting, scope, physical/net caps, archive and
terminal representation review are accepted.

Activate only
`WP-4-5-7A-selected-toolchain-context-cutover-implementation`. Its zero-Rust
manifest freezes the ordered mandatory/optional retained context, exec-
configured selected child analysis, provider-occurrence handoff, alias-aware
lookup, DICE cycle/lifetime behavior and exact implementation allowlist/caps.
Independent reserved-architecture review accepts the corrected manifest. The
correction keeps ordinary `ProviderOccurrence` equality Bazel-visible while
requiring one shared publication-equality state across the retained toolchain
context plus a parent DICE cutoff A/B/A regression. Rust may now proceed only
within the packet allowlist and caps.
Pinned Bazel 9.2 remains authority. Clean Zabel remains peer architecture and
optimization guidance only; BCR Starlark owns `cc_internal`, while `cc_common`
is a generic host/provider-ABI client.

### M7 recursive analysis evaluator-adapter R2 REPLAN; R3 selected (2026-08-28)

R2 corrects the shared evaluator classes, complete callable-authenticated
provider views, action/alias ownership, iterative deep conversion and most
generic depset behavior; focused build-api, loading and analysis suites pass.
Terminal correction rereview rejects acceptance because a sole compatible
different-order nonsingleton child remains behind an extra transitive wrapper.
Pinned Bazel 9.2 `NestedSet` dereferences every sole physical successor after
hoisting, so the requested-order root must share the child's internal successor
array. Publication topology equality makes the extra node observable even when
flattening agrees. The candidate also changes a pre-existing vendored JSON
field-order assertion outside the structural-hash scope.

Activate only
`WP-4-5-7A-recursive-analysis-evaluator-adapter-implementation-r3`. Preserve
the complete R2 architecture, canonicalize and prove the sole different-order
successor by shared physical successor identity plus publication equality, and
restore the untouched JSON assertion. No R1/R2 Rust is accepted or committed.
Pinned Bazel 9.2 remains authority; Buck2 utilities and clean Zabel
`0795445f...` remain reuse/concept guidance only. BCR Starlark still owns all
rules and control flow including `cc_internal`; `cc_common` remains a generic
host-ABI client. Independent architecture review is required before Rust
resumes.

### M7 recursive analysis evaluator-adapter REPLAN; R2 selected (2026-08-28)

The R1 candidate preserves the accepted retained Arc graph, complete
configuration payload, publication equality, numeric payloads, dictionary
order and depset topology/alias comparison, and its focused build-api, loading
and analysis suites pass. Terminal review nevertheless rejected its adapter:
fresh providers, `ToolchainInfo` and depsets use different equality/hash classes
from rematerialized values; dependency depsets flatten eagerly and cannot be
transitive inputs; constructor validation is deferred; and configured targets
hide typed/builtin providers. The proof does not discriminate those failures.
No candidate Rust is accepted or committed.

Activate only
`WP-4-5-7A-recursive-analysis-evaluator-adapter-implementation-r2`. Preserve
the accepted retained graph, but make loading own shared fresh/frozen/
rematerialized provider and depset evaluator classes, use native `AllocStruct`
for both fresh and rematerialized structs, and split conversion into an
analysis-owned module. Add the bounded vendored struct-field hash barrier that
matches Bazel's frozen-list/dict-inside-immutable-struct exception without
making lists/dicts or tuples containing them ordinary keys. Depsets validate
and compose without flattening, retain one occurrence token through lowering,
and flatten only in lazy `to_list()`. Correct the sole shared build-api depset
owner to Bazel's source-defined builder depth/hoisting and right-to-left
topological traversal rather than duplicating those algorithms in analysis.
The marker-era `ctx.toolchains` bridge allocates the same loading-owned
`ToolchainInfo` class. Configured targets look up every already-retained
provider variant by authenticated callable identity; typed views are phase-
only and add no second DICE payload.

Pinned Bazel 9.2 remains behavior authority. Buck2 utilities and clean Zabel
`0795445f...` remain concept/optimization guidance only; no evaluator heap,
Zabel layout/store identity or process token becomes publication identity. BCR
Starlark owns all rules/control flow including `cc_internal`; `cc_common` is a
generic host-ABI consumer. Independent design review is required before more
Rust edits.

### M7 configured-selection server projection-proof REPLAN (2026-08-28)

The accepted R4 helper closes all 14 missing-`platforms` failures. Full server
validation then reaches 49 passes and two previously unreachable assertions.
The C0/C1 marker proof lawfully observes four structural projections—target
and selected execution for each command configuration—rather than its old two
target-only projections. R5 permits only that count/message correction in the
same proof file, with no production or identity behavior change.

The other result is the exact inherited stale external-package event-replay
family already documented by full core: `DEP_BUILD_EVENT` precedes the
otherwise exact zero-invalidation exported-source terminal on the final
remote-mode request. R5 leaves that assertion unchanged and admits only this
single server manifestation as a separately reported baseline; all other 50
server tests must pass. Any other event, terminal, invalidation count or
failure stops the packet. Independent Sol design and terminal implementation
reviews return `ACCEPT`. Full server reports exactly 50 passes plus that
unchanged inherited assertion; the same test passes in isolation. All focused
owners/direct dependents, the CLI rebuild, structural/cap gates and archive
audit satisfy R5. The packet is complete.

### M7 configured-selection server-fixture REPLAN (2026-08-28)

The formatted R3 candidate closes every selection-related core regression and
passes all configuration/loading/analysis/query/Bzlmod/command gates. The full
server direct-dependent suite then reports 14 configured build/cquery failures:
each reaches the exact host-platform graph but its workspace lacks the local
`platforms` dependency already required by the accepted core harness. The
server harness cannot materialize the upstream `http_archive` shape.

Activate only
`WP-4-5-7A-provider-independent-configured-toolchain-selection-r4`. Add the
single proof file `app/slug_server_v2/src/tests.rs` at blob
`fe3596b4b4953c1565f72b173bd921cc571a1060` under +120 test lines and zero
production. One helper may install the same minimal local `platforms` module
and append its root dependency/override only for the 14 named configured
daemon fixtures. Do not alter ordinary loading/query/Bzlmod/wire-only
fixtures, command policy, production builtin, repository semantics or daemon
behavior. Full server validation must pass. Independent Sol design review
returns `ACCEPT`; implementation may resume only within R4.

### M7 configured-selection repository-sidecar authority REPLAN (2026-08-28)

Live R2 proof closes configured-platform discovery but exposes one narrower
general core contract gap. A public observed multi-build selecting the
hermetic local platform repository reaches a lawful selected repository
request, while `SelectedDependencySuperset` rejects every repository request
before applying its exact observed-path superset checks. This is native-demand
closure integrity, not a C++ or ruleset-specific surface.

Activate only
`WP-4-5-7A-provider-independent-configured-toolchain-selection-r3`. Permit at
most ten core production lines in the existing terminal-association validator:
strict path-only continues rejecting repository sidecars; selected dependency
superset admits closure-owned request/validation sidecars and retains exact
demand/value/Arc checks; closure-repository mode retains exact path equality.
In-memory proof uses the minimal command-overridden `bazel_tools` and direct
root host platform. Filesystem configured proof keeps the verbatim builtin and
locally supplies only its minimal `platforms` dependency. No public API,
repository owner, selection owner or compatibility claim changes. Independent
Sol design review returns `ACCEPT`; R3 implementation may resume.

### M7 configured-selection downstream-proof authority REPLAN (2026-08-28)

Superseded for active work by the R3 repository-sidecar correction above; the
R2 proof authority otherwise remains intact.

The original nine-file category-4 candidate reaches exact zero-type execution-
platform selection, but full core validation reports 30 failures versus the
two documented inherited baselines. Independent Sol review returns `REPLAN`:
the behavior is required by the exact packet, while its allowlist forbids the
core test fixture and topology/event expectation changes required by its own
direct-dependent gate. Weakening zero-type selection is not an admissible fix.

Activate only
`WP-4-5-7A-provider-independent-configured-toolchain-selection-r3`. Preserve
the generic provider-free resolution owner and all compatibility boundaries.
Add only the three audited core proof surfaces at their exact `c8064b106`
blobs: the `runtime/dice.rs` `#[cfg(test)]` harness, build-command expectations
and cquery expectations under +300/+250/+300 test-only caps. The harness may
install a hermetic minimal command-overridden `bazel_tools` with a direct host
platform only for configured build/cquery fixtures; production retains the
verbatim builtin, BCR graph and default
`@@bazel_tools//tools:host_platform`. No core production owner or public API
may change.

R2 also records one exact correction found by the live candidate: pinned Bazel
9.2 `BuildConfigurationValueTest.starlarkFlagExecScopes` carries project-
scoped flags into exec configurations alongside default/universal scope and
filters only target scope. Correct the already-allowlisted configuration
projection and regression without a new owner or representation.

The corrected packet remains at 1,650 production lines, expands proof/total
caps only to 2,105/3,755, and must close every selection-related core failure.
The two accepted core baselines—workspace-directory Lstat drift after lockfile
publication and stale external-query event replay—and the three tracked-
thoughts archive baseline remain distinct. Independent Sol design review
returns `ACCEPT`; implementation may resume only within R2.

### M7 configured-selection live audit activates bounded implementation (2026-08-28)

Commit `a2dbdb553` freezes the implementation packet after independent terminal
Sol review returned `ACCEPT`.

The post-`c8064b106` live audit finds one complete nine-file category-4 seam.
Loading already retains ordered mandatory/optional requirements, full native
declarations and command-before-MODULE registrations. Configuration owns the
target, host and platform-specific exec identities; the host row needs only the
same canonical-label projection already admitted for the target row. Analysis
owns configured aliases, platform facts, condition batches, configured package
loading and the old singular marker consumer. No provider, evaluator, source
carrier, parser, configuration store or ruleset state is required for
eligibility or selection.

Activate only
`WP-4-5-7A-provider-independent-configured-toolchain-selection`. Add one
legacy/observed resolution family with ordered requested/actual rows, complete
mandatory/optional multi-type filtering, target/exec constraint and target-
setting eligibility, registered/declaration ordering and greatest-coverage
platform choice. Candidate order is command-before-MODULE registered platforms
followed by the configured host fallback, with ordered actual-label
deduplication. Cut
configured rules over to the selected exec platform, while bounding the old
marker payload after selection to exactly one mandatory/no-optional request.
Provider values, general `ctx.toolchains` and implementation analysis under the
exec configuration remain categories 5 and 6.

The packet freezes exact `c8064b106` blobs and line counts for five production
and four proof files under 1,650/1,255/2,905 caps. No new file is authorized.
Pinned Bazel 9.2 remains behavior authority; Zabel remains peer architecture/
optimization guidance only; BCR Starlark owns all rule flow including
`cc_internal`; and `cc_common` remains a generic future host/provider-ABI
client.

### M7 target-platform and complete builtin mapping accepted; selection audit active (2026-08-28)

Commit `c8064b106` accepts the configured target-platform prerequisite and the
complete graph-derived builtin mapping. One shared root-plus-nonroot extension
projection now supplies selected mappings, ordinary routes and extension
ownership without repeated collision-namespace allocation or RepoSpec/source-
metadata coupling. Selected-route mapping semantic failures retain the former
RepoSpec-Need predecessor order. Exact upstream `tools/BUILD` and
`tools/build_defs.bzl` compose through the selected `platforms` and
graph-declared extension mappings.

Configured analysis now resolves the default host platform through
`@bazel_tools//tools:host_platform` to the BCR `platforms` target and its actual
constraint, using the reusable alias, platform, target-platform, condition and
platform-specific exec-configuration owners. Generic external loading admits
already-typed aliases and config settings; unsupported target kinds remain
fail closed. No toolchain selection, provider payload or implementation
analysis is claimed.

Full Bzlmod, loading, analysis, configuration, identity and query suites pass;
the V2 CLI rebuilds; formatting, exact asset hashes/modes, per-file/aggregate
caps and diff hygiene pass. The core suite passes 290 tests with one ignored
and two inherited failures reproduced at clean `c2ec8481e`: workspace-directory
Lstat drift after lockfile publication and an external-query event replay
expectation. Archive status retains only its three tracked-thoughts baseline
failure. Independent terminal Sol review returns `ACCEPT`.

Run only the zero-Rust
`WP-4-5-7A-configured-toolchain-selection-live-allowlist-audit`. Materialize
the exact live file/blob/line allowlist, caps, proof matrix and stops for the
already-frozen complete category-4 selection owner; do not implement Rust
until independent review accepts that packet.

Pinned Bazel 9.2 remains behavior authority. Clean Zabel `0795445f…` remains
peer architecture/optimization guidance only. Buck2-derived Rust owns generic
Starlark syntax/evaluation, BCR-delivered Starlark owns all rule definitions and
control flow including `cc_internal`, and `cc_common` remains only a generic
host/provider-ABI client.

### Corrected target-platform prerequisite R7 design active (2026-08-28)

Commit `ce38f0373` accepts the independently corrected category-4 architecture.
It freezes reusable configured target-platform and platform facts, the sole
configured alias projection, mandatory/optional multi-type selection, actual-
type convergence, declaration/candidate order, platform-specific exec identity
and a post-selection-only marker bridge. Converged registered execution-
platform aliases fail closed at Bazel's pre-selection duplicate-key boundary;
no-common-platform is distinct from a mandatory type absent everywhere.

The first candidate exposed the documented DICE silent-deadlock boundary for
static configured alias cycles: runtime installed only the `.bzl` user cycle
detector. Independent review accepted `REPLAN` to retain the valid candidate
and compose one configured-analysis detector at the shared request owner.

Terminal R4 review found that its exact builtin files cannot realize the
default host alias: the canonical builtin route discards the selected
`bazel_tools` module mapping before the eager `@platforms` load. R4 is not
accepted. Commit `c2ec8481e` accepts the corrected graph-only mapping design.
R5 implementation then exposed graph-declared extension imports required by
exact `tools/BUILD`. Independent R6 review returned `REPLAN`: root usages live
in `RootModuleFiles`, so projecting non-root usages alone and later rerunning
the existing combined extension owner could assign its shared collision
namespace twice. Activate only
`WP-4-5-7A-target-platform-and-exec-configuration-prerequisite-r7`: retain the
reviewed platform/cycle and mapping candidates, project root plus non-root
extension mappings once from complete graph-level inputs, share that result
with ordinary extension ownership, preserve RepoSpec-Need predecessor order,
and prove the default host alias through configured analysis. Toolchain
selection and provider/implementation analysis remain later packets.

Commit `dfb56b9b5` records R7. Independent Sol review returns `ACCEPT`; preserve
selected-graph failure precedence while deferring only completed mapping-
projection semantic failures behind RepoSpec completion.

Pinned Bazel 9.2 remains behavior authority. Clean Zabel `0795445f…` remains
peer architecture/optimization guidance only. Buck2-derived Rust owns generic
Starlark syntax/evaluation, BCR-delivered Starlark owns all rule definitions and
control flow including `cc_internal`, and `cc_common` remains only a generic
host-ABI client.

### M7 selector resolution accepted; contextual command overlays active (2026-08-28)

Commit `3b8a353ef` completes typed build-setting/config-condition category 2.
One loading-owned concatenation primitive and one analysis-owned recursive
resolver cover all admitted attribute shapes; one request-local condition batch
feeds selectors and native toolchain `target_settings`; and selected values
alone produce configured dependencies, transitions, provider lookups and action
closure. Generic `ctx.attr` and `ctx.outputs` replace the marker shortcut without
retaining evaluator or configured-attribute trees.

Root/canonical selector lifecycles, specialization, ambiguity, selected-only
transitions, ordinary `LabelKeyedStringDict` dependency objects, two-phase
toolchain settings, Need/error ordering, cold cancellation and same-graph
recovery pass. Full loading/analysis suites, locked direct-consumer checks,
format/diff/cap gates and the unchanged three-row archive baseline pass.
Independent selector/DICE/evaluator-lifetime review returns `ACCEPT`.

Activate only the zero-Rust
`WP-4-5-7A-contextual-command-overlays-architecture` review. Freeze one shared
immutable command occurrence projection, one post-Bzlmod contextual setting
preparation key, and command-first signed extra-registration expansion through
the existing loading walker. Plan the whole category before implementation and
do not extend the fixed `@@//:setting` bridge.

Pinned Bazel 9.2 remains behavior authority. Clean Zabel `0795445f…` remains
peer architecture/optimization guidance only. Buck2-derived Rust owns generic
syntax/evaluation; BCR-delivered Starlark owns all rule definitions and control
flow including `cc_internal`; `cc_common` remains only a generic host-ABI
client.

### M7 direct condition matching accepted; selector resolution active (2026-08-28)

Commit `21ad43d24` adds one borrowed native matcher over the existing typed
option vector, one declaration-owned expected-text matcher for all admitted
Starlark build-setting kinds, and the sole configured-condition DICE key.
Native scalar/list/map and define last-wins behavior, old names, INTERNAL and
NON_CONFIGURABLE rejection, the configured disabled-select set, arbitrary-
precision/base-aware integer text, exact Boolean spellings, collection
membership, canonical repositories and all-empty/constraint failures are
discriminated without a second option or predicate store.

Condition packages and referenced flag declarations invalidate independently;
Need outranks semantic failure after all referenced declarations are demanded,
and cold cancellation publishes no condition result before one same-graph
recovery. Full configuration and analysis tests, locked direct-consumer checks,
format/diff hygiene, exact caps and the known three-row archive baseline pass.
Independent native/DICE/retained-memory review returns `ACCEPT` after correcting
the literal `null` Boolean spelling and selectability coverage.

Run only `WP-4-5-7A-batched-selector-resolution`. Resolve every retained typed
selector/concatenation once, batch direct conditions through the accepted key,
expose the complete admitted resolved `ctx.attr` category, and reuse the same
path for native toolchain `target_settings`. Keep constraint truth, command
occurrences, providers and broader platform/toolchain choice deferred.

Pinned Bazel 9.2 remains behavior authority. Clean Zabel `0795445f…` remains
peer ownership/optimization guidance only. Buck2-derived Rust owns syntax and
generic evaluation; BCR Starlark owns rule flow including `cc_internal`, and
`cc_common` remains a generic host-ABI consumer.

The canonical lifecycle proof records one bounded `REPLAN`: Host
root-package rule attributes must use loading's existing repository-aware
canonicalizer for canonical-external labels. The allowlist therefore includes
only `package.rs` for that routing correction; the legacy listing loader,
unmapped apparent labels and analysis's narrow canonical configured-target
admission remain fail closed.

### M7 typed build-setting value resolution accepted; direct condition matching active (2026-08-27)

Commit `aaf23abcc` deletes the copied-default string bridge and installs one
declaration-authenticated resolver for integer, Boolean, string,
allow-multiple string, ordered string-list and normalized string-set values.
Explicit inputs and transition outputs now derive scope from their canonical
loading declaration, retain only nondefault overrides, preserve unrelated
rows, and expose the effective typed value through generic ephemeral
`ctx.build_setting_value`. Canonical-external lookup, Need/error ordering,
cold cancellation before child publication and same-graph recovery are proved;
independent review returns `ACCEPT`.

Run only `WP-4-5-7A-direct-config-setting-matching`. Add the sole configured-
condition DICE owner and handle the complete direct `values`, `define_values`
and typed `flag_values` category together. Keep nonempty `constraint_values`
fail-closed until category 4 owns the configured target-platform fact. Defer
selectors, command occurrences, providers, platforms and toolchain choice.

Pinned Bazel 9.2 remains behavior authority. Clean Zabel `0795445f…` remains
peer ownership/optimization guidance only. Buck2-derived Rust owns syntax and
generic evaluation; BCR Starlark owns all rule/control flow including
`cc_internal`, and `cc_common` remains a generic host-ABI consumer.

### M7 typed scoped-option migration accepted; value resolution active (2026-08-27)

Commit `84bda1971` replaces the singleton root string slot with the sole sorted
canonical-label-keyed typed option map across configuration, analysis, core,
CLI and server consumers. All five configured value shapes and all four scopes
participate structurally in DICE and version-2 Slug-native projection bytes;
target-to-exec propagation carries default/universal, removes target and rejects
project. Stable canonical-label serialization preserves repository-mapping
provenance, while equal replacement and absent removal preserve backing `Arc`
and configuration identity. Independent review accepts the compact retained
layout and removal of the singleton-era unrelated-row rejection.

Run only `WP-4-5-7A-typed-build-setting-value-resolution`. Authenticate the
legacy explicit bridge and transition outputs against loading declarations,
resolve all five kinds and declaration scope through one shared converter,
elide default-equal rows, and expose typed effective values through
`ctx.build_setting_value`. Authorize no command text/occurrence parsing,
condition/selector matching, provider payload, platform or toolchain choice.

Pinned Bazel 9.2 remains behavior authority. Clean Zabel `0795445f…` remains
peer ownership/optimization guidance only. Buck2-derived Rust owns syntax and
generic evaluation; BCR Starlark owns all rule/control flow including
`cc_internal`, and `cc_common` remains a generic host-ABI consumer.

### M7 typed declaration loading accepted; scoped-option migration active (2026-08-27)

Commit `57b1e8a1f` accepts the loading half of the typed build-setting and
configured-condition architecture. All five Bazel 9.2 Starlark definitions
now retain exact flag/multiple/repeatable shape and publish heap-independent
typed defaults plus the derived magic-scope observation. One loading-owned
native config-setting declaration retains all four predicate fields and
provenance, derives the native query row, and supplies deduplicated root and
external query dependencies without changing declaration order. Full loading
and query suites plus analysis/Bzlmod compile checks pass; independent retained-
representation review accepts the single-owner compact layout.

Run only `WP-4-5-7A-typed-scoped-option-map-migration`. Replace the singleton
root string slot across the configuration identity boundary with one sorted,
canonical-label-keyed typed scoped-option map, migrate every direct consumer in
one no-shim cutover, and version the Slug-native canonical byte projection.
Preserve default-row elision as the contract for the following value-resolution
packet; authorize no loading lookup, command parsing, transition conversion,
condition matching, selector resolution, provider payload or toolchain choice.

Bazel 9.2 remains behavior authority. Clean Zabel `0795445f…` remains peer
architecture/optimization guidance only. Buck2-derived Rust owns syntax; BCR
Starlark owns all rule/control flow including `cc_internal`; `cc_common` remains
a generic evaluator/provider/host-ABI consumer, never a Rust C++ parser.

### M7 typed condition architecture accepted; declaration loading active (2026-08-27)

Commit `b949ce8da` accepts the full-category zero-Rust architecture for all five
Bazel 9.2 Starlark build-setting kinds, typed nondefault configuration values,
magic per-option scope, configured conditions and one selector resolver shared
by ordinary attributes and `toolchain.target_settings`. It preserves signed
32-bit declaration defaults versus arbitrary-precision configured integer
overrides, allow-multiple singleton effective defaults, default-row elision,
and a separately deferred `PROJECT.scl` scope owner. Independent review accepts
the one-map/one-matcher/one-resolver ownership and BCR/Buck2 boundary.

Run only `WP-4-5-7A-build-setting-config-declaration-loading`. Publish the
five rule definitions and derived target default/scope declaration, complete
the one semantic native config-setting predicate, derive its native query row,
and expose flag/constraint labels as loading-query dependencies. Authorize no
configuration map, transition, matching, selector, provider, CLI, platform or
toolchain selection work.

### M7 native toolchain declaration accepted; typed condition architecture active (2026-08-27)

Independent terminal review accepts commit `d9df71392`. Loading now owns every
Bazel 9.2 native `toolchain()` configured-semantic input, including the
unflattened configurable target-settings expression and condition
prerequisites. The derived query row preserves default/explicit provenance,
package equality carries every field, and the legacy/observed marker resolver
rejects deferred target/settings policy before implementation selection. Full
loading, analysis, query and Bzlmod suites pass.

Activate only `WP-4-5-7A-typed-build-setting-condition-architecture`. The
zero-Rust design must replace the singleton string slot with one typed
canonical-label-keyed nondefault override map, cover all five Bazel 9.2
Starlark build-setting kinds, complete native config-setting facts, and freeze
one configured condition/selector path shared by ordinary attributes and
`toolchain.target_settings`. Defaults remain loading declarations rather than
copied configuration rows. Command occurrences, platform/toolchain selection
and provider payloads stay in their later frozen categories.

Bazel 9.2 remains authority. Clean Zabel `0795445f…` remains peer design and
optimization guidance only. The Buck2-derived parser owns syntax, BCR Starlark
owns every rule and control path including `cc_internal`, and `cc_common`
remains a generic evaluator/provider/host-ABI client rather than a Rust C++
parser or rule engine.

### M7 registration cutover accepted; bootstrap toolchain architecture frozen (2026-08-27)

Independent terminal review accepts commit `1f0b396cd`. Configured analysis now
consumes the loading-owned execution-platform and toolchain expansion families
in fixed order, carries canonical labels through the full-identity package
closure, and no longer parses raw root MODULE registrations. The exact
zero-requirement/zero-local-declaration bypass, first-compatible marker
selection, event ownership and root/nonroot identity distinctions remain
proved.

The next ordinary Stage 10.3 probe is not yet honest. Live Bazel 9.2
`rules_rust` toolchains carry target constraints and `target_settings`, the
nightly setting is selected through a canonical Starlark flag, the selected
implementation is a dependency-bearing BCR Starlark rule returning arbitrary
`ToolchainInfo` fields, and command extra-registration options have their own
precedence. The current Slug path retains only execution constraints, accepts
one root string setting and one marker field, and analyzes the implementation
under the owner's configuration.

Freeze the M7A bootstrap toolchain sequence in the Stage 6 owner plan. Loading
first owns the complete native declaration. Typed build-setting/config-condition
identity, per-effective-option scope and matching follow, then contextual
command build-setting and
extra-registration overlays, provider-independent configured eligibility and
selection, one recursive V2-owned analysis-value/provider representation, and
only then selected implementation analysis under the exec configuration plus
the `ctx.toolchains` payload cutover. The value graph is shared by arbitrary
`ToolchainInfo`, user providers and future host-builtin provider families. Each
slice is independently fail-closed; no bootstrap shortcut or rules_rust-
specific parser/store is allowed.

Activate only `WP-4-5-7A-native-toolchain-declaration-semantics`. Preserve
configurable `target_settings` as an expression with condition dependencies,
derive the RuleClass/query view from the semantic declaration, and make the
current marker consumer reject non-default target/settings policy before
implementation analysis. Bazel 9.2 remains authority. Clean Zabel `0795445f…`
is peer design/optimization guidance only. BCR Starlark owns every rule and
control path including `cc_internal`; `cc_common` remains a demanding client of
the shared evaluator/provider/host ABI rather than a Rust C++ rule engine.

### M7 canonical package context accepted; registration cutover resumed (2026-08-27)

Independent terminal review returns `ACCEPT`, and commit `97f2bfeeb` makes
canonical repository BUILD label conversion consume its route-owned selected
mapping and retain final repository/package identities. The full loading,
Bzlmod and query suites pass; the external query consumer category now compares
complete canonical repository plus package identity.

Resume `WP-4-5-7A-expanded-registration-consumer-cutover` from that accepted
prerequisite. Keep its three analysis files as the sole allowlist. Redesign the
selected-nonroot proof around native declarations loaded from canonical BCR
repository packages; do not add root BUILD apparent mapping or analysis-side
label repair. Bazel 9.2 remains authority, Zabel remains peer guidance only,
and BCR Starlark continues to own every rule including `cc_internal`.

### M7 registration cutover REPLAN; canonical package context prerequisite active (2026-08-27)

Correction proof for the expanded-registration consumer exposed a missing
loading prerequisite: canonical repository BUILD evaluation drops its full
package/repository context before string-label conversion. Internal native
references therefore remain provisional root labels, and explicit canonical
references are rejected before configured analysis can consume them.

Activate only `WP-4-5-7A-canonical-package-label-context-prerequisite-r3`. Keep
package-context canonicalization in the loading evaluator, using the existing
canonical route's selected mapping, and retain only final canonical labels in
`LoadedPackage`. The registration-consumer implementation remains parked and
resumes after this prerequisite. Bazel 9.2 remains authority; Zabel supplies
peer ownership guidance only. BCR Starlark continues to own every rule,
including `cc_internal`, while `cc_common` remains a generic host-ABI client.

The first implementation passed loading validation, then the required query
dependent exposed its old provisional-root assumption for same-package
external labels. R2 converts the complete existing external-query consumer
category—visibility, filegroup, alias, test-suite and package-group—to compare
full canonical repository plus package identity. It must not accept both
representations or repair final labels back to root.

Complete query validation then exposed one existing precedence fixture that
used unmapped apparent `@dep` as a stand-in for a different-repository query
edge. R3 admits only that proof file and converts the fixture to explicit
canonical `@@other+`; loading continues to reject absent apparent mappings.

### M7 configured package identity accepted; registration cutover active (2026-08-27)

Independent terminal review returns `ACCEPT`, and commit `58d0d0357` converges
configured package inputs and the retained closure on full `PackageIdentifier`
plus the general loading carrier. Root behavior, outer/Need order and event
ownership remain unchanged; canonical targets and registration families stay
inactive at that commit.

Activate only `WP-4-5-7A-expanded-registration-consumer-cutover`. Compute the
execution-platform and toolchain loading families once per configured owner
unless the exact zero-requirement/zero-local-declaration bypass applies. Carry
their canonical labels through the repository-aware native package closure,
delete the raw MODULE parser, and admit only the represented native and bounded
marker-leaf shapes. BCR Starlark continues to own all rules including
`cc_internal`; `cc_common` remains a generic evaluator/host-ABI consumer, and
Zabel remains peer guidance only.

### M7 repository-aware loading package carrier accepted; configured identity active (2026-08-27)

Independent correction review returns `ACCEPT`, and commit `00f1453ef`
accepts the workspace-plus-`PackageIdentifier` package carrier. Root requests
retain the accepted root-load result `Arc`; canonical requests retain the
accepted route or general-inventory result `Arc`. Observed composition preserves
route-before-inventory order, exact epoch result `Arc`s, typed frontier errors,
child-only event ownership, cancellation, warm reuse and A/B/A restoration.

Activate only `WP-4-5-7A-repository-aware-configured-package-identity`.
Replace analysis's path-only package collection and root-load adapter with the
full-identity carrier while preserving the current root-only admission boundary.
Do not activate registration expansion or canonical configured targets yet.
BCR Starlark continues to own all rules including `cc_internal`; `cc_common`
remains a generic evaluator/host-ABI client. Zabel remains peer guidance only.

### M7 configured registration-consumer architecture accepted; package carrier active (2026-08-27)

Commit `104291321` freezes the repository-aware consumer sequence. A direct
adapter swap is forbidden while configured package closure is path-only and
root-only. The accepted order is a general loading carrier, configured package-
identity convergence, then two-family cutover; alias/provider/settings and
option precedence remain later packets. Registrations are bypassed only when
`!has_toolchain_requirement && local_declarations.is_empty()`.

Activate only `WP-4-5-7A-repository-aware-loading-package-carrier`. Compose
the accepted root package or canonical route plus general inventory beneath a
workspace-plus-`PackageIdentifier` key and observed sibling. Retain child result
and observation `Arc`s, add no analysis consumer or behavior, and preserve BCR
Starlark ownership of all rules including `cc_internal`. Zabel remains peer
guidance only.

### M7 shared registration expander accepted; configured-consumer architecture active (2026-08-27)

Commit `3c6779966` accepts the independently keyed toolchain and execution-
platform MODULE expansion families over one loading driver. Selected/declaration
order, contextual mappings, root/canonical exact/package/recursive expansion,
stable dedupe, family filtering, ambiguity warnings, observed epoch/event
ownership, cancellation and row-terminal precedence are proved. The owner
retains only canonical label and warning `Arc` slices; it performs no configured
activation.

Activate only `WP-4-5-7A-configured-registration-consumer-architecture`.
Freeze the repository-aware loading carrier, configured package-identity and
two-family consumer sequence before changing analysis. The present root-only
analysis package map cannot consume canonical labels without collapsing
repository identity. BCR Starlark still owns all rules including `cc_internal`;
`cc_common` remains a generic evaluator/host-ABI consumer, and Zabel remains
peer design/optimization guidance only.

### M7 registration prerequisite owners accepted; shared expander active (2026-08-27)

Commit `aa79d7736` accepts the one contextual canonical target-pattern grammar,
borrowed selected-mapping point lookup and general Root/Canonical external
package inventory beneath the unchanged consumer-policy adapter. Root/nonroot
`@//`, direct `//`, mapped `@repo` and canonical `@@repo` semantics are proved;
accepted packages share exact result and observation `Arc`s, and inventory
children solely own BUILD events.

Activate only `WP-4-5-7A-shared-module-registration-expander`. Add the two
independently keyed family instances and their one loading driver, retaining
ordered canonical labels and ambiguity facts but adding no configured consumer
or activation. Bazel 9 BCR Starlark owns all rules including `cc_internal`;
`cc_common` remains a reusable host-ABI consumer. Zabel is peer
design/optimization guidance only.

### M7 canonical loading implementation preflight REPLAN; R2 selected (2026-08-27)

The nine-file implementation preflight stopped before Rust edits. Point lookup
through `mapping_target` is sufficient for parsed child `load()` resolution,
but generic evaluation also needs the complete final repository mapping for
Starlark `Label()` construction in BCR modules. Reconstructing that mapping
from loads would silently lose valid labels, and copying it into loading would
create a second semantic owner.

Activate only
`WP-4-5-7A-canonical-loading-source-address-implementation-r2`. Add the
canonical route owner to the allowlist solely for a read-only complete mapping
projection, and prove a mapped `Label()` absent from all `load()` statements.
The corrected ten-file packet retains the same architecture under
1,450-production/2,200-proof/3,650-total caps. It does not change route
production, mapping semantics, BCR rule ownership or the `cc_common` boundary.

### M7 canonical loading source-address design accepted; implementation selected (2026-08-27)

Commit `e47d5d4c8` freezes the independently accepted Stage B architecture. One
Root/Canonical semantic carrier reaches external subtree, repository package,
recursive `.bzl` and cycle owners. Host absolute addresses, built-in
catalog-relative addresses, canonical parser names and Slug-native published
package paths remain distinct domains; producer-owned byte `Arc`s are retained
without copying. Canonical mapped child loads resolve through final mappings
and observe child canonical route/effect before source. Root constructors,
source owners and dependency order remain exact.

Activate only
`WP-4-5-7A-canonical-loading-source-address-implementation` under its exact
nine-file 1,400-production/2,200-proof/3,600-total contract. Bazel 9 BCR
Starlark owns every rule definition and control-flow layer, including
`cc_internal`; `cc_common` is only a demanding reusable host-ABI consumer, not
a Rust parser or rule engine. Builtins remain grouped by reusable capability
category. Zabel informs source/access/runtime-identity separation and compact
ownership only; Bazel 9.2 remains the behavioral authority.

### M7 canonical source/policy Stage A accepted; Stage B address design active (2026-08-27)

Commit `fa896aca4` accepts the compact Root/Canonical Bzlmod policy carrier,
alias-free canonical REPO/ignore/package/source ownership and deletion of all
four temporary canonical source/listing wrappers. Root results, errors, hashes,
events and dependency order remain exact. Canonical built-in selection retains
its exact catalog-relative BUILD address in a typed Slug-native deferred
terminal; no absolute path or apparent alias is fabricated. Full serial
Bzlmod/loading/query and focused core suites, locked CLI, formatting, caps,
archive baseline and independent terminal review pass. The default-parallel
Bzlmod run still exposes an unrelated pre-existing activation-order assertion;
the failing test passes alone and in the complete 577-test serial run.

The implementation review exposed the precise Stage B seam: loading needs a
source-address discriminant and canonical Starlark source-name adapter before
it can consume embedded catalog content. Activate only the docs packet
`WP-4-5-7A-canonical-loading-source-address-design`. Preserve the shared
carrier; separate Host access paths, catalog-relative addresses, canonical
parser names and published-package presentation paths; generalize subtree,
package, recursive `.bzl` and cycle owners without touching query/core
production. Bazel 9 BCR Starlark still owns every rule including
`cc_internal`; `cc_common` remains a generic host-ABI consumer. Zabel is peer
architecture/optimization guidance only.

### M7 repository source-observation owner accepted; corrected policy Stage A active (2026-08-27)

Commit `9764f8a4f` accepts one shared zero-copy Root/Canonical source-observation
owner and its observed sibling. Built-in and materialized payload `Arc`s,
logical paths, hashes, executable metadata, materialization/path ownership and
epoch ordering remain exact. All old root source-file keys, constructors and
loading/core consumers remain unchanged; the four temporary canonical wrappers
delegate to the shared owner. Focused source/load-route and downstream core
proof, full Bzlmod/loading/query suites, locked CLI, formatting, caps, structural
guards and the known three-row archive baseline pass. Independent terminal
review returned `ACCEPT` after restoring the legacy root error accessor.

Activate only
`WP-4-5-7A-canonical-source-policy-convergence-implementation-r3`. Generalize
the Bzlmod path/listing, REPO, ignore, package boundary and BUILD-source chain
over one compact Root/Canonical carrier, preserve root outputs and dependency
order, migrate alias-free canonical callers and delete all four temporary
wrappers. The corrected eleven-file allowlist keeps embedded source addresses
catalog-relative, returns a typed deferred package-source terminal for
canonical built-ins, and adds canonical policy/package cancellation proof.
Stage B owns Root/Canonical source-address and Starlark source-name adaptation.
Bazel 9 BCR Starlark owns all rules including
`cc_internal`; `cc_common` is only a generic host-ABI consumer. Zabel remains
peer architecture/optimization guidance, never behavioral authority.

### M7 source/policy implementation preflight REPLAN (2026-08-27)

Stage A preflight stopped before edits. Existing root source keys have fixed
`HostRepositorySourceFileValue`, while canonical source keys have fixed
`HostRepositorySourceObservation` so built-in catalog values remain
zero-copy. A route enum cannot change a Rust `Key::Value`; projecting built-in
content into the root value would copy bytes and widening the shared result
affects loading/core consumers outside the ten-file allowlist. The accepted
wrapper deletion is therefore impossible in the selected packet.

Activate only docs design
`WP-4-5-7A-repository-source-result-convergence-design`. Audit one shared
zero-copy source-result carrier and its exact consumer ripple, then freeze a
bounded prerequisite before returning to source/policy convergence. Retain all
accepted canonical wrappers meanwhile. No package, `.bzl`, registration,
configured, rule or action behavior is activated.

### M7 source-observation convergence audited (2026-08-27)

The audit found that no new result representation or broad consumer migration
is needed. Existing `HostRepositorySourceObservation::{Builtin, Request}` is
already the zero-copy carrier: it retains either the built-in catalog value or
the materialized root value. The bounded prerequisite instead generalizes only
that observation owner over Root/Canonical source input and adds its observed
epoch sibling. Existing `HostRepositorySourceFileKey` values and every
loading/core exhaustive consumer remain unchanged.

Freeze only the six-file
`WP-4-5-7A-repository-source-observation-owner-convergence` contract recorded
in the current manifest. Temporary canonical source/listing wrappers delegate
to the shared owner but remain until corrected Stage A migrates their policy
callers and tests, then deletes them. Do not hash or copy source bytes, invent
an apparent alias, change package policy, or activate loading/rule/action
behavior. Bazel 9 BCR Starlark continues to own rule control flow including
`cc_internal`; `cc_common` is only a demanding generic host-ABI consumer.
Zabel remains peer architecture/optimization guidance, not behavioral
authority.

Independent design review returned `ACCEPT`, and commit `fdd13400d` freezes the
bounded prerequisite. Activate only
`WP-4-5-7A-repository-source-observation-owner-convergence` under its exact
six-file 800-production/1,000-proof/1,800-total contract. Preserve all legacy
root keys and consumers, retain all four temporary canonical wrappers, and
stop before package policy or loading adaptation.

### M7 package-adapter design accepted; Bzlmod convergence selected (2026-08-27)

Commit `9d55b7157` freezes the independently accepted two-stage package adapter.
Bzlmod first converges root and canonical source/policy ownership and deletes
the temporary canonical source/listing wrappers; loading adaptation follows in
a separate packet. Activate only
`WP-4-5-7A-canonical-source-policy-convergence-implementation` under its
ten-file 1,200-production/1,500-proof caps. Preserve every root constructor and
dependency order; add no loading, package traversal, `.bzl`, registration,
configured, rule or action behavior.

### M7 canonical load route accepted; package-adapter design selected (2026-08-27)

Commit `85593f300` accepts the apparent-free canonical source input, canonical
source/listing projections and loading-owned route. Independent review accepted
the root-unmapped transitive registry proof, independent selected spec/mapping
A/B/A discriminators, route/effect lifecycle, compact retained shape and
shared materialization/path ownership. Focused 12/12, full loading/Bzlmod,
dependent, CLI, formatting and archive-baseline gates pass.

The immediate caller audit found that package policy is a chain, not one key:
REPO, ignore, private lookup, public boundary and package source all retain the
root route, while subtree, external `.bzl` cycle identity and package load do
the same in loading. A single implementation packet would cross two public
DICE boundaries and obscure root compatibility. Activate only the docs design
`WP-4-5-7A-canonical-external-package-loading-adapter-design`; freeze a
Bzlmod source/policy convergence packet followed by a loading/package adapter.
Canonical child `.bzl` loads must resolve through the current canonical mapping
and merge the child canonical route/effect epoch before source. Never fabricate
an apparent alias. Registration, configured semantics, rules and actions stay
deferred.

### M7 canonical load-route implementation R2 REPLAN; R3 selected (2026-08-27)

R2 preserved the accepted six-file architecture, used normal formatting and
passed focused 13/13, full bzlmod 576/576, full loading 352/352, dependent,
locked CLI, formatting and archive-baseline validation. Independent terminal
review rejected two remaining proof-contract gaps: the selected successes were
still reachable through root aliases, and selected repository specification
and mapping identity were not varied independently. No Rust is retained.

Activate only
`WP-4-5-7A-canonical-repository-load-route-implementation-r3`. Use a
transitively selected canonical repository absent from the root mapping and
prove its source and listing success. Independently vary that selected
repository's specification and mapping for equality/hash coverage. Preserve
the 1,200 production and 700/500 module caps, but raise proof to 1,450 lines so
the real registry graph is explicit rather than compressed. The lifecycle
contract also records that effect-only Need after route success is
unconstructible for the admitted `ctx.file`-only ABI; prove route Need/no
effect and semantic effect error instead. No package, registration, configured,
rule, C++-specific or action surface is activated.

### M7 canonical load-route implementation REPLAN; r2 selected (2026-08-27)

The first six-file implementation candidate retained the accepted canonical
source/load architecture and passed focused, full bzlmod/loading, dependent,
CLI, formatting and archive-baseline validation. Its one allowed test-only
correction added selected-registry, lifecycle, hash and built-in dependency
coverage. Independent terminal review nevertheless rejected three remaining
proof gaps: local/immutable/generated source and listing wrappers did not log
their exact deepest owners, route/effect failure prefixes were not frozen, and
route source/mapping/effect-plan plus retained-size discriminators were
incomplete. Per the correction limit, no Rust is retained.

Activate only
`WP-4-5-7A-canonical-repository-load-route-implementation-r2`. Preserve the
same six files and semantic boundary, require the complete four-disposition
source/listing and failure-prefix matrices, and use honest normal formatting:
the rejected 897-line compact candidate measures 1,118 production lines with
its `rustfmt::skip` attributes removed, so r2 caps production/proof at
1,200/1,100 with 700/500 module caps. No package, subtree, target-pattern,
registration, configured, rule or action surface is activated.

### M7 canonical route owner accepted; apparent-free load route selected (2026-08-27)

Commit `496168758` adds the one workspace-plus-canonical-name route and moves
its sole definition/mapping DICE ownership to loading. Root, built-in,
selected-registry, selected-nonregistry and generated dispositions retain their
complete source and mapping facts without a root-apparent alias. Generated
route/mapping lookup retains only the owner/ordinal effect seed and does not
activate source effects. Focused and full bzlmod/loading validation, direct
dependents, locked CLI, formatting and structural gates pass; independent
DICE/retained-representation review returned `ACCEPT`.

The next live trace found that source and package consumers still accept only
`RootRepositoryRoute`. Materialization identity below them is already
workspace plus canonical repository, but directory listing, package boundary,
external subtree and package loading cannot lawfully consume a selected
canonical repository with no root alias. Do not fabricate an apparent name or
duplicate materialization/path ownership.

Activate only `WP-4-5-7A-canonical-repository-load-route-implementation-r2`.
Add one apparent-free canonical source input, one loading route that consumes
generated effects only after canonical-route success, and canonical source-file
and directory-listing projections sharing the existing materialization/path
owners. Keep root-apparent keys exact adapters. Stop before package boundary,
subtree/package loading, target-pattern expansion, registration, configured
semantics, rules or actions.

This remains generic Starlark repository/loading infrastructure. Bazel 9 BCR
Starlark supplies rule definitions and control flow, including `cc_internal`;
`cc_common` is a demanding generic host-ABI consumer, not a Rust C++ parser or
rule engine. Builtins remain planned by reusable category. Zabel informs the
context/source/expansion split and compact ownership only; Bazel 9.2 remains
the behavioral authority.

### M7 external subtree owner accepted; canonical route owner design selected (2026-08-27)

Commit `4fabef5e0` adds the loading-owned external recursive package-set owner
over an authenticated route plus prefix. It preserves ignored pruning versus
deleted descendant traversal, fails symlink/unknown/non-Unicode children
closed, retains one compact lexical Slug-native slice and activates no target
pattern, package-loading, registration or rule surface. Focused and full
loading plus locked bzlmod/query/core/CLI validation pass; independent review
returned `ACCEPT`.

The next target-pattern step exposed a prerequisite. `RootRepositoryRoute` is
root-apparent presentation and admission state, while a selected module's
`//...` pattern is rooted in that module's canonical repository even when no
root alias exists. The accepted selected-before-generated canonical definition
and any-context mapping owners currently live privately in core, where loading
and query cannot reuse them without duplicating semantic DICE keys.

The active docs-only packet therefore freezes a canonical-name-keyed loading
owner before expansion. Bazel 9 BCR Starlark remains the rule/control-flow
source, including `cc_internal`; `cc_common` is a demanding consumer of the
generic host-builtin ABI, not a Rust parser or rule implementation. Builtins
are planned by reusable capability category. Zabel remains peer architectural
and optimization guidance only; Bazel 9.2 owns behavior.

### M7 external package boundary accepted; subtree producer design selected (2026-08-27)

Commit `ee20d5c7c` adds the route/package-keyed public boundary over the sole
private external point lookup. Deleted-package and ignored-directory terminals
are now distinct for later traversal, while existing source/include consumers
preserve their accepted shared failure. The public result exposes only five
semantic states, selected marker spelling and payload-free error tags; observed
form forwards the exact private epoch.

Full bzlmod/loading and locked dependent validation passes at 363 production
and 408 proof additions after the known mixed-horizon flake passed on rerun.
Formatting, scope, dependency, no-lock and archive-baseline gates pass, and
independent DICE/source-boundary review returned `ACCEPT`.

The current docs-only packet freezes one loading producer over the complete
authenticated route plus prefix. It obtains the boundary first, prunes ignored
candidates before any listing, and lists every other candidate through the
accepted routed owner. Symlink, unknown-kind and non-Unicode children fail
closed instead of disappearing; followed symlink traversal remains deferred.
It retains one lexically ordered compact package slice whose order is
Slug-native and activates no query,
registration, package-loading or rule surface. Bazel 9 BCR Starlark remains the
source of `cc_internal`; `cc_common` remains a host-ABI consumer. Zabel informs
the producer/consumer split as peer guidance only.

Independent DICE/retained-representation review returned `ACCEPT` after the
design made ignored-listing nonactivation, typed symlink/unknown/non-Unicode
stops and Slug-native lexical order explicit. Commit `ae26c9a60` freezes the
decision; implement only its bounded three-file producer.

### M7 built-in optional inputs accepted; external boundary resumed (2026-08-27)

Commit `18cd8f35b` projects normal built-in `REPO.bazel`, `.bazelignore` and
BUILD-marker absence/presence through the accepted routed listing. REPO absence
now terminates before root Starlark semantics without injected policy inputs;
ignore and package lookup preserve their natural predecessor order. Exact
file-only `BUILD.bazel` before `BUILD` priority, all admitted catalog packages,
empty observed catalog epochs and route A/B/A identity are proved.

Materialized direct-local, selected-registry and generated paths are unchanged.
The implementation adds no source tree, physical path, retained cache,
dependency, lock, rule or evaluator surface. Full bzlmod/loading suites and
locked dependent checks pass at 252 production and 261 proof additions;
independent DICE/source-boundary review returned `ACCEPT`. The GNU-Windows
no-run failure is an unchanged predecessor test-import defect, not part of this
three-file packet.

Resume the already-reviewed external package-boundary projection at this base.
It remains a generic loading/DICE boundary: Bazel 9 BCR Starlark defines rules,
including `cc_internal`; `cc_common` is only a host-builtin ABI consumer. Zabel
remains peer architectural guidance rather than behavior authority.

### M7 built-in optional-input implementation ordering/cap REPLAN (2026-08-27)

The exact three-file candidate compiles and its focused built-in REPO, ignore,
root/package marker and observed-epoch matrix passes. It measures 252
production and 148 proof additions, exceeding the frozen 220 production
ceiling. Independent cap review also found the test injected policy inputs and
masked that built-in REPO absence still computes root Starlark semantics first.

Freeze only two corrections: move the built-in listing/absence terminal before
REPO-semantics projection, add a no-policy direct regression, and raise the
production cap to 280. Retain the same owners, four-file allowlist, 420 proof
cap and all other semantics/stops. No rule, Starlark builtin, BCR source,
materialized-route behavior or compatibility classification changes.

Independent review returned `ACCEPT`. Commit `caff3bfc1` freezes the no-policy
ordering regression and 280/420 caps. Implement only that correction and the
reviewed retained candidate.

### M7 external boundary implementation REPLAN; built-in optional-input design selected (2026-08-27)

The external package-boundary implementation stopped before acceptance because
catalog-backed `@bazel_tools` routes still send optional `REPO.bazel`,
`.bazelignore` and BUILD-marker checks through materialized-only consumers.
The complete Rust candidate was removed. Converting the built-in exact-source
`UnsupportedCatalog` terminal to absence globally would weaken catalog
integrity.

The current docs-only packet freezes built-in branches in the existing routed
REPO, repository-ignore and package-marker owners. They consume the accepted
routed directory listing for normal optional absence and exact built-in BUILD
membership while materialized direct-local, selected-registry and generated
routes retain their current source/path semantics. No new retained entry key,
physical root, traversal or package evaluation is added.

After this prerequisite, resume the reviewed external package-boundary
projection. Bazel 9 BCR Starlark remains the source of rules including
`cc_internal`; `cc_common` is only a generic host-builtin consumer. Zabel's
directory-presence split informs the architecture as peer guidance only, never
as semantic authority.

Independent architecture/DICE review returned `ACCEPT`. Commit `078518b88`
freezes the built-in-only listing branches, future metadata fail-closed stop,
materialized-route preservation and four-file implementation bounds. Implement
only that correction before resuming the unchanged external boundary packet.

### M7 routed repository directory listing accepted; external boundary design selected (2026-08-27)

Commit `0055c653b` adds one policy-free bzlmod directory-listing owner keyed by
the complete authenticated repository route and root-capable package path.
Direct-local, selected-registry and generated routes privately reuse the
existing materialization and workspace listing owners; built-in `@bazel_tools`
uses its authenticated catalog. The public result contains only sorted direct
entries, repository-relative semantic errors and observations.

The implementation reuses the existing immutable `PathDirectoryEntries`
representation, carries complete source identity in the key, keeps Need
transient and preserves exact observed epochs. It adds no root/namespace leak,
package policy, traversal, dependency, global cache or lock. Full bzlmod and
loading suites, locked core check and rebuilt locked CLI pass at 556 production
and 544 proof additions. Independent DICE/source-boundary review returned
`ACCEPT` after physical error payloads were removed and source/generation A/B/A
plus outer-error proof was added.

The next docs-only packet freezes the smaller external package-boundary
projection required before selected-external traversal. It must distinguish
ignored-subtree from deleted-current-package while consuming the existing
private lookup rather than duplicating policy or marker probes. This remains
generic Starlark/loading infrastructure: Bazel 9 BCR Starlark supplies rules
including `cc_internal`, `cc_common` is only a later host-capability consumer,
and Zabel remains peer design guidance rather than semantic authority.

### M7 selected-external subtree design REPLAN; routed listing prerequisite selected (2026-08-27)

The selected-external design audit stopped before Rust because bzlmod has no
repository-routed directory-listing owner. Workspace `PathDirectoryListingKey`
requires an observation namespace and physical logical path, while route and
materialization owners intentionally keep those details private. Loading
cannot lawfully reconstruct or expose them.

The bounded prerequisite is one bzlmod key over the complete
`RootRepositoryRoute` and validated root-capable `PackagePath`, with
legacy and observed forms. Materialized direct-local, selected-registry and
generated sources project privately through the existing materialization
result into the workspace listing owner. Built-in `@bazel_tools` projects its
authenticated immutable catalog instead of inventing a filesystem root. Both
return the same sorted immutable direct-entry value and expose no physical
path, namespace or catalog internals.

Package deletion, repository ignore and BUILD-marker policy remain in bzlmod.
The current point lookup collapses deleted-current-package and ignored-subtree
to one `Deleted` result, so the later subtree packet must project those states
separately: only repository ignore prunes descendants. Recursive package
discovery remains a later loading owner. This prerequisite activates no
traversal, pattern, registration or Bazel compatibility surface. Its carrier
and error projection are Slug-native; repository content/catalog integrity
remains exact for Slug's actual graph. Zabel's authenticated-source loading
ownership and thin consumer split informed the design only as peer guidance.

Next, implement only the routed directory-listing prerequisite for all four
admitted source dispositions. Stop before selected-external subtree discovery,
target-pattern expansion, family filtering, registration activation or rule
semantics. Bazel 9 BCR Starlark remains the source of `cc_internal` and other
rules; `cc_common` is only a later host-capability consumer.

### M7 root subtree loading owner accepted; selected-external owner design selected (2026-08-27)

Commit `b9736cb47` moves the existing root subtree package-set result, legacy
and observed DICE keys, marker probes and traversal from query into one cohesive
loading module. Query now only converts the loading terminal, merges the
observed epoch and loads discovered packages. No second root traversal remains.

The semantic key inputs, complete-only equality/validity, lexical sort/dedup,
package-root and marker/ignore behavior, cancellation and observed-outer before
accumulated-Need before terminal precedence are unchanged. All 308 loading
units plus integration suites, all 54 query units and 68 query integrations,
the focused multi-root create/edit/delete/restore lifecycle, locked query/core
checks and rebuilt locked CLI pass. The change adds 691 production and 25 proof
lines while removing 593 lines; formatting, diff, scope, cap, dependency,
no-lock and archive-baseline gates pass. Independent DICE ownership review
returned `ACCEPT` after the packet made the preserved terminal order explicit.

The retained value remains one immutable `Arc` slice of compact package names
plus the existing observation epoch. There is no mapping/source copy, new
dependency, interner, global cache or manual lock. Zabel's natural loading-
producer/multiple-consumer split informed the review as peer guidance only.

Next, design the selected-external subtree counterpart. The design must trace
every admitted route/source kind and select a lawful observed producer or a
smaller prerequisite; it must not reconstruct physical roots, add a second
traversal, expand target patterns or activate registrations.

### M7 shared registration-pattern syntax accepted; loading subtree owner selected (2026-08-27)

Commit `e9947e8ba` completes the one shared absolute target-pattern vocabulary.
Package wildcards retain exact `all`, `*` or `all-targets` spelling and
recursive patterns retain their optional suffix, so loading can later resolve
explicit-target ambiguity without inventing a label or reparsing raw text.
Rules-only versus all-target policy and invalid recursive suffixes match the
pinned Bazel 9.2 source and focused eight-row oracle.

Existing `:all` and bare-recursive query/build behavior remains unchanged.
New all-target forms fail closed before package loading or DICE publication,
and external-repository errors retain their accepted precedence. This is one
generic identity/loading boundary for every consumer, not C++ semantics:
Bazel 9 BCR Starlark remains the source of `cc_internal` and other rule
definitions, while `cc_common` is only a host-capability use case.

All 23 identity tests, 54 query units plus 68 query integrations, the focused
core regressions, locked core check and rebuilt locked CLI pass. The Bazel-only
oracle verifies cleanly. One full CLI integration retains the same unavailable-
root-DICE-node failure reproduced at exact predecessor `0cd339800`; the only
packet-caused external-error regression was corrected and its focused test is
green. Formatting, diff, scope, helper and cap gates pass at 147 production and
445 proof additions. Archive status retains only its three known archive-only
paths.

The suffix is one small enum field in the existing command-local pattern; no
raw copy, DICE key, mapping, traversal, interner, dependency or global state is
added. Zabel's shared parser/contextual-resolution split informed the shape as
peer guidance only; Bazel 9.2 remains behavioral authority.

Next, extract the existing root subtree package-set owner from query into
loading without behavior change. Query must reuse it, and the packet must stop
before selected-external traversal, repository mapping, wildcard expansion or
registration activation.

### M7 selected registration owner accepted; shared syntax selected (2026-08-27)

Commit `0cd339800` replaces root and nonroot direct-label registration storage
with one exact raw `ModuleRegistrationPattern` category and one shared collector.
Ignored dev rows are suppressed before their variadic values are inspected.
The public selected projection retains the existing post-extension mapping
result plus checked compact route/pattern ordinals, so the live generated
`@rust_toolchains` apparent name resolves through the declaring module's final
mapping without copying it. Legacy and observed DICE keys preserve complete-only
reuse, semantic A/B/A restoration, epochs and cancellation nonpublication.

The direct-only analysis adapter fails package and recursive wildcards closed
before package publication while preserving the accepted external-error versus
package-Need order. This is generic MODULE/loading infrastructure: it does not
implement `cc_common`, `cc_internal`, C++ rules or rules_rust in Rust. Bazel 9
BCR Starlark remains the rule source; host builtins remain capability surfaces.

All 552 bzlmod units plus integration suites, all analysis and loading suites,
the locked core check and locked CLI build pass. The 1,076 additions split into
607 production and 469 proof lines. Formatting, diff, helper, scope and DICE
no-lock gates pass. The ordinary query moves beyond the former MODULE rejection;
a bounded two-minute replay remained CPU-active without output and was
interrupted, so no query-success or new-terminal claim is made. Archive status
retains only its three known archive-only paths.

The retained-size change replaces each parsed three-component apparent label
with one compact raw string. The selected value adds one predecessor `Arc`
handle, two immutable-slice handles and one 8-byte ordinal pair per declaration;
it copies no raw string, route or mapping. Existing Buck2-derived
`CompactString`, `Arc`, `SmallMap`, `Dupe` and `Allocative` utilities suffice,
so no utility-ledger row or dependency is added. Zabel's analogous split and
packed ordinal lesson remain peer guidance only.

Next, correct the one shared identity parser's complete absolute package and
recursive wildcard vocabulary, retaining the suffix needed for rules-only/all-
target policy and later explicit-target conflict lookup. Add only pinned Bazel
9.2 source/oracle evidence and mechanical consumer exhaustiveness; do not load
packages, resolve mappings, expand patterns or activate registration semantics.

### Complete registration-pattern design accepted; retention owner selected (2026-08-27)

The complete category design separates raw MODULE declarations, selected
canonical owner/mapping views, shared target-pattern syntax, loading-owned
expansion, family-specific filters and configured validation. Both
`register_toolchains` and `register_execution_platforms` use the same layers;
direct, package-wildcard and recursive forms are planned together. Recursive
work reuses a loading-owned subtree-package primitive extracted from query,
and no wildcard becomes a fake label or expands during MODULE evaluation.

The first bounded packet changes only declaration retention and selected owner
projection. It retains compact route/pattern ordinals over the existing
post-extension mapping owner, whose final mappings include generated `use_repo`
names such as `@rust_toolchains`; the dependency-only base route mapping is not
sufficient. Package expansion and configured semantics remain closed. Zabel's
analogous raw-pattern/owner/parser/expansion split is peer guidance only; Bazel
9.2 sources and focused tests are behavioral authority.

### M7 complete JSON/toolchain parent accepted; registration-pattern design selected (2026-08-27)

Commit `0a799e522` installs the adopted starlark-rust JSON module in shared
BUILD/`.bzl` globals and corrects the complete four-method category: positional
or named decode defaults, the `indent` keyword, recursive lexical object-key
ordering and Bazel's token-preserving single-pass formatter. The authenticated
1,002-line rules_rust `rust/private/toolchain.bzl` then freezes over all ten
real children with exact source/hash, load inventory, imported identities,
functions, rules, fragments, toolchain requirements and public/all inventories;
nothing is invoked. This remains generic BCR Starlark loading, not a Rust C++
rule implementation.

All six focused starlark-rust JSON tests, 307 loading units, 32 BUILD-loading
tests, 25 invalidation tests, locked analysis/core checks and the locked CLI
build pass within 131 production, 1,275 proof and 1,406 total additions.
Formatting, diff, caps and helper-size gates pass; archive status retains only
its three known archive-only paths. Zabel's pure shared-global separation was
architecture guidance only; Bazel 9.2 remained exact authority.

The first ordinary `slug query '//...'` bootstrap replay now stops earlier in
root MODULE evaluation because `register_toolchains("@rust_toolchains//:all")`
is incorrectly forced into a direct-label representation. Bazel retains raw
absolute patterns, associates each selected module's canonical repository and
mapping, then expands and filters them later. Design the complete shared
`register_toolchains`/`register_execution_platforms` pattern category before
more Rust so direct, package-wildcard and recursive cases do not create
successive representation churn. Zabel's raw-pattern/declaring-owner/parser/
expansion separation is peer guidance; Bazel source and tests own behavior.

### M7 shared fragments accepted; complete JSON/toolchain parent selected (2026-08-27)

Commit `4a2022764` admits absent, empty list/tuple, arbitrary multiple and
duplicate rule/aspect target-fragment declarations through one shared parser.
First-seen normalized `Arc<[CompactString]>` values survive rule/aspect freeze,
rule values participate in loaded-target equality, and nonempty rule invocation
fails closed until configured fragment producers exist. All 305 loading units,
32 BUILD-loading tests, 25 invalidation tests, locked analysis/core checks and
locked CLI build pass within 43 production, 109 proof and 152 total additions.
Formatting and diff pass; archive status retains only its three known
archive-only paths. Root review returned `ACCEPT`.

The complete rules_rust toolchain attempt then stopped, as required, because
Bazel's shared `json` global is not installed. The adopted starlark-rust
module already supplies all four methods, so the next packet corrects its
Bazel-visible ABI/order/indent gaps and installs it in BUILD and `.bzl`
globals before retrying the same complete parent. Zabel's pure shared-global
ownership is concept guidance only; Bazel 9.2 remains exact authority.

### M7 complete Skylib common-settings accepted; shared fragments/toolchain parent selected (2026-08-27)

Commit `1070b0cf5` freezes all 181 authenticated dependency-free Bazel Skylib
`rules/common_settings.bzl` lines and proves its provider, two shared
attributes, six functions, complete nine-rule typed build-setting family and
exact ten-public/eighteen-all inventories without invocation. The focused proof
and locked direct compile dependent pass within 0/358/358; formatting, diff,
source hash, function size and archive baseline pass. Root review returned
`ACCEPT`.

All ten direct children of authenticated rules_rust
`rust/private/toolchain.bzl` are now complete. Its first missing eager surface
is the generic rule/aspect `fragments` declaration family. Admit arbitrary
string list/tuple declarations with Bazel's first-seen duplicate normalization,
retain them structurally through freeze and loaded-target equality, fail closed
at invocation, then freeze the complete 1,002-line parent over its real child
graph. Zabel's declared/active/typed-producer separation is peer guidance;
Bazel 9.2 and authenticated BCR bytes remain sole exact authority.

### M7 complete incompatible settings accepted; Skylib common-settings family selected (2026-08-27)

Commit `ee15a98c5` freezes all 27 authenticated dependency-free
`rust/settings/incompatible.bzl` lines and proves its provider, private
function, Boolean flag rule, mandatory string attribute and exact two-public/
three-all inventories without invocation. The focused proof and locked direct
compile dependent pass within 0/95/95; formatting, diff, source hash, function
size and archive baseline pass. Root review returned `ACCEPT`.

The only remaining incomplete direct toolchain child is dependency-free
181-line Bazel Skylib `rules/common_settings.bzl` (`f3bcedef…`). Freeze its
complete one-provider/two-attribute/six-function/nine-rule typed build-setting
family under 0/450/450 without invocation. This intentionally handles the
whole declaration category in one packet and removes the synthetic
`BuildSettingInfo` edge before the complete toolchain parent. Zabel remains
peer guidance; Bazel 9.2 and authenticated BCR bytes remain sole authority.

### M7 complete rules_rust semver accepted; incompatible-settings child selected (2026-08-27)

Commit `f7a3a3f10` freezes all 51 authenticated dependency-free `semver.bzl`
lines and proves its sole public function plus exact one-public/one-all
inventories without invocation. The focused proof and locked direct compile
dependent pass within 0/83/83; formatting, diff, source hash, function size and
archive baseline pass. Root review returned `ACCEPT`.

The next direct toolchain load is dependency-free 27-line
`rust/settings/incompatible.bzl` (`534d5103…`). Freeze its documented provider,
Boolean build-setting rule, mandatory string attribute, private function and
exact two-public/three-all inventories under 0/150/150 without invocation.
This remains generic BCR Starlark declaration loading; Zabel is peer guidance
and Bazel 9.2 plus authenticated rules_rust bytes remain sole authority.

### M7 advertised-provider/allocator family accepted; semver child selected (2026-08-27)

Commit `a9610a724` admits absent, empty, list/tuple, multiple and duplicate
advertised user-provider sequences through one shared rule/aspect parser. The
first-seen normalized immutable provider-ID slice survives rule freeze and
loaded-target publication and participates in equality/invalidation; native
providers and configured returned-provider enforcement remain deferred. The
same commit freezes all 302 authenticated `rust_allocator_libraries.bzl` lines
over five complete real children and proves seven imports, eager attributes,
advertised provider, two toolchains, three functions and exact ten-public/
twelve-all inventories without invocation.

All 300 loading units, 32 BUILD-loading tests, 25 invalidation tests, locked
analysis/core checks and CLI build pass within 61 production, 625 proof and 686
total additions. Formatting, diff and source/hash/function-size gates pass;
archive status has only its three known archive-only paths. Root review returned
`ACCEPT`. The retained owner reuses existing `SmallSet`, immutable `Arc` slice,
`ProviderId`, `Dupe` and `Allocative` patterns, so no new Buck2 utility or ledger
row is needed.

The next direct toolchain load is dependency-free 51-line
`rust/private/semver.bzl` (`966fe4b9…`). Freeze its sole public function and
exact one/one inventories under 0/150/150 without invocation. This remains
generic BCR Starlark loading; Zabel is peer guidance and Bazel 9.2 plus
authenticated rules_rust bytes remain sole compatibility authority.

### M7 complete rules_rust LTO accepted; advertised-provider allocator family selected (2026-08-27)

Commit `01920f594` freezes all 120 authenticated `lto.bzl` lines over the
complete utility child and proves its import, ordered modes, provider, rule,
three functions and exact four-public/seven-all inventories without invocation.
Focused and locked direct-dependent validation pass within 0/235/235; broad
loading and integration suites were not repeated immediately after the
unchanged test-only utility checkpoint. Root review returned `ACCEPT`.

The next 302-line toolchain child, `rust_allocator_libraries.bzl`
(`ae4acb50…`), has five complete real children but first requires the generic
Bazel `rule(provides = [...])` declaration argument. Admit that complete list,
export and duplicate-normalization category through the same provider-ID owner
as aspects, retain it in loaded-target equality, then freeze the allocator
module under 100/850/950 without invocation. This is generic BCR declaration
loading, not Rust allocator or C++ rule semantics. Zabel remains peer guidance;
Bazel 9.2 plus authenticated rules_rust bytes remain sole authority.

### M7 complete rules_rust utility family accepted; LTO child selected (2026-08-27)

Commit `22db26f19` freezes all 1,032 authenticated `utils.bzl` lines over five
complete real children and proves 11 import identities, six eager bindings, 39
functions and exact 46-public/56-all inventories without invocation. Focused,
296 library, protected integrations, locked checks and CLI build pass within
0/1,335/1,335; root review returned `ACCEPT`.

The next toolchain child is the 120-line `rust/private/lto.bzl` (`9907a241…`),
whose sole utility dependency is now complete. Run its complete provider/rule,
mode-list, function and inventory proof under 0/350/350 without invocation.
This is generic BCR declaration loading; Zabel remains peer guidance and Bazel
9.2 plus authenticated rules_rust bytes remain sole compatibility authority.

### M7 complete recursive find-toolchain accepted; complete utility family selected (2026-08-27)

Commit `feb0b204c` refreezes authenticated `cc/find_cc_toolchain.bzl` over the
actual complete public `cc_common` chain and proves child identity, eager values,
functions and exact inventories without invocation. Focused, 295 library,
protected integrations, locked checks and CLI build pass within 0/92/92; root
review returned `ACCEPT`.

All five direct children of 1,032-line `rust/private/utils.bzl` (`8aa49b93…`)
are now complete without stubs. Run the whole utility family under
0/2,000/2,000, proving 11 imports, six eager values, 39 functions and exact
46-public/56-all inventories without invocation. This replaces further leaf
slicing with generic recursive BCR loading. Zabel remains peer guidance; Bazel
9.2 and authenticated rules_rust bytes remain sole compatibility authority.

### M7 complete rules_rust common accepted; complete utility family selected (2026-08-27)

Commit `5e7864995` freezes all 85 authenticated lines of
`rust/private/common.bzl` over the complete provider child and proves six import
identities, version strings, private constructor, the ordered eight-field
`rust_common` struct, three-entry provider list and exact inventories without
invocation. Focused, 294 library, protected integrations, locked checks and CLI
build pass within 0/208/208; root review returned `ACCEPT`.

Audit corrects one recursive-boundary assumption before selecting the full
1,032-line `rust/private/utils.bzl`: the accepted authenticated
`cc/find_cc_toolchain.bzl` test used a narrowed `cc_common = struct()` child.
Now that the public `cc_common` route is complete, reprove that exact parent over
the actual child under 0/250/250. This removes the last stubbed utility edge;
the complete utility family follows. Zabel remains peer guidance; Bazel 9.2 and
authenticated rules bytes remain sole compatibility authority.

### M7 complete rules_rust provider family accepted; common facade selected (2026-08-27)

Commit `72d68b3dc` freezes all 238 authenticated lines and all 18 declarations
of dependency-free `rust/private/providers.bzl`, proving exact owners/names,
pairwise identity and inventories without invocation. Focused, 293 library,
protected integrations, locked checks and CLI build pass within 0/296/296;
root review returned `ACCEPT`.

Its 85-line sole consumer child `rust/private/common.bzl` (`cee50122…`) can now
freeze over the actual provider module. Run only its complete import, version,
private-constructor, ordered `rust_common` struct and `COMMON_PROVIDERS` proof
under 0/350/350 without invocation. This is generic BCR value composition;
Zabel remains peer architectural guidance and Bazel 9.2 plus authenticated
rules_rust bytes remain sole compatibility authority.

### M7 complete rules_rust triple accepted; provider family selected (2026-08-27)

Commit `4bce1f88e` freezes all 172 authenticated lines of dependency-free
`rust/platform/triple.bzl` and proves its two public and one private function
bindings plus exact inventories without invocation. Focused, 292 library,
protected integrations, locked checks and CLI build pass within 0/220/220;
root review returned `ACCEPT`.

The next toolchain child, `rust/private/common.bzl`, depends on the complete
dependency-free 238-line `providers.bzl` (`57a59ec9…`). Run all 18 provider
declarations as one family packet under 0/700/700 before composing
`rust_common`. This uses the general provider builtin across the full category,
not Rust-specific host semantics. Zabel remains peer architectural guidance;
Bazel 9.2 and authenticated rules_rust bytes are sole compatibility authority.

### M7 complete public CcInfo accepted; rules_rust triple selected (2026-08-27)

Commit `60a0e5630` freezes the complete 18-line public `CcInfo` wrapper over the
accepted generated symbols child and proves canonical owner/mapping,
private-to-public provider identity and exact inventories without invocation.
Focused, 291 library, protected integrations, locked checks and CLI build pass
within 0/83/83; root review returned `ACCEPT`.

The next direct load in authenticated rules_rust toolchain source order is the
dependency-free 172-line `rust/platform/triple.bzl` (`19fd04c6…`). Run only its
complete freeze and function-visibility proof under 0/400/400 without invoking
triple parsing or repository host observation. This advances generic BCR
Starlark loading, not a special platform or Rust-toolchain parser. Zabel remains
peer architectural guidance; Bazel 9.2 and authenticated rules_rust bytes are
sole compatibility authority.

### M7 complete public cc_common accepted; public CcInfo selected (2026-08-27)

Commit `be1562848` freezes the complete 18-line public `cc_common` wrapper over
the accepted generated symbols child and proves canonical owner/mapping,
private-to-public pointer identity and exact inventories without invocation.
Focused, 290 library, protected integrations, locked checks and CLI build pass
within 0/83/83; root review returned `ACCEPT`.

The second direct rules_rust toolchain load is the analogous authenticated
18-line public `cc/common/cc_info.bzl` (`bac2bc30…`). Its complete generated and
private producer chain is already accepted. Run only its re-export proof under
0/250/250, then resume the rules_rust source-order audit before invoking any
`cc_common` method or provider. This advances generic BCR Starlark loading, not
C++-specific parsing or Rust-side rules. Zabel is peer ownership guidance;
Bazel 9.2 and authenticated rules_cc bytes remain sole compatibility authority.

### M7 complete compatibility symbols accepted; public cc_common selected (2026-08-27)

Commit `324de9474` freezes the complete 14-line Bazel-9 compatibility symbols
payload over six accepted children and proves their owners, six private imports,
seven public exports, the ObjcInfo alias and exact inventories without
invocation. Focused, 289 library, protected integrations, locked checks and CLI
build pass within 0/188/188; root review returned `ACCEPT`.

The authenticated 18-line public `cc/common/cc_common.bzl` (`65e91cf0…`) now
has its complete generated child. Run only its re-export proof under 0/250/250,
proving exact owner/mapping and private-to-public pointer identity without
invocation. This closes the generic BCR Starlark façade-loading chain, not C++
semantics. Zabel is peer ownership guidance; Bazel 9.2 and authenticated
rules_cc bytes remain sole compatibility authority.

### M7 complete private cc_common accepted; compatibility symbols selected (2026-08-27)

Commit `873f07e2d` byte-verifies all 788 private cc_common lines over 22
accepted children and proves their owners, all 35 imports, eager private
values, 38 private functions, the exact 56-field façade and
32-public/78-all inventories without invocation. Focused, 288 library,
protected integrations, locked checks and CLI build pass within 0/1,236/1,236;
root review returned `ACCEPT`.

The authenticated Bazel-9 branch of rules_cc's compatibility repository now
has all six `symbols.bzl` children complete. Run only its 14-line normalized
payload proof under 0/500/500, proving six private imports, seven public exports
and the ObjcInfo alias without invocation. This is generated BCR Starlark
composition, not a C++ parser. Zabel is peer ownership guidance; Bazel 9.2 and
authenticated rules_cc generator bytes remain sole compatibility authority.

### M7 complete configure-features producer accepted; private cc_common selected (2026-08-27)

Commit `f52148534` byte-verifies all 232 configure-features lines over two
accepted children and proves both imported identities, all six ordered eager
lists, exact function visibility and nine-public/ten-all inventories without
invocation. Focused, 287 library, protected integrations, locked checks and CLI
build pass within 0/406/406; root review returned `ACCEPT`.

The complete 788-line private `cc_common.bzl` (`5e6ab737…`) now has all 22
direct defining modules accepted. Run only its complete freeze proof under
0/1,850/1,850, covering 35 imports, private eager values, 38 private functions,
the ordered 56-field façade and full inventories without invoking C++
semantics. This is the integration case for generic BCR Starlark evaluation,
not a C++ parser. Zabel is peer ownership guidance; Bazel 9.2 and authenticated
rules_cc bytes remain sole compatibility authority.

### M7 complete toolchain-config-info producer accepted; configure-features child selected (2026-08-27)

Commit `c4d19156d` byte-verifies all 143 toolchain-config-info lines over three
accepted children and proves five imports, exact initialized-provider identity,
public constructor, private initializer/raw constructor and exact
six-public/nine-all inventories without invocation. Focused, 286 library,
protected integrations, locked checks and CLI build pass within 0/337/337;
root review returned `ACCEPT`.

Private `cc_common.bzl` now reaches complete 232-line
`configure_features.bzl` (`d950aa9a…`) over accepted action names and semantics.
Run only its defining-module proof under 0/550/550, proving both imports, all
six ordered eager action-name lists and exact function/visibility inventories
without invocation. This remains generic BCR Starlark evaluation. Zabel is
peer ownership guidance; Bazel 9.2 and authenticated rules_cc bytes remain sole
compatibility authority.

### M7 complete legacy-features producer accepted; toolchain-config-info parent selected (2026-08-27)

Commit `1a3f543e2` byte-verifies all 1,387 `legacy_features.bzl` lines over
accepted action-name and toolchain-config-library children and proves ten
imported identities, three public plus one private lazy functions and exact
thirteen-public/fourteen-all inventories without invocation. Focused, 285
library, protected integrations, locked checks and CLI build pass within
0/1,495/1,495; root review returned `ACCEPT`.

Its 143-line parent `cc_toolchain_config_info.bzl` (`8c522773…`) now has a
complete three-child closure through Skylib paths, `cc_internal` and legacy
features. Run only its defining-module proof under 0/350/350, proving all
imports, exact `CcToolchainConfigInfo` identity, source functions and
six-public/nine-all inventories without invocation. This remains generic BCR
Starlark evaluation. Zabel is peer ownership guidance; Bazel 9.2 and
authenticated rules_cc bytes remain sole compatibility authority.

### M7 complete toolchain-info producer accepted; legacy-features child selected (2026-08-27)

Commit `7bb8d670f` byte-verifies all 255 dependency-free
`cc_toolchain_info.bzl` lines at the real rules_cc owner and proves normalized
`//cc/...` visibility, exact `CcToolchainInfo` identity, four private lazy
functions, the private raw constructor and exact one-public/six-all inventories
without invocation. Focused, 284 library, 25/25 invalidation, 32/32 BUILD
loading, locked checks and CLI build pass within 0/356/356; root review returned
`ACCEPT`.

Private `cc_common.bzl` source order passes the already accepted native bridge
and next reaches `cc_toolchain_config_info.bzl`, whose first unaccepted child is
complete 1,387-line `legacy_features.bzl` (`9a6cafe5…`) over accepted action
names and toolchain-config library modules. Run only its defining-module proof
under 0/1,600/1,600, prove all ten imported identities and exact four-function
visibility inventory, and invoke nothing. This remains generic BCR Starlark
evaluation; Zabel supplies peer ownership guidance only, while Bazel 9.2 and
authenticated rules_cc bytes remain sole compatibility authority.

### M7 `.bzl` visibility implementation accepted; complete toolchain-info proof selected (2026-08-27)

Commit `e14652d22` adds the generic Bazel 9.2 default-enabled `.bzl`
visibility family: evaluation-scoped declaration capture, compact canonical
policy retained in `FrozenBzlModule` equality, and one direct-edge checker at
all five Bzl/BUILD composition sites before importer evaluation. Focused 9/9,
all 283 loading-library, 25/25 invalidation and 32/32 BUILD-loading tests,
locked checks and CLI build pass within 412/540/952; independent correction
rereview returned `ACCEPT`.

Private `cc_common.bzl` source order returns to the authenticated dependency-free
255-line `rules_impl/cc_toolchain_info.bzl` (`f1958957…`). Run only its complete
defining-module proof under 0/450/450: preserve its normalized `//cc/...`
policy, exact `CcToolchainInfo` defining identity, four lazy functions, private
raw constructor and exact visibility inventories without invocation. This is a
generic BCR Starlark evaluation proof, not a C++ parser or native rule body.
Zabel remains peer ownership guidance; Bazel 9.2 and authenticated rules_cc
bytes remain sole compatibility authority.

### M7 `.bzl` visibility design accepted; implementation selected (2026-08-27)

Commit `33b7009a2` accepts one evaluation-scratch declaration slot, one compact
immutable policy owned by `FrozenBzlModule`, and one pure direct-edge checker
at all five Bzl/BUILD composition sites. Existing source/child/mapping/route
DICE dependencies remain the only producers; no key, lock, cache, registry,
source scan or digest domain is added. Independent correction rereview returned
`ACCEPT`.

Run only the bounded implementation under 500/850/1,350 caps. Prove the exact
default-enabled positional ABI, package-spec/mapping behavior, same-package
override, five-site pre-evaluation denial and observable A/B/A restoration.
Internal Rust representation and DICE mechanics remain Slug-native; the two
flag variants and Java diagnostic aggregation remain deferred. Zabel remains
concept-only peer guidance and Bazel 9.2 sole compatibility authority.

### M7 complete link producer accepted; `.bzl` visibility owner design selected (2026-08-27)

Commit `879d879f5` byte-verifies all 197 `link.bzl` lines over four actual
accepted children and proves five imports, the exact five-entry target-type
dictionary, one function and exact five-public/seven-all inventories without
invocation. Focused, 272 library, 24/31 integration, locked checks and CLI
build pass within 0/352/352; independent review returned `ACCEPT`.

Private `cc_common.bzl` source order next reaches dependency-free 255-line
`rules_impl/cc_toolchain_info.bzl` (`f1958957…`), whose first executable line
is `visibility(["//cc/..."])`. Run only the docs design packet for a generic
Bazel 9.2 default-enabled `.bzl` visibility fact retained by the loaded module
and enforced on every direct Bzl/BUILD load edge before importer evaluation.
Do not add a no-op global or C++-specific parser. Zabel is concept-only peer
guidance for evaluation-scoped capture and immutable policy ownership; Bazel
9.2 remains sole compatibility authority.

### M7 complete linkstamp producer accepted; link producer selected (2026-08-27)

Commit `6959f0370` byte-verifies all 44 linkstamp-producer lines over its actual
helper child and proves the imported identity, private provider, public
function and exact two-public/three-all inventories. Focused, 271 library,
24/31 integration, locked checks and CLI build pass within 0/119/119;
independent review returned `ACCEPT`.

The generic authenticated rules_cc traversal next reaches complete 197-line
`link/link.bzl` (`666e819d…`) over four accepted children. Run only its
defining-module proof under 0/450/450, prove all imports, the five-entry private
target-type dictionary, function and exact visibility inventory, and invoke
nothing. This remains generic Starlark evaluation of BCR-owned modules, not a
C++ parser; low-level Bazel host capabilities remain a separate boundary.
Zabel remains peer ownership/optimization guidance while Bazel 9.2 and
authenticated rules_cc bytes remain sole compatibility authority.

### M7 complete linking-context producer accepted; linkstamp producer selected (2026-08-27)

Commit `da0d9a5a5` byte-verifies all 137 linking-context-producer lines over
five actual complete children and proves all seven imported identities, one
function and exact seven-public/eight-all inventories. Focused, 270 library,
24/31 integration, locked checks and CLI build pass within 0/279/279;
independent review returned `ACCEPT`.

The generic authenticated rules_cc traversal next reaches complete 44-line
`link/create_linkstamp.bzl` (`8d5fc394…`) over accepted
`cc_helper_internal.bzl`. Run only its defining-module proof under 0/250/250,
prove the import, provider identity, function and exact visibility inventory,
and invoke nothing. This remains generic Starlark evaluation of BCR-owned
modules, not a C++ parser; low-level Bazel host capabilities remain a separate
boundary. Zabel remains peer ownership/optimization guidance while Bazel 9.2
and authenticated rules_cc bytes remain sole compatibility authority.

Freeze the authenticated complete 197-line rules_cc link producer over its four
accepted children. Prove its full imported/dictionary/function inventory
without invocation.

### M7 complete linking helper accepted; linking-context producer selected (2026-08-27)

Commit `233cdf9ef` byte-verifies all 675 C++ linking-helper lines over eight
actual complete children and proves fourteen imports, eight functions and exact
twelve-public/twenty-two-all inventories. Focused, 269 library, 24/31
integration, locked checks and CLI build pass within 0/862/862; independent
review returned `ACCEPT`.

Public `cc_common.bzl` source order first reaches complete 137-line
`create_linking_context_from_compilation_outputs.bzl` (`664a4615…`), whose five
children are accepted. Run only its defining-module proof under 0/400/400 and
invoke nothing. Zabel remains peer ownership guidance; Bazel 9.2 and
authenticated rules_cc bytes remain sole compatibility authority.

### M7 complete LTO indexing action accepted; linking helper selected (2026-08-27)

Commit `99d9289da` byte-verifies all 288 LTO-indexing-action lines over seven
actual complete children and proves nine imports, two functions and exact
nine-public/eleven-all inventories. Focused, 268 library, 24/31 integration,
locked checks and CLI build pass within 0/420/420; independent review returned
`ACCEPT`.

Complete 675-line `cc_linking_helper.bzl` (`c45dd243…`) now has all eight
children accepted. Run only its defining-module proof under 0/950/950 and
invoke nothing. Zabel remains peer ownership guidance; Bazel 9.2 and
authenticated rules_cc bytes remain sole compatibility authority.

### M7 complete C++ link action accepted; LTO indexing action selected (2026-08-27)

Commit `8daf80a2c` byte-verifies all 273 C++ link-action lines over eight actual
complete children and proves eleven imports, two functions and exact
ten-public/thirteen-all inventories. Focused, 267 library, 24/31 integration,
locked checks and CLI build pass within 0/454/454; independent review returned
`ACCEPT`.

The direct `cc_linking_helper.bzl` parent first lacks complete 288-line
`lto_indexing_action.bzl` (`03cb57e9…`), whose seven children are accepted. Run
only its defining-module proof under 0/625/625 and invoke nothing. Zabel remains
peer ownership guidance; Bazel 9.2 and authenticated rules_cc bytes remain sole
compatibility authority.

### M7 complete link finalizer accepted; C++ link action selected (2026-08-27)

Commit `aa797d082` byte-verifies all 469 finalizer lines over eight complete
children and proves fourteen imports, six functions and exact
thirteen-public/twenty-all inventories. Focused, 266 library, 24/31 integration,
locked checks and CLI build pass within 0/678/678; independent review returned
`ACCEPT`.

The first direct consumer is complete 273-line `cpp_link_action.bzl`
(`0cbe9d6b…`), whose eight children are accepted. Run only its defining-module
proof under 0/600/600 and invoke nothing. Zabel remains peer ownership guidance;
Bazel 9.2 and authenticated rules_cc bytes remain sole compatibility authority.

### M7 complete link-build variables accepted; finalizer selected (2026-08-27)

Commit `3b82f098c` byte-verifies all 392 link-build-variable lines over two
complete children and proves four imports, all 24 struct fields, all four
dictionary entries, five functions and exact eight-public/eleven-all
inventories. Focused, 265 library, 24/31 integration, locked checks and CLI
build pass within 0/530/530; independent review returned `ACCEPT`.

All eight children of complete 469-line `finalize_link_action.bzl`
(`adc6ea3b…`) are now accepted. Run only its defining-module proof under
0/800/800 and invoke nothing. Zabel remains peer ownership guidance; Bazel 9.2
and authenticated rules_cc bytes remain sole compatibility authority.

### M7 complete link values accepted; link-build variables selected (2026-08-27)

Commit `955e2204f` byte-verifies all 363 library-to-link-value lines over its
complete child and proves three imports, the six-field type struct, three
private provider identities, five functions and exact six-public/twelve-all
inventories. Focused, 264 library, 24/31 integration, locked checks and CLI
build pass within 0/498/498; independent review returned `ACCEPT`.

Finalizer source order next reaches complete 392-line
`link_build_variables.bzl` (`bdf03036…`) over accepted helper and internal
children. Run only its defining-module proof under 0/700/700 and invoke
nothing. Zabel remains peer ownership guidance; Bazel 9.2 and authenticated
rules_cc bytes remain sole compatibility authority.

### M7 complete solib dirs accepted; link values selected (2026-08-27)

Commit `6833c72de` byte-verifies all 479 solib-directory lines over three
complete children and proves five imports, seven functions and exact
six-public/twelve-all inventories. Focused, 263 library, 24/31 integration,
locked checks and CLI build pass within 0/599/599; independent review returned
`ACCEPT`.

Finalizer source order next reaches complete 363-line
`create_libraries_to_link_values.bzl` (`7d8df512…`) over accepted
`cc_helper_internal.bzl`. Run only its defining-module proof under 0/650/650
and invoke nothing. Zabel remains peer ownership guidance; Bazel 9.2 and
authenticated rules_cc bytes remain sole compatibility authority.

### M7 complete target types accepted; solib dirs selected (2026-08-27)

Commit `49e139212` byte-verifies all 131 target-type lines over two complete
children and proves exact named imports, strings, linking modes, all ten
six-field target mappings, the function and seven-public/seven-all inventories.
Struct iteration order remains Slug-native. Focused, 262 library, 24/31
integration, locked checks and CLI build pass within 0/283/283; independent
review returned `ACCEPT`.

Recursive source order resumes at complete 479-line `collect_solib_dirs.bzl`
(`f25b0f97…`) over three accepted children. Run only its defining-module proof
under 0/750/750 and invoke nothing. Zabel remains peer ownership guidance;
Bazel 9.2 and authenticated rules_cc bytes remain sole compatibility authority.

### M7 complete linker-input accepted; target types selected (2026-08-27)

Commit `2c1706e70` byte-verifies all 69 linker-input lines over complete
`cc_internal.bzl` and proves the private import/provider, public function and
exact one-public/three-all inventories. Focused, 261 library, 24/31 integration,
locked checks and CLI build pass within 0/142/142; independent review returned
`ACCEPT`.

Recursive source order through the next private `cc_common` link producer and
helpers reaches complete 131-line `target_types.bzl` (`12110c7d…`) over two
accepted children. Run only its defining-module proof under 0/500/500 and
invoke nothing. Zabel remains peer ownership guidance; Bazel 9.2 and
authenticated rules_cc bytes remain sole compatibility authority.

### M7 complete create-library accepted; linker-input selected (2026-08-27)

Commit `ace75573b` byte-verifies all 291 create-library lines over five complete
children and proves exact child mappings, six imports, the warning, provider,
four functions and exact seven-public/twelve-all inventories. Focused, 260
library, 24/31 integration, locked checks and CLI build pass within 0/463/463;
independent correction review returned `ACCEPT`.

The private `cc_common.bzl` source-order audit next reaches complete 69-line
`create_linker_input.bzl` (`e4e8a7fc…`) over accepted `cc_internal.bzl`. Run
only its defining-module proof under 0/300/300; invoke nothing and inspect no
callable defaults. Zabel remains peer ownership guidance; Bazel 9.2 and
authenticated rules_cc bytes remain sole compatibility authority.

### M7 complete LTO backends accepted; create-library selected (2026-08-27)

Commit `ccab93d4c` byte-verifies all 540 LTO-backend lines over four complete
children and proves child/native alias identities, four imports, the provider,
ten functions and exact seven-public/fifteen-all inventories. Focused, 259
library, 24/31 integration, locked checks and CLI build pass within 0/657/657;
independent review returned `ACCEPT`.

All five children of complete 291-line `create_library_to_link.bzl`
(`5f574233…`) are now accepted. Run only its defining-module proof under
0/600/600 and invoke nothing. Zabel remains peer ownership guidance; Bazel 9.2
and authenticated rules_cc bytes remain sole compatibility authority.

### M7 complete linkstamp accepted; LTO-backends child selected (2026-08-26)

Commit `78acfe43f` byte-verifies all 111 linkstamp lines over six actual complete
children and proves all imported identities, the public function and exact
six-public/seven-all inventories. Focused, 258 library, 24/31 integration,
locked checks and CLI build pass within 0/223/223; independent review returned
`ACCEPT`.

The private `cc_common.bzl` audit reaches `create_library_to_link.bzl`, whose
first missing child is complete 540-line `lto_backends.bzl` (`078bfb68…`) over
four accepted children. Run only its defining-module proof under 0/900/900 and
invoke nothing. Zabel remains peer ownership guidance; Bazel 9.2 and
authenticated rules_cc bytes remain sole compatibility authority.

### M7 complete compile producer accepted; linkstamp child selected (2026-08-26)

Commit `d32e2602d` byte-verifies all 2,295 compile lines over eleven complete
children and proves 25 imported identities, four ordered sets, the initialized
provider/raw constructor, 28 functions and exact 25-public/59-all inventories.
Focused, 257 library, 24/31 integration, locked checks and CLI build pass within
0/2694/2694; independent review returned `ACCEPT`.

The private `cc_common.bzl` source-order audit now reaches complete 111-line
`linkstamp_compile.bzl` (`6f5ceb39…`). Its six children are accepted and its
entire eager surface is six imported aliases plus one public lazy function. Run
only its complete proof under 0/300/300 and invoke nothing. Clean Zabel
`0795445f…` remains peer guidance for defining-module versus action-time
ownership; Bazel 9.2 and authenticated rules_cc bytes remain sole compatibility
authority.

### M7 complete action templates accepted; compile parent selected (2026-08-26)

Commit `bb11a1f73` byte-verifies all 266 action-template lines over six complete
children and proves ten imported pointers/visibility, one public and four
private lazy functions and exact public/all-visibility names. Focused, 255
library, 24/31 integration, locked checks and CLI build pass within 0/482/482;
independent review returned `ACCEPT`.

All eleven children of 2,295-line `compile.bzl` (`bec506ff…`) are now accepted.
Its eager source constructs one three-item private set, one private provider
with initializer and raw constructor, three public extension-category sets and
28 lazy functions using only accepted evaluator shapes. Run only the complete
producer proof under 0/3000/3000 caps and invoke nothing. Clean Zabel
`0795445f…` remains peer guidance for defining-module ownership of imported
values, provider initializer and global sets versus later invocation results;
Bazel 9.2 and authenticated rules_cc bytes remain sole compatibility authority.

### M7 complete compile variables accepted; action templates selected (2026-08-26)

Commit `97faa6e71` byte-verifies the 18-line native wrapper and 644-line
compile-variable producer. It proves the native predeclared alias using the same
globals instance, all three child identities, four imports, 25 `_VARS` fields,
provider/sentinel identity, the ordered 22-item source-type set, six public and
seven private functions and exact public/all-visibility names. Focused, 254
library, 24/31 integration, locked checks and CLI build pass within 0/875/875;
independent review returned `ACCEPT`.

The recursive audit now finds all six children of 266-line
`compile_action_templates.bzl` (`10a43c51…`) accepted: paths, helper, semantics,
cc_internal, compilation helper and compile variables. Its complete eager
surface contains only nine public and one private imported alias plus one public
and four private lazy functions. Run only its complete proof under 0/600/600
caps and invoke nothing. Clean Zabel `0795445f…` remains peer guidance for
imported defining-module ownership only; Bazel 9.2 and authenticated rules_cc
bytes remain sole compatibility authority.

### M7 complete compilation helper accepted; compile-variable producer selected (2026-08-26)

Commit `3060e4d4d` byte-verifies and freezes all 666 authenticated helper lines
over five complete children. It proves all imported pointers, exact public and
all-visibility name sets, private constant/provider, all 12 lazy functions and
the one-field captured-function struct without invoking anything. Independent
review caught and root corrected one reused child identity whose mapping was
not retained in the manifest; the final proof asserts all five child labels and
mappings. Focused, 253 library, 24/31 integration, locked checks and CLI build
pass within 0/871/871 additions; rereview returned `ACCEPT`.

The mandated recursive audit finds that 2,295-line `compile.bzl` first reaches
266-line `compile_action_templates.bzl`, which loads 644-line
`compile_build_variables.bzl` (`463ea66c…`). Private `cc_common.bzl` also loads
that producer directly, so proxy and toolchain consumers converge there. Its
only additional leaf is 18-line `native_cc_common.bzl` (`d8e5feda…`); all eager
forms use accepted imports, struct/provider/empty-depset shapes and the shared
default-enabled `set`. Run only the complete producer proof under 0/1050/1050
caps and invoke nothing. Clean Zabel `0795445f…` supplies peer guidance for
defining-module-owned globals/defaults versus later invocation values only;
Bazel 9.2 and authenticated rules_cc bytes remain sole compatibility authority.

### M7 compilation-helper proof REPLAN; complete universal environment selected (2026-08-26)

The exact 666-line `cc_compilation_helper.bzl` candidate stops during global
resolution at line 251: lazy `_module_map_struct_to_module_map_content`
references absent `set()`. No function was invoked and the complete +855 proof
candidate was removed byte-for-byte, returning to clean `1fb05138a`.

The widened audit found separate universes in loading, root/nonroot MODULE,
REPO and the live core evaluator. Vendored standard globals additionally expose
non-Bazel `chr`/`ord`, while REPO carries a stale always-disabled `set` shim
despite Bazel 9.2's default-enabled universal binding. Run only
`WP-4-5-7A-bazel-universal-builtins-environment` under 220/300/520 caps:
introduce a low-level process-stable exact 30-name owner, migrate every active
route, keep context overlays separate and prove the bounded exact set subset
including non-aliasing copy. Flag plumbing, exhaustive callable parity and
helper proof remain deferred. Clean `../zabel` `0795445f…` is a peer
implementation whose immutable-universe/predeclared separation and allocation
ideas inform the independently justified Rust design; Bazel 9.2 alone owns
compatibility and no Zig content is adopted.

### M7 universal environment accepted; compilation-helper retry selected (2026-08-26)

Commit `cb71a302d` adds one low-level process-stable exact 30-name Bazel 9.2
universe and migrates every active BUILD, `.bzl`, root/nonroot/include MODULE,
REPO and core evaluator. It enables vendored `SetType`, removes REPO's stale
disabled shim, excludes `chr`/`ord` and preserves distinct overlays. The bounded
set proof covers zero/one positional construction, invalid categories, type,
order, membership, add, non-aliasing copy and post-freeze immutability. Full
252/547/24/31 regressions, locked checks and CLI build pass; independent review
returned `ACCEPT`.

Commit `5c3b4492f` adds only the exact `app/slug_starlark_v2/**` checker
pathspec. Shell syntax and scope proofs pass; the app gate returns `OK` and only
the three longstanding thoughts-path baseline rows remain. Independent review
returned `ACCEPT`.

Retry only `WP-4-7A-rules-cc-compilation-helper-complete-loading-proof-r2` under
0/1050/1050 caps. Embed and hash all 666 authenticated lines, retain all
nine imports and exact eager/private/public inventory, and invoke nothing.
Zabel remains peer guidance for defining-module freeze ownership only; Bazel
9.2 and authenticated rules_cc bytes remain sole compatibility authority.

### M7 complete toolchain-config library accepted; compilation helper selected (2026-08-26)

Commit `acca5cb68` adds 703 proof lines and no production. It byte-verifies all
622 dependency-free library lines and proves the exact 27-name public surface,
all 13 provider-callable identities, 14 public functions and seven private
functions without invoking a callable or claiming schema/call behavior. All
251/24/31 tests, locked checks, CLI build and hygiene pass; independent review
returned `ACCEPT`.

The toolchain consumer next appears to offer 84-line
`armeabi_cc_toolchain_config.bzl`, but its `cc_common` and
`CcToolchainConfigInfo` loads re-enter incomplete generated-proxy/private
children. The compile branch's 666-line `cc_compilation_helper.bzl`
(`2c484cad…`) has five accepted children and is the first bounded frontier. Run
only `WP-4-7A-rules-cc-compilation-helper-complete-loading-proof` under
0/1050/1050 caps. Clean `../zabel` `0795445f…` guides imported/captured value
ownership and recursive freeze only; Bazel 9.2 and authenticated rules_cc remain
exact authority.

### M7 complete C++ semantics accepted; toolchain-config library selected (2026-08-26)

Commit `9cc0d4ace` adds 363 proof lines and no production. It byte-verifies all
234 semantics lines and proves both Booleans, the private canonical Windows
label, all 30 private functions, the exact 43-field name/value mapping, all 29
captured pointer identities, scalar/dictionary values and ordered lists. It
invokes no lazy function or `configuration_field`. Struct iteration order is
explicitly Slug-native rather than an exact claim. All 250/24/31 tests, locked
checks, CLI build and hygiene pass; independent correction rereview returned
`ACCEPT`.

The compile branch next reaches complete CcInfo/internal children before
666-line `cc_compilation_helper.bzl`; the competing toolchain branch reaches the
smaller dependency-free 622-line `cc_toolchain_config_lib.bzl` (`f8418490…`).
Run only `WP-4-7A-rules-cc-toolchain-config-lib-complete-loading-proof` under
0/850/850 caps. Clean `../zabel` `0795445f…` guides declaration-owned provider
and function freeze only; Bazel 9.2 and authenticated rules_cc remain exact
authority.

### M7 configuration-field binding accepted; complete semantics retry selected (2026-08-26)

Commit `fc131d7aa` adds 9 production and 59 proof lines. The `.bzl`-only binding
accepts Bazel's required positional-or-named strings, keeps BUILD absence and
routes four lawful forms to one Slug-native fail-closed diagnostic before any
result exists. All 249/24/31 tests, locked checks, CLI build and hygiene pass;
independent review returned `ACCEPT`. No descriptor/schema/configured behavior or
retained type was added.

The exact 234-line semantics source (`029254fd…`) now resolves every global while
all 30 bodies remain lazy. Run only
`WP-4-7A-rules-cc-semantics-complete-loading-proof-r2` under 0/550/550 caps.
Clean `../zabel` `0795445f…` guides captured-function and defining-module freeze
architecture only; Bazel 9.2 and authenticated rules_cc remain exact authority.

### M7 configuration-field named-only candidate REPLAN; dual ABI retry selected (2026-08-26)

Independent implementation review found pinned Bazel's `@Param(named = true)`
leaves `positional = true` by default. The rejected Rust candidate required both
arguments named and therefore silently narrowed valid two-positional and mixed
calls. Its +12/+58 diff was fully removed; all 249/24/31 tests had otherwise
passed and no descriptor/configured behavior was present.

Run only `WP-4-7A-bazel-configuration-field-loading-binding-r2` under the same
20/80/100 caps. Accept required string arguments positionally or by name,
including positional-then-named, and route every valid form to the identical
Slug-native fail-closed diagnostic. Clean `../zabel` `0795445f…` continues to
guide only binding/descriptor separation; Bazel 9.2 owns the exact ABI.

### M7 complete semantics proof REPLAN; configuration-field binding selected (2026-08-26)

Exact 234-line `cc/common/semantics.bzl` stops during name resolution at line 80:
lazy `_get_coverage_attrs` references absent predeclared `configuration_field`.
No function was invoked, no production changed and the candidate proof was
removed. Bazel 9.2 defines the `.bzl` global with required positional-or-named
string `fragment`/`name`; Slug lacks the late-bound value/configuration resolver.

Run only `WP-4-7A-bazel-configuration-field-loading-binding-r2` under 20/80/100
caps: add exact `.bzl` placement/type/ABI and fail every valid invocation closed
without retaining a descriptor. BUILD absence remains exact. Clean `../zabel`
`0795445f…` guides the separation between the predeclared binding and retained
late-bound semantics only; Bazel 9.2 remains exact authority. Retry complete
semantics only after this prerequisite is accepted.

### M7 complete action names accepted; C++ semantics selected (2026-08-26)

Commit `9e312f958` adds 328 proof lines and no production. It byte-verifies all
220 action-name lines and exhaustively proves 33 constants, the 33-field
`ACTION_NAMES`, seven ordered lists, and all seven pointer-identical final group
fields. All 248 loading-library, 24 invalidation and 31 BUILD-loading tests,
locked checks, CLI build and hygiene pass. Independent review returned `ACCEPT`.

Private `compile.bzl` now passes complete paths, action-names and helper children
before dependency-free 234-line `cc/common/semantics.bzl` (`029254fd…`). Its
eager surface uses only accepted constants, `Label`, lazy functions and one
capturing struct. The alternative toolchain branch next reaches a 622-line
library. Run only `WP-4-7A-rules-cc-semantics-complete-loading-proof` under
0/550/550 caps. Clean `../zabel` `0795445f…` guides captured-function and
defining-module recursive-freeze architecture only; Bazel 9.2 and authenticated
rules_cc remain exact authority.

### M7 complete compilation outputs accepted; action names selected (2026-08-26)

Commit `63d4bda76` adds exactly 450 proof lines and no production. It
byte-verifies all 226 compilation-output lines, rebuilds the complete
helper/internal/LTO closure, preserves all five imported pointers, and proves
sentinel/output providers, all lazy types and the exact empty output. All 247
loading-library, 24 invalidation and 31 BUILD-loading tests, locked checks, CLI
build and hygiene pass. Independent review returned `ACCEPT`, including the
captured helper-closure ownership boundary.

Private `cc_common` next enters 2,295-line `compile.bzl`; after accepted Skylib
paths its first incomplete child is dependency-free 220-line
`cc/action_names.bzl` (`e52d1647…`). The deferred 1,387-line legacy-features
branch loads the same child first, making this the smallest shared frontier. Run
only `WP-4-7A-rules-cc-action-names-complete-loading-proof` under 0/450/450
caps. Clean `../zabel` `0795445f…` guides declaration-owned aggregate and
defining-module freeze architecture only; Bazel 9.2 and authenticated rules_cc
remain exact authority.

### M7 complete LTO context accepted; compilation outputs selected (2026-08-26)

Commit `974b9e981` adds 207 proof lines and no production. It byte-verifies all
97 LTO-context lines, rebuilds both complete children, retains imported pointer
identity, and proves two provider IDs, all lazy types and the exact empty context.
All 246 loading-library, 24 invalidation and 31 BUILD-loading tests, locked
checks, CLI build and hygiene pass. Independent review accepts caps/boundaries.

All three children of 226-line `cc_compilation_outputs.bzl` (`294e3da1…`) are
now complete. Its eager surface is two provider declarations, a private
sentinel instance, and one source-owned empty-output construction using only
accepted shapes. Run only
`WP-4-7A-rules-cc-private-compilation-outputs-complete-loading-proof` under
0/450/450 caps. Clean `../zabel` `0795445f…` guides defining-module, captured
closure and recursive-freeze ownership only; Bazel 9.2 and authenticated
rules_cc remain exact authority.

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
