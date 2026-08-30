# Current Slug V2 Packet

Packet: `WP-4-5-7A-innate-repository-rule-owner-certificate-implementation`

Milestone: M7A category 6 generated-repository prerequisite.

Base: accepted architecture commit `30118d7fd` and effective Host-input
implementation commit `64878a1be`. The stopped canonical Host-capability draft
must be parked recoverably; the older dirty selected-context R2 candidate
remains unaccepted worktree state.

## Observable result

Implement one generic selected-owner boundary for Bazel 9.2 innate
`use_repo_rule`: keep the synthetic selected-extension identity used by module
resolution, but authenticate its actual repository-rule label/export and
instantiate retained calls through a distinct certificate. The accepted
ordinary `module_extension` owner remains unchanged.

The implementation stops at an authenticated RepoSpec/certificate handoff. It
does not execute `winsdk_configure`, expose a repository-context capability,
change MODULE evaluation, or resume the stopped Host-capability or selected-
context drafts.

## Learned facts and authority

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole semantic authority. `ModuleFileGlobals.useRepoRule` creates an innate
extension whose synthetic `.bzl` identity is `//:MODULE.bazel` and whose name
is `"<raw bzl label> <rule name>"`. `InnateRunnableExtension.load` requires a
singular owning module, splits that name at the last space, resolves the raw
label in that module's repository mapping, rejects a private rule name, and
loads the actual `.bzl`; `run` authenticates an exported repository rule and
instantiates the retained tag kwargs in call order. Pinned
`ModuleExtensionResolutionTest.innate`, `innate_repoRuleDependencies`,
`innate_noSuchRepoRule`, `innate_noSuchValue`,
`innate_noSuchValueIfPrivate`, and `innate_invalidAttributeValue` cover the
positive, mapping/dependency, export-kind/name, privacy and attribute borders.

Slug already retains Bazel's compound name in `module_eval.rs`; that file and
representation are correct and stay unchanged. The first live rejection in
`selected_extension_demand.rs` is not the whole bug: current owner inputs also
require a root use and root mapping, construct a load request for the synthetic
MODULE label, and `module_extension.rs` then authenticates only
`FrozenModuleExtensionDefinition`. Deleting either guard would load the wrong
file/type and fabricate ordinary-extension semantics.

`docs/developers/dice.md` governs producer ownership, dependency recording,
equality cutoff, observed path frontiers and the no-lock-across-compute rule.
Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is concept and optimization
guidance only; it supplies no behavior, identity or byte authority.

- **Exact:** compound identity and last-space split; exactly one owning
  module, including nonroot; owner-relative label resolution; private-name and
  exported-kind authentication; retained tag order/kwargs; separate actual
  rule and synthetic MODULE label domains; generated repository names,
  dependencies, imports, mappings, overrides and RepoSpecs; and unchanged
  ordinary-extension behavior.
- **Slug-native:** the Rust enum/certificate and DICE key layout, retained Arc
  projections, path-observation equality, and non-oracle diagnostic wording.
- **Unsupported/deferred:** repository-rule attribute/value families outside
  Slug's already admitted matrix; Windows Host realization and unadmitted
  repository-context capabilities; lockfile marker-byte parity; and any
  ruleset-specific shortcut. Unsupported values fail closed.

BCR Starlark remains the owner of all rule logic, including `cc_internal`.
`cc_common` remains only a generic Host/provider ABI client. This is neither a
`set`-builtin nor a C++ parser packet.

## Accepted architecture and implementation

### Bzlmod owner projection

Keep `HostSelectedExtensionOwner` as the synthetic demand/namespace identity.
Split selected-owner inputs into an explicit ordinary or innate projection;
do not infer the branch in loading from whitespace alone. The innate
projection owns:

- the untouched synthetic owner and unique generated-repository namespace;
- exactly one `HostGraphModuleKey` owner and its base/final repository mapping;
- the actual canonical repository-rule `.bzl` label and exported rule name,
  resolved by splitting at the last space in the singular owner's mapping;
