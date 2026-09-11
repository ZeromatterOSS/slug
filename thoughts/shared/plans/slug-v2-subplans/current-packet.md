# Current Slug V2 Work Packet

Packet: WP-5-7A-registration-error-identity-presentation-proof-r2

Status: narrow proof-correction successor independently ACCEPTED. R1 returned
REPLAN for missing required proofs and the necessary proof-cap increase; its
production architecture is sound, but no implementation acceptance is claimed.
Implement only the focused proof correction; preserve production unchanged.
Reviewer authorized exactly one investigation command before rollover:
`cargo test -p slug_core_v2 --lib --no-run --message-format=json`, serialized with
pinned Cargo and timeout60. This closes compile-frontier evidence only; no stale
binary, automatic runtime test, longer retry, probe or replay is authorized.

## Immediate predecessor and focused correction

R1 candidate: base78bfaee3c,16 Rust files,473 production/912 proof/1385 gross additions.
Complete export /tmp/slug-registration-r1.NfIhTj/candidate.patch SHA256
05c392952c62e652e0a13176206569ed63891f974e300e46b40da2480fb8fac1.
It remains in the worktree and must ship only after all gates and final review.
R2 preserves every production line/semantic choice below; only missing proofs
and their cfg(test) scaffolding may change within the same exact allowlist.
No new source fixture, semantic family, shared boundary or source audit is selected.

Required correction: exercise the actual merge command/module conversions and
both preparation functions with natural Loading-produced errors, legacy/observed
and command/module sources. Explicitly inspect the typed Registration payload,
first-error selection and Arc sharing; manual wrapper construction alone is not
production-handoff coverage. Retain the named existing Need/outer/order/dependency
gates. Complete the declared causal/incomplete branch matrix, notably Generated
Demand through Loading, direct Registration/Load/Route boundaries, RootBzl and
Pure/Innate Inputs/LoadRoute. Compile/run the new natural owner and Core/lifetime
proofs. Existing scaffolding and table-driven rows keep correction focused.
Budget adds488 proof lines of headroom over current912 (estimated220 for natural
handoffs and268 for missing branch/scaffolding rows):1400 proof/2050 gross,
production cap650 unchanged. This is a proof-estimate correction, not permission
to trim required assertions, broaden behavior or modify frozen production.

Recorded R1 gates in /tmp/slug-registration-errors.gM2tqt: Bzlmod3pass/21.29s;
Loading4pass/23.80s after replacing an ineffective comment-only discriminator
with real extension identity; Analysis1pass/50.96s. New Loading owner proof added
after its gate remains uncompiled. Core filtered libtest timed out60s before any
test start/result; no compiler error, completion fingerprint empty, executable
stale. Invocation timestamps support about38s before Core and22s after, not a
precise compiler/link phase. No stale binary ran and host process check is clear.
The one reviewed JSON compile-only diagnostic then succeeded: exit0,26.98s,
peak compiler-command RSS2821376KiB, fresh Core executable and build-finished true;
367 artifacts/51 build scripts/22 compiler messages. No tests ran. Warm prior work
precludes an isolated timing claim. Its success clears the compile-stop for the
named runtime gates only after R2 contract review; no further diagnostic retry.
Synthetic old/new/new/old rendering39680/112/112/39680bytes,76/3/0/29us;
no speedup or runtime peak-memory claim. Stage5 owns durable close evidence.

## Decision and evidence

Choose Loading-owned shared typed registration failures, carried unchanged
through Analysis and Core, plus a separate borrowed bounded diagnostic. Presentation
is Slug-native; registration semantics, structural identity/integrity, source
observations and publication are unchanged. No Bazel diagnostic-parity claim.
This prerequisite serves complete M7A selected-request/output-conflict acceptance;
it does not accept or partially restore the preserved R2 implementation.

