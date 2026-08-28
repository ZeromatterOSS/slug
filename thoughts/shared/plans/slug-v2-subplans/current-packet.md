# Current Slug V2 Packet

Packet: `WP-4-5-7A-provider-independent-configured-toolchain-selection-r2`

Milestone: M7A provider-independent configured eligibility and selection,
feeding ordinary M8 Stage 10.3 analysis.

Base: `c8064b106`; commits through `76980a0b3` changed plans only.

Design: commit `a2dbdb553` froze the original nine-file packet. Downstream
validation forced the bounded R2 authority correction below; independent Sol
design review returns `ACCEPT` and implementation may resume within R2.

Result: implement category 4 of the frozen M7A toolchain sequence as one
provider-free configured-selection owner, then cut the configured rule path
over to its selected execution platform while retaining the old marker payload
only as a post-selection single-type bridge.

## R2 downstream-proof authority correction

The original implementation candidate passes its focused owner proofs, but
full `slug_core_v2` validation reports 30 failures versus the two documented
inherited core baselines. At least 28 new failures are the required consequence
of the exact zero-type contract: ordinary configured rules now materialize the
selected registered/host execution platform, retain its candidate-platform
topology and replay the newly reached MODULE/platform event producers.

Independent terminal Sol review returns `REPLAN`. Restoring the former core
behavior would violate the exact contract below, while the original allowlist
forbids the test fixture and expected-topology changes required by its own
direct-dependent gate. R2 preserves every semantic owner, compatibility class,
production cap and generic implementation boundary of the original packet. It
adds only:

- the `#[cfg(test)]` harness in core `runtime/dice.rs`, to install a hermetic
  minimal command-overridden `bazel_tools` containing a direct host platform
  for configured build/cquery fixtures without changing production's verbatim
  builtin or default host label;
- build and cquery proof expectations for the newly visible candidate-platform
  nodes/edges and child-owned MODULE/platform events; and
- focused proof that zero-type public action contexts select the test host
  through the same generic resolution owner, with warm replay and A/B/A
  behavior unchanged; and
- the exact exec-scope correction demonstrated by pinned Bazel 9.2
  `BuildConfigurationValueTest.starlarkFlagExecScopes`: default, universal and
  project-scoped Starlark flags survive exec projection while target-scoped
  flags are removed. Correct the existing configuration projection/regression
  only; add no owner or representation.

Do not edit core production behavior. The test-only override is fixture
provenance, not a semantic fallback: it exists because these unit tests provide
no registry/materializer capability for the real `@platforms` dependency. Its
deletion condition is a core harness that supplies the complete verbatim
builtin/BCR dependency graph hermetically; category 4 owns the fixture until
then. A regression must continue to prove that production configuration
defaults to `@@bazel_tools//tools:host_platform` and that the local override is
installed only by the test helper.

## Accepted prerequisite and live audit

Commit `c8064b106` supplies the reusable configured target-platform fact,
actual-target alias projection, platform-specific exec configuration,
constraint-aware configured conditions, complete graph-derived builtin mapping
and exact `@bazel_tools//tools:host_platform` composition. Independent
terminal review returned `ACCEPT`.

The live `c8064b106` audit found a bounded complete seam:

- loading already retains ordered `ToolchainTypeRequirement` values including
  `mandatory`, complete native toolchain declaration semantics and the
  command-before-MODULE registration expansions;
- configuration already owns structural target-platform selection and
  `to_exec_for_platform()`; its typed host-platform row needs only the matching
  canonical-label projection for Bazel's final execution-platform fallback;
- analysis already owns configured aliases, configured platform facts, target-
  setting condition batches, configured package loading and the singular marker
  consumer;
- no provider, evaluator heap, raw MODULE/source carrier, new parser, new
  configuration store or ruleset-specific state is required for eligibility
  and selection.

The loading invocation gate still rejects an already-retained optional
requirement, and the configured analysis path still narrows requirements to
zero-or-one and performs selection inline with marker implementation analysis.
Those are the only cutover defects in this packet.