- the distinct canonical synthetic `//:MODULE.bazel` label-conversion base in
  the owning repository, never the actual rule's package;
- that owner's retained `repo` tags in source order, including raw kwargs,
  call location and `dev_dependency` filtering already performed by MODULE
  evaluation; and
- validation imports plus any lawful namespace override projection.

Reject zero/multiple owners, isolation, an absent last-space component,
private names, route/mapping ambiguity, mismatched unique identities and
unresolvable labels before loading. Preserve full structural equality: the
synthetic identity, owner key, both mappings, actual label/name, tags,
imports/overrides and canonical source-route demand all participate.

`selected_extension_demand.rs` remains the natural Bzlmod producer and DICE
owner of these graph-derived facts. `module_eval.rs` stays unchanged. No
loading-side graph scan, string repair, side registry or winsdk branch is
allowed.

### Loading-owned innate certificate

Add a cohesive `module_extension_innate_repository.rs` next to, not inside,
the ordinary owner. Root definitions reuse the root `.bzl` owner. Every
nonroot definition computes the existing `HostCanonicalRepositoryLoadRoute`
and passes its complete canonical source input to the existing external `.bzl`
owner; this admits built-in, selected-registry, selected-nonregistry and
generated repositories without a new loader. A generated definition therefore
depends on its producer's existing repository-file effect plan, typed Need,
path observations and cycle detector before any load. Authenticate
`FrozenRepositoryRuleDefinition`, then reobserve the same definition
projection after the existing observed-load retry boundary and convert the
retained innate tags into ordered `RepositoryRuleCallRecord`s. Only the
already admitted None/bool/i32/string/label matrix is accepted.

Keep actual rule identity and call-site label conversion separate. The
`RepoSpec.rule_id` retains the authenticated definition's actual `.bzl` label.
For an innate supplied label string, add a narrow instantiation seam that uses
the retained synthetic owner `//:MODULE.bazel` label as the relative package/
repository base plus the owning module mapping. Repository-rule defaults keep
their definition-owned canonical labels. The existing ordinary instantiation
entry point continues to use each call's actual defining label and is
unchanged.

The result is the existing heap-independent pure invocation receipt shape, so
the existing repository instantiation and validation owners continue to own
canonical names, namespace mappings, RepoSpecs, import/override validation and
certification. Their public certificate dispatches by the explicit Bzlmod
owner kind and returns one unchanged consumer-facing iterator. Ordinary
extension evaluation remains in `module_extension.rs` and never sees an
innate projection.

Root/canonical source dependencies, generated effect plan, mapping/route
inputs, definition manifest, retained calls, separate conversion base and
merged `PathObservationEpoch` are DICE-retained semantic memory. Evaluator/load
values are phase scratch; no evaluator heap escapes. There is no service cache
or mutable registry. Cancellation publishes no partial certificate; equality
cutoff includes the complete projection and observations; workspace shutdown
releases retained state. No lock spans a DICE compute, repository effect or
`.bzl` load.

### Request, retry and downstream handoff

The immutable workspace/selected-graph request and existing path frontier are
the only request inputs. Root and canonical loads preserve their existing
typed Need/retry, generated-effect observation, cycle failure and final
reobservation behavior. A definition-load or instantiation failure publishes
no new repository effect or materialization; a generated definition may first
observe the already-owned effect that produces its source. Successful
certification hands the same RepoSpec/mapping surface to the existing
generated-repository consumer; additional repository Host observations remain
the stopped successor's job.

No fallback is added. The stopped Host-capability draft must be parked
recoverably before implementation because it overlaps `slug_loading_v2` and
currently contains a deliberately discriminating failing winsdk test. The
older selected-context candidate remains untouched. Restore the Host draft
only after the innate implementation is terminally accepted.

## Exact allowlist, blobs and caps

Only the following existing files may change, at their exact base blobs:

- `app/slug_bzlmod_v2/src/selected_repo_spec/selected_extension_demand.rs`
  `45dcb30d2d23b42e58b573a7ac8625aa6f86771b`;
