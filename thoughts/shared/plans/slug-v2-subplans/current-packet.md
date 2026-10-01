# Current Slug V2 Work Packet

Packet: WP-7-174-m7a-identical-artifact-symlink-registration-r1
Status: accepted; independent corrected DESIGN/FINAL accepted; M7A remains partial

## Outcome and compatibility

Baseline `0361ef2a4` (WP873). Let repeated equivalent ordinary artifact-symlink
registrations within one CtxActions return the first registered index/action,
using the existing output-owner candidate index. Advance unchanged authentic
Rust-toolchain query past its sysroot libLLVM duplicate output at toolchain.bzl:156
if and only if the actions prove equivalent. The first authentic input is not
retained after one-shot cleanup; overlap is a hypothesis, not accepted evidence.
Exact bounded same-context/same-structural-output and ownerful-identical-artifact
registration eligibility; Slug-native structural identity remains unchanged.
First-action/index registry canonicalization and action-vector cardinality are
Slug-native: Bazel assigns the first generating key but retains original action
rows (Actions.java:233–282; RuleConfiguredTargetBuilder.java:278–294). This does
not admit exact raw duplicate-row/progress inspection or ordinary-symlink aquery.
Cross-target ordinary-symlink sharing, ownerless artifact aliases, absolute-path
symlink deduplication, general action sharing and Windows remain deferred.

Pinned clean Bazel 9.2 `8220c6198837d5c13d53fea211cf3282aa12408a`:
RuleConfiguredTargetBuilder.java:277–280 uses Actions' shared-action conflict gate;
Actions.java:85–115 compares mnemonic/key and ordered mandatory inputs/outputs,
:191–220 permits equivalent registrations in one target. AbstractAction.java:
352–353 is shareable. SymlinkAction.java:429–448 keys optional inputPath and changes
mnemonic for executable symlinks; progress text is not a key input. We admit only
same typed Artifact input/output identity plus require_executable and
use_exec_root_for_source flags; differing presentation progress may reuse the
first action. No Bazel ActionKey-byte or ownerless/full-sharing claim. Existing
Stage 6 configured-action closure eligibility remains unchanged. Upstream Java
instance/reference assertions are not used; genuine Starlark regression exercises
the public sink and retained typed actions instead.

## Owners, invariants and scope

BuildAPI actions/ctx_actions.rs::register_symlink owns the decision. SymlinkSpec
already owns output, typed target and optional progress; ActionRegistry already
owns ordered actions and an output-path candidate index. Resolve that candidate,
require its typed symlink spec, require both targets to be Artifact, compare full
output and target equality, then return its original index. Progress differs only
in presentation and first-action retention; do not mutate the accepted action.
The candidate ActionSpec execution properties must be empty and exec_group None,
matching register_symlink's default envelope. Generic batch-supplied nondefault
properties/groups must conflict; they cannot be erased by comparing payload only.
Non-equivalent candidates continue through existing registry validation/conflict
paths, including projected-path collisions, output kinds/metadata, other action
families and malformed output paths. Generic register/register_batch and atomic
replace_actions publication remain unchanged. Preserve first-seen ordering and
all unrelated outputs; no filesystem/path-only inference or registration bypass.

Loading active-token and native Sink current-output ownership/regular File/input
checks run before registration and remain unchanged. Sink locks remain synchronous;
no new DICE key, lock/await, retained fact, cache or memory owner. Candidate/spec
comparison is call scratch borrowing existing phase registry; no new representation,
collection or donor utility. Existing DICE source/configuration/provider ownership
publishes the deduplicated ordinary action vector, invalidates and restores it.
No replay postprocessing, command repair or fresh graph bypass. Buck2 utility skill
requires no donor change for unchanged storage/representation. ctx_actions.rs is
small/cohesive; existing large starlark_rule test fixture is reused without new
monolithic production responsibilities. Independent DESIGN/FINAL required.