## Behavior authority

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole behavior authority:

- `ToolchainResolutionFunction#loadToolchainTypes` resolves requested aliases
  to actual type identity while preserving requirement policy;
- `RegisteredToolchainsFunction` filters declarations by configured
  `target_settings` before selection;
- `SingleToolchainResolutionFunction#resolveConstraints` preserves
  declaration order, target constraints, execution constraints and
  target-to-exec constraint policy;
- `ToolchainResolutionFunction#determineToolchainImplementations`,
  `findExecutionPlatformForToolchains` and `PlatformKeys#isPlatformSuitable`
  distinguish mandatory absence from no common platform, allow optional
  absence, require every mandatory actual type, maximize covered distinct
  requested actual types and preserve candidate order on ties;
- `PlatformKeys#findExecutionPlatformKeys` appends the configured host platform
  after registered candidates and deduplicates it through the ordered set;
- `RegisteredExecutionPlatformsFunction` resolves configured aliases and its
  immutable-map construction rejects distinct registrations converging on one
  actual platform.

Reuse the authenticated source/tests already frozen in
`06-analysis-toolchains-and-actions.md`. Add oracle evidence only for a
demonstrated uncovered discriminator.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer architecture and
optimization guidance only. It guides producer-owned platform facts,
requested/actual identity separation and immutable compact selection rows. No
Zig layout, scheduler, store, identity, diagnostic or behavior is copied.

## Ownership and representation

Add one legacy/observed DICE family:
`ConfiguredToolchainResolutionKey` and its doc-hidden observation sibling.
The semantic key is exactly workspace, structural target configuration and the
ordered immutable requirement slice. It consumes the existing four prepared
registration families, configured target-platform key, configured
node/actual-target keys, configured platform keys, configured package owners
and configured-condition key.

Publish one immutable `ConfiguredToolchainResolution`:

- the configured target-platform fact;
- the selected actual configured execution-platform fact and its platform-
  specific exec configuration;
- an ordered `Arc<[ConfiguredToolchainResolutionRow]>` with one row per
  requested requirement, retaining requested configured type, actual
  configured type, mandatory policy and optional selected native declaration
  identity.

Rows contain no implementation/provider/evaluator value. Distinct requested
aliases converging on one actual type retain distinct output rows. Their actual
selection group is mandatory when any requested row is mandatory; the group is
evaluated once, and its selected declaration or optional absence is projected
back to every requested row.

Use existing Buck2-derived `SmallMap`/`SmallSet` only as compute scratch,
preserving registration, requirement and candidate order explicitly. Retained
state uses `Arc` slices, existing configured/canonical identities, `Dupe`
and `Allocative`; add no hash cache, interner, standard retained hash
collection, global store or memory-ledger category.

## Computation and precedence

1. Always prepare command-before-MODULE execution-platform registrations for a
   structurally configured rule, resolve their configured aliases and append
   the typed configured host-platform label as the final candidate. Add the
   small `host_platform_label()` projection beside the accepted target-platform
   projection; do not parse or stringify the native row. With no requested
   type, select the first actual registered execution platform, or the host
   fallback when none is registered. Bypass toolchain-registration,
   declaration, package and condition work only when there is no requested type
   and no local marker-topology declaration. Otherwise prepare the remaining
   registration families through the accepted owner.
2. Compute the configured target platform and resolve every requested type,
   registered execution platform, host fallback and declaration type through
   existing actual-target/configured-platform owners. Candidate convergence
   among registered platforms is a semantic error before selection; convergence
   between an earlier registered actual platform and the appended host fallback
   keeps the earlier candidate once, matching Bazel's ordered-set fallback.
3. Load complete native declarations and referenced constraints/settings via
   existing configured package owners. Resolve all declaration
   `target_settings` through the sole configured-condition batch in the target
   configuration.
4. Validate actual toolchain types and constraint values/settings. Filter each
   declaration by type group, settings, target compatibility and
   `use_target_platform_constraints`; then retain its first compatible
   declaration per candidate execution platform.