- `app/slug_bzlmod_v2/src/selected_repo_spec.rs`
  `5bc1424cd42420174049bd318440541fde8ec6b0`;
- `app/slug_bzlmod_v2/src/lib.rs`
  `c565c5bfbd58f294826ecfe7bac56f5258ecafdb`;
- `app/slug_loading_v2/src/bzl_module.rs`
  `8309f65c379a12e66fcd53eccfc49cd9f53cb889`;
- new `app/slug_loading_v2/src/module_extension_innate_repository.rs`;
- `app/slug_loading_v2/src/module_extension_repository_instantiation.rs`
  `57b937c19a655c9eb52827f7ac305f04af198670`;
- `app/slug_loading_v2/src/module_extension_repository_validation.rs`
  `29b11c178b58550bcc34c41f16c9846fbbdcefdf`;
- `app/slug_loading_v2/src/lib.rs`
  `9e4d4ec028ca3ec7ea95ff88298cb85943f7945a`; and
- new `app/slug_loading_v2/tests/innate_repository_rule_owner.rs`.

`module_extension.rs`, `module_eval.rs`, Cargo files, core, command, server,
analysis, action, REAPI, repository-context/effect files and all ruleset/BCR
sources are forbidden. Permit at most 1,250 production Rust, 1,400 proof Rust
and 2,650 aggregate Rust additions. The new loading module is the required
complexity split: `selected_repo_spec.rs`, `bzl_module.rs`, instantiation and
validation already exceed or approach the 2,000-line review trigger and may
gain only thin projection/reuse/dispatch seams, not a second evaluator,
loader, namespace owner or general cleanup. This is graph/loading work, not a
demonstrated hot path; no benchmark is required.

## Discriminating proof and validation

The implementation proof must cover:

- explicit ordinary-versus-innate classification and unchanged ordinary
  owner output;
- root and nonroot singular owners, multiple-owner rejection, last-space
  parsing, owner-relative apparent-label resolution and mapping A/B/A;
- private, absent and wrong-kind exports plus definition reobservation drift;
- multiple retained calls in source order with name, None/bool/i32/string/
  label kwargs, unsupported-value rejection and generated namespace identity;
- a rule exported from `//defs:rule.bzl` whose supplied `:dep` resolves to the
  owner repository root `//:dep`, while its `RepoRuleId` remains
  `//defs:rule.bzl` and definition defaults remain definition-owned;
- imports/override validation; root, built-in, selected and generated source
  observations; generated repo-rule dependency routing, typed retry and cycle
  failure; and create/edit/delete/recreate invalidation; and
- the exact authenticated built-in winsdk owner reaching (but not executing)
  the generic repository-effect handoff without a winsdk/local-config branch.

Reuse pinned sources and existing deterministic BCR fixtures; add no Bazel
oracle fixture unless a demonstrated message/output gap remains. Run focused
Bzlmod projection and loading certificate tests, full serial
`slug_bzlmod_v2` and `slug_loading_v2`, one direct generated-repository
consumer, `cargo fmt --all`, `git diff --check`, exact scope/blob/cap/dirty-
isolation checks and `scripts/v2_archive_status.sh`.

## Stops and successors

`REPLAN` for whitespace-only dispatch; multiple owning modules; root-only
mapping; actual rule resolution outside the owner mapping; conflating the
actual definition label with the synthetic MODULE conversion base; synthetic
MODULE loading; ordinary-extension invocation for innate calls; generated
source without the existing canonical load-route/effect/cycle owners; copied
repository instantiation/validation; a new loader/key registry; loss of path
observation or call/mapping identity; evaluator values escaping; ruleset/
winsdk special case; change outside the allowlist/caps; or inability to isolate
both dirty candidates.

After terminal implementation acceptance, restore and reissue
`WP-4-5-7A-canonical-repository-rule-host-capability-implementation` against
the accepted innate certificate, remove its now-obsolete discriminating
failure expectation, complete its exact Host/context/effect proofs, and keep
Windows stopped. Then run the proof-only registered-toolchain closure before
returning selected-context R2 to terminal review.
