# Current Slug V2 Work Packet

Packet: WP-7-37-m7a-generated-chain-execution-r1
Status: accepted

## Outcome and compatibility

Execute a selected typed Spawn and its reachable FileWrite/Spawn prerequisites
inside one Core native request. Bind generated File/Directory inputs only to
verified producer results from that attempt, and accept the selected result only
after final validation of the entire reachable source frontier. This advances the
bootstrap LALRPOP/proto build-script out_dir -> Rustc dependency requirement.
Baseline 99c9f146e: WP736 accepted owner-preserving prerequisite planning; WP734
owns native completion/precheck/finalization; WP735 owns verified output manifests.

Pinned Bazel 9.2 8220c6198837d5c13d53fea211cf3282aa12408a:
ArtifactFunction generating-action lookup and ActionExecutionFunction input
prerequisites, plus MerkleTreeComputer.java:1033-1047 (all input file nodes
executable), 847-880 (tree inputs rebuilt from file children), 299-305 (empty
output-directory roots). Exact: retained producer ownership, verified CAS SHA-256,
source/generated/forced-param input file executable policy and generated tree projection from file children.
Keep declared tree roots even when empty, omit nested empty producer directories
unless needed as ancestors, and include declared output-directory roots in the
new chain's input tree. Producer manifests retain their truthful modes/empty dirs.
Slug-native: configured identities, sequential order, errors, existing Command/
Action wire profile. No full Bazel input-root/Action digest parity claim. Deferred:
runfiles/symlinks/discovered inputs, missing-CAS producer recovery, CLI activation,
local publication, full build scheduling, bootstrap and exact ActionKey.

## Ownership and lifecycle

Add a selected-chain DICE preparation key over BuildCommandRootKey + configured
owner + action ordinal. Compute the existing observed build root, derive WP736's
borrowed plan, observe every deduplicated reachable Source input using existing
SourceArtifactInputObservationKey, and union those observations with the complete
build frontier into one associated SourceCertificate. Need/errors, equality,
validity, event reconciliation and request initialization follow SourceStagingKey.
No remote results, byte buffers, channels or open files enter DICE. Existing
source-only preparation retains its Derived rejection. Read docs/developers/dice.md;
reuse native dependency recording/finalization and existing lifecycle evidence.

PreparedActionChainInputs privately retains evaluation, selection, observed
sources and certificate; public plan(), sources(), observations(), open_source()
are borrowed metadata/source transfer projections. Rebuild the plan at operation
start rather than retaining a self-referential graph. Factor source opening and
build-frontier checking for reuse where useful. Use existing compact SmallSet/
SmallMap and Arc retained storage (Stage 9 retained utility disposition); no new
interner, dependency, global cache or lock.

Core ActionChainTransport has Session:Send, Staged:Send, Output:Send+Sync and Error;
start(Arc<PreparedActionChainInputs>) -> Session; stage(&mut Session,index) -> Staged;
execute(&mut Session,index,Staged) -> (); finish(Session) -> Output. All methods are async and
return Send futures. This is a trusted extension like SourceActionTransport. Core
starts a fresh session each native attempt, walks the prerequisite-first plan,
stages then prechecks the FULL certificate before each Execute, and invokes the
existing final native validation/retry. Stage/precheck failures abort the chain;
all provisional session/results drop on failure, cancellation or retry. No lock
is held over transport effects. AcceptedCommand<ActionChainResult<Output>> exposes
only final accepted inputs/output; no local publication authority is added.

REAPI ActionChainReapiTransport owns immutable config; private session/staged fields
bind endpoint, instance, step order and verified results. Preflight every reachable
action's lowering/policy, exact declared input/output/param namespace collision and
forced-param expansion before connecting or executing. Reuse shared Spawn command
policy and FileWrite lowering; expand each Spawn once. Resolve each generated
input through the plan producer index and exact declared output path AND kind.
Only verified AC/Execute results produced inside this session enter its table;
never accept caller-supplied RemoteExecutionResult as provenance. Keep intermediate
file content in CAS by verifying through a discard sink; return bounded output
metadata (existing tree budgets) and execution evidence, with empty output_blobs.

Compose input files with explicit executable mode plus explicit directory roots;
rebuild canonical Directory blobs from verified tree file children. Reject file/
directory/ancestor/mode conflicts, whole-tree versus param/source/output namespace
collisions, including empty roots. Preserve explicit dirs through param composition.
Generated digests are CAS-only; sources are openable only via prepared observations;
protocol/Directory/param/FileWrite content blobs are locally owned uploads. Missing
generated content fails before consumer Execute even if another upload category
shares that digest. Verify all required digests after staging, including CAS hits.
One channel/instance per attempt; no output-path reads or producer rerun fallback.