5. If one mandatory actual type has no declaration on any candidate, return the
   mandatory-absent error. Otherwise choose among candidates containing every
   mandatory group by greatest number of distinct requested actual groups;
   preserve registered candidate order on ties. A nonempty eligible set with no
   common candidate is a distinct error. Optional missing groups publish
   `None`.
6. Derive the selected execution platform through its existing
   `ConfiguredPlatformKey`; its actual key already carries
   `to_exec_for_platform()` identity. Publish rows in requested order.

Compute independent prerequisite batches completely before reduction. Preserve
outer frontier error before merged Need before semantic failure. Need unions
must retain all compatible source/repository demands. Cancellation publishes no
resolution value; same-graph repair must recover. Hold no lock across DICE.

## Configured-analysis cutover

Configured rule analysis passes the complete retained ordered requirement slice
to the new family; delete the zero-or-one restriction and the inline selector.
All admitted rules use the resolution's selected execution platform for their
default action context.

The existing marker payload remains a temporary post-selection bridge only when
there is exactly one mandatory requested row, no optional row and one selected
declaration. After selection, reload that declaration's implementation through
existing owners and retain the current marker validation/analysis behavior.
The bridge cannot affect eligibility, grouping, platform choice or the
resolution value. Multi-type and optional cases expose no `ctx.toolchains`
payload and perform no selected implementation analysis; attempting provider
access remains deferred/fail-closed until categories 5 and 6.

Keep existing candidate-platform topology edges. Publish the old singular
toolchain selection/implementation edges only for the marker bridge; do not
invent a partial multi-type provider/topology projection.

## Compatibility

- **Exact:** admitted Bazel 9.2 empty-type first-registered/host-fallback
  platform selection, requested/actual alias grouping, mandatory-OR, one row per
  request, optional absence, configured target settings, target and execution
  constraints, `use_target_platform_constraints`, registration and declaration
  order, candidate coverage/tie order, platform alias convergence, mandatory-
  absent versus no-common-platform behavior and selected exec-configuration
  semantics.
- **Slug-native:** Rust representation/layout, structural configuration and
  DICE identity bytes, compact scratch/retained containers, memory accounting
  and unproved diagnostic wording.
- **Unsupported/deferred:** arbitrary `ToolchainInfo` or user-provider values,
  `ctx.toolchains` for the new multi/optional category, selected implementation
  analysis under the exec configuration, provider/action behavior beyond the
  old marker bridge, broader exec groups and exact Bazel configuration/output
  bytes. Categories 5 and 6 own those surfaces.

BCR-delivered Starlark owns every rule definition and control path, including
`cc_internal`. `cc_common` remains only a demanding client of the future
generic host/provider ABI. No C++ or rules_rust rule engine, builtin-specific
selector or source-level ruleset shortcut is permitted.

## Exact allowlist and caps

All baselines are the live `c8064b106` blobs:

| Path | Baseline blob / lines | Maximum physical growth |
|---|---:|---:|
| `app/slug_configuration_v2/src/native/configuration.rs` | `0adf79acda69b71204c826c7524411affced6dc3` / 1,629 | +25 |
| `app/slug_configuration_v2/src/native/tests.rs` | `63141d47f9bc75f65cb533b31ca2417d2e06b4c9` / 3,550 | +35 |
| `app/slug_loading_v2/src/package.rs` | `0f54f092669e37551c70290ef0a35b3200dee047` / 6,901 | +5 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `59477bc6a2204fef5fa091e5fd769390d165a535` / 35,108 | +20 |
| `app/slug_analysis_v2/src/dice.rs` | `2fa5583b541f167f095901f37be292e25287cc20` / 4,065 | +1,250 |
| `app/slug_analysis_v2/src/result.rs` | `eb0c73b830714475777eed55d8b05366c1a295c2` / 723 | +350 |
| `app/slug_analysis_v2/src/lib.rs` | `9a8e4bacb9c99d3a430602fa3282cf7a8249d02e` / 83 | +20 |
| `app/slug_analysis_v2/tests/starlark_rule.rs` | `e53ead56097cd882c1ccc48f6a6a7afaef7275f9` / 6,602 | +900 |
| `app/slug_analysis_v2/tests/root_analysis.rs` | `afab443f8ae51a78a38926cfdcfdea3796f4f797` / 1,204 | +300 |
| `app/slug_core_v2/src/runtime/dice.rs` | `6ad6f61973f51dd3cbf94a9c0c01e257c231b1a0` / 12,010 | +300, test-only |
| `app/slug_core_v2/src/runtime/tests/build_command_tests.rs` | `8f6eb8444539fd2a6b3efb5ff8233e5444e40ba8` / 4,056 | +250 |
| `app/slug_core_v2/src/runtime/tests/cquery_command_tests.rs` | `749b1703c5ce6b8c8af909104f7b88495ebf5cb3` / 1,082 | +300 |