The Stage5 source audit at09fc8f0bd proves successful selected_missing graph Debug
precedes the unseen generated error. This design also finds the execution-platform
only path at analysis dice.rs:3794-3799 serializes the same error. All four
conversions (merge's two, prepare_registrations, prepare_execution_platform_registrations)
must change together. Observation errors and DICE compute errors are separate
families and are not silently reclassified or repaired.

Analysis/resolution keys retain success-only equality. Core completed build
terminals compare Result errors through complete_eq. Preserve complete structural
registration errors, not their display, digest or pointer address, as identity.
No claim that old Debug text was an injective encoding, or that the historical14GB
allocation / exact captured terminal cause has been recovered.
Already-materialized compute/evaluation messages and other error families are
unchanged; a bounded borrowed renderer does not prove bounded total build memory.

## Frozen typed ownership

- In loading registration_expansion.rs, change only the private labels Err
  storage to Arc<ModuleRegistrationExpansionError>; allocate that Arc once in
  ExpansionScratch::finish. Existing labels() keeps its public borrowed-error
  signature through Arc::as_ref. Add doc-hidden labels_with_shared_error() returning
  Result<&Arc<[CanonicalLabel]>, &Arc<ModuleRegistrationExpansionError>>.
  Both views borrow the same retained error, without cloning its inner graph.
- Add public AnalysisErrorKind::Registration(RegistrationAnalysisError).
  The Analysis-owned wrapper has one private Arc<ModuleRegistrationExpansionError>,
  derives Clone/Eq/PartialEq/Allocative, and exposes a borrowed error accessor.
  Constructor consumes/shares that Arc; no public unchecked error fabrication.
  Its Display AND Debug use the bounded Loading diagnostic, preventing the
  surrounding derived AnalysisError/Kind Debug from visiting the retained graph.
- All four conversions use the shared-error view and cheap Arc clone. Existing
  Core BuildCommandErrorKind::Analysis and CqueryCommandError::Analysis retain
  the wrapper unchanged. Core's current Debug-to-string is bounded for this
  variant once Analysis Display is bounded; do not rewrite other Core variants.
  No change to global Loading/Bzlmod error Display/Debug or string-error consumers.
- Loading outcome equality is unchanged by structural Arc equality. Analysis
  errors may distinguish old text collisions; no two distinct typed causes may
  become equal because diagnostics truncate. Success-only cutoff/validity,
  complete_eq, outer-error precedence, Need precedence, family/row ordering,
  dependency recording and event publication stay exactly at current owners.
- Explicit lifetime change: downstream retained Analysis/Core errors now share
  the full Loading error DAG, instead of retaining its serialized String. Existing
  predecessors may therefore live until the last downstream error/DICE version
  releases them, not merely command completion. This is deliberate typed semantic
  retention, not a second graph, scratch borrow, command cache or global root.
  Arc/Allocative remain accounted; no deep inner clone, new interner/key/lock.
  Need/outer-error discarded temporaries drop immediately; retained terminals
  release on last reference/version eviction, transaction/runtime shutdown and
  cancellation/join as applicable. Do not promise release while DICE still owns it.

## Bounded presentation and causal coverage

Loading adds error.diagnostic() -> impl Display + '_; no retained text field.
A focused Loading helper owns a3072byte stack output buffer, including an explicit
"[diagnostic incomplete: ...]" suffix. Encode control/non-ASCII characters as
ASCII escapes while consuming bounded UTF-8 prefixes. Stop before an escape would
cross the budget; never split a code point/escape, scan a whole oversized string,
format a full error then truncate, or call arbitrary nested Debug/Display.
Return a completed borrowed-buffer rendering, not fmt::Error for internal budget
exhaustion. Actual downstream writer errors still propagate normally.
The current Core Debug adds at most one escaping layer: <=6144bytes plus its
fixed wrapper/newline, below unchanged8192stderr cap. Test the real Core wrapper.
The new Loading/Bzlmod renderer allocates no heap presentation buffer. Preserved
Core Debug still allocates its to_string() String (<=3072bytes of content), and
publication allocates the final escaped stderr String (<8192bytes of content).
Count/report these existing bounded buffers and their actual allocator capacities;
the no-heap claim does not apply to the full Analysis/Core publication path.

At most32 recursive Loading error nodes; fixed finite Bzlmod walks at most8
variants each, all streamed through the same bounded writer. Propagate writer
stop immediately. No collection iteration or successful-state formatting.
Use family/row, fixed variant tags and borrowed repository/label components,
then the actual failing child. No selected_missing graph for Generated;
Missing may name both lookup misses without opening their successful predecessors.
Do not use label/repository to_string unless its implementation is verified bounded;
prefer existing borrowed component accessors.

Required generated branch coverage:
- Demand / DemandCompute / Loading / LoadingCompute / Duplicate and Missing.
  Compute branches emit bounded borrowed messages; Duplicate emits both ordinals.
- Demand Missing/Ambiguous/Inconsistent names requested repo and at most two owner
  names, never owner Debug. Mappings follows RoutesCompute/RootFiles/
  RootFilesCompute/Invalid messages and Routes' compute/Invalid/RegistryMismatch/
  CanonicalCollision scalars. No selected graph or RepoSpec traversal.
- Loading follows Pure, InnatePure, Compute, Instantiation, InnateInstantiation,
  Validation and InnateValidation through their error fields, not pure,
  instantiated, inputs, requests, current_calls, current repositories or call data.
  Pure Compute/AfterInputs emits message; Inputs uses the Bzlmod adapter.
  Innate Compute/Label/Export/Call emits message, Drift its fixed reason,
  LoadRoute recurses with the same depth budget, Inputs uses the adapter.
  Instantiation emits Join/Namespace/Attribute and message. Validation emits
  Join message or MissingImport/MissingOverride/InjectCollision reason; no
  arbitrary offender Debug. Bzlmod owner-input Missing/Inconsistent/Unsupported
  names owner; Invalid adds its borrowed message; Mappings as above.
- Innate RootBzl/ExternalBzl follows Child with the shared depth budget; direct
  Parse/Freeze and external SourceCompute/Route/Evaluation emit messages;
  Absent/Encoding/Cycle name their fixed reason. Source/input/load-label and
  root Evaluation owners not audited here are explicit incomplete boundaries.

Every remaining registration/load-route kind still receives its own fixed tag
and safe scalar context, never a generic "registration failed". Parse/MissingTarget/
RowOverflow/RootMappingUnavailable are direct leaf diagnostics. Other unaudited
child owners (Selected/Configuration/RootMapping/subtree/package, load-route
Effect/Projection, route Builtin/Selected, Bzlmod Graph/RepoSpecs) must say
"[diagnostic incomplete: <owner>]" without claiming a leaf or silently dropping
the cause. These boundaries are diagnostic coverage, NOT unsupported semantics;
the entire typed error stays retained/equal-compared. No fallback to graph Debug.
If the actual future probe lands at such a boundary, its exact next owner receives
a reviewed bounded extension; this packet never guesses a missing source or
changes registrations/platforms to get past it.

Bzlmod adds only doc-hidden write_registration_diagnostic(&mut dyn fmt::Write)
methods on Demand and the two owner-input error wrappers, implemented in a
child helper; fixed-depth typed matching and borrowed strings only. This narrow
presentation handoff is not a general error trait, schema or diagnostics framework.
Loading private fields needed by its sibling helper may become pub(crate) only.

## Implementation allowlist and complexity

Only these Rust paths may change (prefixes app/slug_*_v2/src):
- loading: registration_expansion.rs, canonical_repository_load_route.rs,
  module_extension_repository_validation.rs, lib.rs; new registration_diagnostic.rs,
  registration_diagnostic_tests.rs; existing registration_expansion_tests.rs.
- bzlmod: selected_repo_spec/selected_extension_demand.rs; new child
  selected_repo_spec/selected_extension_demand/registration_diagnostic.rs
  and registration_diagnostic_tests.rs.
- analysis: dice.rs, lib.rs; new dice/registration_error.rs and
  dice/registration_error_tests.rs.
- core: runtime/dice.rs for a cfg(test) include only; new
  runtime/tests/registration_error_tests.rs.
No Cargo/profile/dependency, CLI/probe/driver or other semantic owner changes.
Caps:650 production/1400 proof/2050 gross additions relative to78bfaee3c;
R2 may not change the preserved473 production additions. Existing oversized owners
<=80 added production lines combined. New production helper<=350lines each.
Above2000lines: Analysis dice6055, Core dice12475, selected_repo_spec15978,
certificate2314, extension3162, instantiation3345. Keep traversal in focused
helpers; existing owners only storage/API/handoff/visibility or test includes.
No general split/reorganization of their semantic computes.

Audited public-kind consumers: Analysis Display and Core cquery mapping;
the latter's catch-all already preserves typed errors. Analysis lib reexports
the wrapper. root_analysis.rs:760 checks Message for deleted package (unchanged);
starlark_rule.rs checks other named variants. Core build-command tests and Loading
registration tests inspect preserved typed variants. Compile both query and CLI
direct consumers; no enum wildcard may silently stringify Registration.

## Proof and validation contract

Use focused in-memory synthetic errors and existing registration EpochBuilder/
tracker scaffolding, no copied trees, fake platform, acquired payload or replay.
Tests are Slug-native structured/diagnostic comparisons, not Bazel oracle fixtures.
Bazel 9.2 RegisteredToolchainsFunction/RegisteredExecutionPlatformsFunction semantics
are not changed; Stage6:18541-18570 retains their source/test basis. Rerunning their
oracle tests is unnecessary for this representation.
DICE basis: docs/developers/dice.md, retained dice/dice/src/api/key.rs equality/
validity contract, and existing complete_eq; use concept/tests only, no donor runtime.
Stage9 retained Arc/Allocative row applies; no new utility import or fallback.

Required tests, all named registration_diagnostic or registration_error:
1. Real Loading error views share one Arc; full predecessor-sensitive unequal
   errors can render identical bounded text. A/B/A restores exact typed equality,
   including Core completed terminal complete_eq; no pointer-only comparison.
2. Natural registration producer integration in both command/module and legacy/
   observed paths; four-source and execution-platform-only first-error/Need/outer
   precedence, observations, no later-row activation, event/publication unchanged.
3. Each generated/owner branch above, including explicit incomplete boundaries;
   failing message precedes no successful graph text. Pure/selected predecessor
   changes discriminate identity but not display. No formatter visits those fields.
4. UTF-8, controls, quotes/backslashes, huge messages/names,32-node recursion and
   output exhaustion; exact complete/incomplete marker and real Analysis/Core
   Debug and terminal stderr envelope remain below8192bytes.
5. Arc pointer sharing/no deep clones, Allocative derivation, Weak/reference-count
   release after temporary drops and final terminal/DICE owner release; retained
   references must stay alive while terminals own them. No cancellation leaks.
6. Bounded synthetic control/candidate A/B/B/A measurements, workload <=1MiB,
   no runtime graph. Preserve typed outcome/ordering and compare expected causal
   diagnostic fields (new presentation intentionally differs from old graph text).
   Acceptance: zero successful-predecessor render visits, one output buffer<=3072,
   only bounded-depth scalar traversal frames, encoded body<=3072, constant Arc
   handoff cost and observed final-reference release; no heap presentation buffer
   in the new Loading/Bzlmod renderer. Count the preserved bounded Core/publication
   Strings separately as above; do not report their allocation as zero.
   Record wall/RSS where available, no measured speedup/peak-memory reduction claim
   without stable evidence. Do not hide downstream retained-DAG lifetime cost.

Serialize pinned Cargo; every command timeout60, no automatic longer retry.
Run each crate's new filtered libtests separately (bzlmod/loading/analysis/core),
existing loading registration_expansion_tests, then cargo check -p slug_query_v2
-p slug_cli_v2. Reuse Analysis --test starlark_rule filters registration_family_
and command_registrations_precede_module_and_empty_overlay_restores_module_only_result,
and --test root_analysis filter
observed_toolchain_closure_depends_on_both_sources_and_families_once, separately.
These preserve existing message/Need/outer/dependency-order/restoration assertions;
do not replace them with only new synthetic helpers. First compile timeout/failure
stops runtime tests for investigation.
No full suite or actual probe is implied. Run rustfmt/diff, R2 hash/forward-apply,
old draft hash, archive checker (exact known3 failures), independent final review.
A future authentic probe requires explicit reviewed authorization at unchanged caps.

Docs writable: manifest, canonical Live Status, relevant Stage5/6 and routing
REPLAN only;<=120added lines outside manifest, PROGRESS<=500. The design inspected
the9 initial owners plus12 additional source/test files, excerpts<2MiB. Read the
declared owners/direct consumers; any further semantic-owner or public-boundary
expansion requires REPLAN. Missing ownership, cap overflow, graph formatting,
lossy equality or second material correction requires REPLAN, not a bypass.

## Preserved evidence

Probe /tmp/slug-sentinel-demand.PXDhxy/logs remains INCONCLUSIVE:
publication1/stderr overflow14.4498s/no abseil open; cleanup complete, no survivors.
No runtime repeated. Complete R2 /tmp/slug-conflict-r2.XZJWwv/candidate.patch SHA256
90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e stays unaccepted;
actual CLI conflicts, positive sharing and full gates remain open. Old probe draft
SHA2568eef40138afa23caa2601b89e6006de40f7e6d139f40124f4c2c430ae5fdf0a2 unchanged.
Never partially restore/ship R2 or inspect/print/copy ~/.bazelrc or secrets.