Allowlist: app/slug_build_api_v2/src/actions/ctx_actions.rs; existing
app/slug_build_api_v2/tests/actions.rs; existing Analysis tests/starlark_rule.rs
sibling/symlink fixture and regression only; canonical/manifest and Stage 6 current
action-registration paragraph. No Loading, generic registry/spec/identity, Core
closure, executor, CLI, dependencies or copied fixture assets. Estimate production
<20/proof <150 net Rust lines. WP873 fixture hygiene checkpoint passed; no file growth.

## Proof, validation and stops

BuildAPI proof must retain original action/index/progress/order for repeats and
progress-only differences; distinguish source/derived ownerful inputs, execution
flags, structural output roots/kinds/metadata and other action kinds. Each rejected
registration preserves full registry state; generic duplicate/batch atomicity stay
protected, including batch candidates with execution properties/group differences.
Absolute-path deduplication remains refused. Genuine sibling fixture
repeats source and exec-derived symlinks with different progress, keeps two actions,
and proves root/nested/external, Target/Exec, both Analysis routes and source/declared
output/configuration A/B/A. Add a real public different-input/executable conflict
mutation and restoration; preserve active/lifetime downstream evidence.

Root serial pinned Cargo JSON BuildAPI --test actions and Analysis --test
starlark_rule no-run60s; exact ordinary nonignored preflight/runtime20s. Analysis
lib downstream/lifecycle compile/focused proof, Core check and CLI rebuild before
unchanged authentic supervised180s replay. Its ~79s setup is required to prove real
toolchain overlap; authored equivalence checks cannot establish bootstrap progress.
Actual process pre/post cleanup, three original hashes, scoped fmt/diff/archive/plan
and fixture hygiene gates. Caps are operation bounds, not user limits. REPLAN if
actual overlap needs different Artifact identity, a new action-key/ownerless rule,
cross-target sharing or broader action family. A later query refusal is successor
evidence, not full query/execution/bootstrap closure. Windows stays minimal.

## Predecessor

WP873 `0361ef2a4`: independent DESIGN/FINAL and fixture hygiene, 28 focused tests
in1.433s, Core/CLI pass. Target.files reuses frozen DefaultInfo.files; flag and
missing-default boundaries deferred/refused. Unchanged authentic79.222s exit2
clears files projection and reaches duplicate sysroot symlink registration.
Original hashes and actual cleanup pass. Receipts target/wp873; full M7A remains partial.

## Acceptance

Corrected DESIGN and FINAL independently ACCEPT. The execution-envelope correction
prevents generic batch metadata from disappearing. Production +15 Rust lines;
proof +234 exceeds the +150 estimate because source/derived owner/config/root/kind,
execution flags, output kinds/constant metadata, progress/order and batch envelopes
each discriminate separate retained invariants. No new fixtures, copied assets or
representation/helper growth; the existing genuine Analysis matrix is preserved.

Pinned serial preparation BuildAPI3.78s, Analysis lib1.34s, starlark_rule11.15s;
exact ordinary nonignored preflights pass. BuildAPI6/6 in0.032s, Analysis4/4 in0.033s,
integration2/2 in0.414s: 12 focused tests in0.479s. Core check3.31s and CLI rebuild
5.62s pass. Unchanged authentic78.763s exit2 clears duplicate sysroot symlink and
reaches ToolchainInfo.make_libstd_and_allocator_ccinfo function publication refusal.
This verifies actual overlap under the strict ownerful/default-envelope predicate;
different-input/flag/envelope conflicts remain protected. Three original hashes,
supervised and actual process cleanup, scoped fmt/diff/archive/status and fixture
hygiene pass. Receipts target/wp874. Packet window03:23–03:36UTC (~13m), no speedup
claim. Broader sharing/raw duplicate-row inspection, Windows and full bootstrap stay
deferred; a retained callable requires its own source/lifetime/identity review.