Production additions remain capped at 1,650 physical Rust lines, proof
additions are capped at 2,105, and total additions at 3,755. No new non-plan
file is allowed. The
existing large `dice.rs` remains cohesive because the new key reuses its
private configured-package, condition, platform, alias and rule-driver
helpers; exporting those internals into a new module would widen the ownership
surface and cause more churn.

No file beyond this table and writable plans/ledgers may change. Core
`runtime/dice.rs` authority is confined to its `#[cfg(test)]` module; no core
production owner or public API may change. No Cargo/BUILD, CLI/server,
registration-expansion, Bzlmod/repository, fixture/asset, lockfile, oracle or
generated-file change is authorized.

## Proof matrix

Use the existing local generic fixtures and add only discriminating gaps:

1. two mandatory types, optional present/absent and mandatory-absent versus
   no-common-platform;
2. requested aliases converging on one actual type, mandatory-OR, first-request
   order and one row per requested alias;
3. declaration type aliases and registered execution-platform alias chains,
   including convergence rejection;
4. target-setting selected/unselected branches, target constraints, execution
   constraints and `use_target_platform_constraints`;
5. declaration order, candidate order and greatest-distinct-type coverage with
   stable ties;
6. target-platform versus selected actual execution-platform identity and
   platform-specific exec configuration;
7. legacy/observed dependency rows, outer/Need/semantic precedence, cancellation,
   A/B/A restoration and unchanged-parent Arc reuse;
8. zero-requirement first-registered then host-fallback selection, registered/
   host actual convergence, no toolchain-declaration/condition activation,
   platform-only local-declaration context and exact single-mandatory marker
   bridge isolation; and
9. optional invocation loading acceptance while malformed/duplicate
   requirements remain fail closed.

## Validation and stops

Run focused loading/analysis proofs; full `slug_loading_v2` and
`slug_analysis_v2` suites serially; configuration/query/Bzlmod and core direct
dependents; rebuild `slug_cli_v2`; rustfmt; `git diff --check`; source/blob/
line/cap and packet/canonical audits; and `scripts/v2_archive_status.sh`.
Full core may reproduce only its two independently documented inherited
failures—workspace-directory Lstat drift after lockfile publication and the
stale external-query event replay expectation—with every selection-related
regression closed.
Report those two core baselines and the three tracked-thoughts archive baseline
distinctly. Completion requires independent Sol terminal review returning
`ACCEPT`.

STOP and `REPLAN` for provider/evaluator data in the resolution value;
implementation analysis inside selection; raw source/MODULE/display-label
inputs; a second condition/platform/registration/parser/configuration owner; a
DICE cycle or lock across compute; narrowed multi-type/optional/alias behavior;
platform alias first-wins; a selection error outranking outer/Need; lost
observations; ruleset/`cc_common` specialization; Zabel authority; an
unallowlisted file; cap breach; or any material implementation correction not
already bounded above.

R2 additionally stops on any production core edit, a test helper that changes
ordinary command policy outside configured build/cquery fixtures, replacement
of the verbatim builtin in production, suppression of candidate-platform
topology/events merely to restore old expectations, or a third newly failing
core baseline.