New preparation memory is DICE-owned semantic Arc metadata; plans/templates and
verified results are attempt-owned, Directory reconstruction is action scratch,
and verified transfer buffers remain cache-library bounded. No copied depset graph
or cloned action graph. Release on attempt completion/error/cancellation; accepted
output metadata survives only with its result. Complexity: one plan/source pass,
linear step traversal and bounded per-tree reconstruction plus ordered protocol
sorting. Existing large dice.rs receives only module/export delegation; new chain
logic and focused tests live in separate modules. No performance claim.

## Scope and validation

Allowlist: Core runtime new action_chain_staging.rs/action_chain_execution.rs and
focused test files; minimal source_staging.rs helper extraction, dice.rs/mod.rs
registration/exports. REAPI new action_chain.rs and tests; input_tree.rs/tests,
Cargo.toml/BUILD.bazel promoting the existing Analysis dependency for shared typed projection;
source_spawn.rs shared command projection, executor.rs metadata-only verified
output mode/shared lowering, lib.rs exports. Scheduling and Stage 7/bootstrap
status paragraphs. No CLI activation or standalone publication edits.

Parallel workers: Core preparation/lifecycle; REAPI input-tree representation;
root REAPI session/lowering/integration. Independent design then final review.
Discriminators: public source -> File+Directory -> consumer (including FileWrite,
forced params, executable policy, nested files and empty tree policy); diamond
producer once; full source frontier mutation during consumer staging and restored
fresh attempt; unsupported downstream policy fails before effects; producer
failure/missing or corrupt output blocks consumer; generated CAS eviction between
steps blocks consumer; empty-root/param/output and same-digest/mode collisions.
Reuse WP735 output-tree corruption/budget gates and WP736 exact-owner/cycle gates.
NativeLink wire gate uses a tiny public workspace and fresh supervised storage;
portable pure/Core transport tests cover lifecycle/failure boundaries.

Compile separately with pinned nightly, each preparation capped at 60s; exact
selector preflight and focused groups expected under a few seconds. Any >30s
runtime test needs strict necessity; none planned. Compile Core/REAPI plus CLI
direct consumer, rustfmt, diff/plan/archive checks. Receipts target/wp737. Reuse
passing unchanged tests; no broad suite or fresh Bazel run required. Keep review
branch until independent final ACCEPT, then commit/fast-forward main/push as
already authorized. M7A remains partial and M8 unproved. REPLAN for new semantic
identity, producer synthesis outside the closure, or publication/recovery needs.

## Validation receipt

Baseline 99c9f146e, review/wp737-generated-execution. Independent design and final review ACCEPT. Eight Core selectors pass in 3.025s: four new chain gates
(diamond/selected FileWrite, producer-only source and build-frontier changes during
downstream staging, failure/unwind disposal, whole-chain fresh retry) plus four
protected source lifecycle/staging controls. Sixteen focused REAPI selectors pass:
15 in 1.732s plus the added empty-tree namespace case in 0.323s. They cover shared
lowering, explicit directory/mode serialization, params, preflight before connecting,
whole-tree/output/source namespace conflicts and protected tree integrity/budgets.

Four supervised fresh-backend NativeLink selectors pass: public FileWrite ->
File/Directory producer -> forced-param consumer with cold execution, A/A/B/A cache and
content identity in 2.188s; actual generated CAS removal/corruption plus producer
exit/missing-output rejection in 2.123s; protected source-tree execution in 1.247s;
protected FileWrite bytes/mode in 0.292s. The generated/source digest deliberately
matches: after eviction, two failed reads clear NativeLink's stale filesystem
indices and FindMissing must report absence before the adapter rejects it without
source reupload. Corruption is still advertised present and fails verified read.
All backends terminated and fresh roots were removed. No local chain output was
published. No Bazel oracle fixture, CLI activation or broad suite was added/run.

Seven separate pinned preparations total 84.208s, maximum 28.743s under the 60s
preparation cap. Core/REAPI no-run builds and CLI dependency-chain check pass.
Initial REAPI compile needed two routine corrections (Derived is a struct variant;
artifact.path returns Cow). The negative wire fixture needed diagnostic attribution
for NativeLink's stale index and owner-write permission on its exact temporary CAS
inode before corruption. These changed test setup/assertions, not production
semantics. Initial failures and final receipts remain under target/wp737; passing
unchanged selectors were reused after test-only additions/corrections.

Exact selection, rustfmt, diff, plan and archive checks pass. Longest runtime group
was 3.025s; no >30s runtime test was required. Packet wall/review time was not
continuously recorded; Core implementation/validation, input-tree implementation,
wire fixtures and independent review overlapped root REAPI integration.
