# Current Slug V2 Work Packet

Packet: WP-7-188-m7a-observed-input-image-capture-r1
Status: accepted after independent FINAL ACCEPT; no process activation; M7A remains partial

## Outcome and authority

At baseline main6c4558824, implemented WP887's required observed input-image/native
capture capability: a real DICE producer creates a complete bounded source tree
certificate, then native fulfillment copies exactly that observed content/mode/
namespace into a private image with fixed-size buffers and validates its source
frontier. Exercise natural Need → observations → certificate → native capture,
source invalidation/restoration and image lease cleanup. This is a required
native building block for the complete query + locked-generate consumer, not an
alternative get_child-only endpoint or a bootstrap success claim.

[Stage5 selected scratch/process decision](./05-bzlmod-and-repository-graph.md#selected-extension-scratchprocess-decision)
at6c4558824 owns the full admitted/deferred profile. Closed ELF/loader/assets
catalog and unknown-access attribution, quota-backed writable scratch retained
through join/capture, selected module process epoch/replay/activation and real
root execution remain separate required work. A captured declared source tree
cannot claim complete actual ToolRuntimeClosure without those gates.

Reuse WP886 authenticated Bazel9.2/rules_rust source and WP887 design receipts.
Bazel9.2 StarlarkBaseExternalContext.java path/file/read/execute contracts and
ModuleExtensionContext.java cleanup remain authority; no process parity is
introduced here. DICE docs/path observation/digest tests own Need/epoch,
equality/invalidation/frontier behavior. Compatibility is Slug-native captured
image namespace, streaming, resource/confinement and Rust Host facts; exact
file bytes/SHA/mode facts preserve existing content contracts. No JVM/Java.

## Proposed ownership and design checkpoint

Workspace observed input-image key binds namespace/root and keyed limits;
compute derives file/directory/alias/absence facts only from normal observed
resolution/Lstat/FileDigest/ReadLink/directory Needs. Retained certificate is
structural tree/kind/mode/digest/namespace plus observation frontier, no tool
contents or native handle/path side store. A closed bounded directory operation
must stop collection before allocation limits; existing unbounded directory
observation cannot be called and checked afterward. Source membership, content,
mode, alias targets/ancestors and missing/error facts explain invalidation.

Core capture consumes only a producer-issued certificate plus authorized live
source/materialization/session capability, snapshots through descriptor-relative
no-follow access and64KiB streaming copy, verifies actual digest/size/mode and
complete membership, validates source frontier before promotion, and creates
an immutable image lease. Cancellation/stale/error discards incomplete images;
last lease Drop releases image storage. Session revocation/foreign namespace/
materialization cannot produce a usable image. Native stage spelling/handles
are transfer capabilities, never semantic identity or evaluator state. No lock
spans DICE compute, no direct Host filesystem reads inside the key, no fabricated
manifest or command semantic cache. Resolve precise backend/API/normal native
Need routing and alias completeness before implementation at independent design
checkpoint; this contract does not authorize an unreviewed fallback.

Descriptor storage uses the accepted workspace-owned64MiB last-Drop reservation
contract, including old DICE history. Source/tool contents remain streamed disk
native leases, not Arc payloads. Reserve owned record/string/header/capacity
memory before allocation, not just after tree creation. Transient allocation
exhaustion is invalid/incomplete, not reusable Complete error; deterministic
keyed image limits can produce typed source-owned errors with observations.
Define semantic equality independent of physical credit/cache warmth and exact
retirement/cancellation/shutdown owner.

## Scope and staffing

Root edits scheduling, compact Stage5/9 decisions, Workspace shared path-demand/
result bounded-directory integration and narrow exports/registrations as needed.
Input worker owns new Workspace input_image.rs (plus focused tests/submodules)
and descriptor-budget capability after reviewed API. Supervisor worker owns new
Core runtime/input_image.rs (plus cohesive native capture/tests modules). Parent
runtime/mod.rs, NativeDemand/SourcePreparation routing and existing path observer
receive small registration/dispatch deltas owned by root. Root also owns a
cohesive private input_image_bridge.rs child of dice.rs, the source-owned
materialization compute helper and affected exhaustive diagnostic adapters/tests. No workers overlap
files; root may amend allowlist before implementation when reviewed ownership
requires it. New production modules3121 lines plus narrow parent deltas, tests2000;
review physical size/module boundaries rather than appending to huge parents.
No new dependency/lockfile/general cleanup or external messages. No new oracle
fixture unless review finds an exact new surface needing a portable case;
current upstream process/module runtime tests are skipped because phase remains
unsupported. Root replay, child execution, network/acquisition/new extraction,
home/env-value inspection are excluded.

Input and native workers first produce concrete read-only API/design receipts
in target/wp888. Independent reviewer accepts the complete consequential owner/
authority/memory/capture design before code, then reviews actual final diff and
gates. Root integrates, formats and owns all Cargo/runtime validation; workers
run no Cargo/native freshness tests. Source-only research may run concurrently;
Native tests sharing observed ancestor paths remain serial.

## Required gates and acceptance

Require observed producer and actual native wrapper evidence: full membership/
mode/content/digest/order; namespace/owner and materialization authority refusal;
file/directory/alias create/edit/delete/recreate and warm A/B/A; missing/wrongkind/
symlink escape/special/size/count/depth/namebyte violations; no partial image
promotion or unobserved reads; stale content/mode/membership race; cancellation,
revocation, duplicate/foreign certificate, sharing/last-Drop cleanup and parallel
isolated image owners. Alias support must own its bounded target/ancestor graph;
an unresolved arbitrary alias remains typed unsupported, never silently followed.

Resource gates: large streamed input uses fixed buffers rather than file-size
retained payloads; oversized directory stops before unbounded collection; held
historical empty-file descriptors consume workspace reservation through actual
last Drop and overflow yields invalid transient error. Semantic size/equality
results are unaffected by shared allocation/cache warmth. Reuse source/digest
and immutable materialization gates where unchanged; add only demonstrated gaps.

Resolve pinned nightly2025-09-14 Cargo/rustc/rustdoc/rustfmt before Rust validation.
Compile separately under60s preparation caps, select focused tests via Cargo JSON
and one preflight per unchanged executable, then serial bounded test groups.
Direct Workspace/Core and affected Bzlmod/Loading/default CLI compile coverage
must match actual public changes. New native filesystem gates execute ordinarily
without process/JVM/Bazel/bootstrap replay; source/namespace capability is checked
in actual environment. No identical failed runtime retry without new hypothesis.

Final independent ACCEPT, source/structure checks, python3 scripts/v2_plan_status.py,
bash scripts/v2_archive_status.sh and git diff --check required before accepted
commit. Record selected/pass/elapsed/base/toolchain evidence and remaining exact
prerequisites. REPLAN only if source contradicts accepted ownership or complete
capture needs a genuinely new required decision; compiler/selector/capability
corrections stay here. Preserve unfinished substantive candidates on review branch
with receipt rather than restoring/reconstructing them. No process/bootstrap or
whole-daemon RSS claim from input-image tests.

## Immediate predecessor

WP887 at6c4558824 independently accepted selected producer/process identity,
exact replay/path/env, deterministic occurrence and workspace live payload/
descriptor budgets plus two native prerequisites. No runtime/code activation.
Original memory repair d95f2bda6 and WP885's deeper peak1090.074MiB/zero swap
remain protected; full bootstrap/deeper-memory and actual tool asset closure
are unproved.

## Design corrections to freeze before code

Use one dedicated composite PathObservationOperation::InputImage with semantic
namespace/anchor scope and bounded Lstat/digest/directory/readlink request. Its
result owns exact request/data in a credited shared block. Existing generic
operations retain their accepted behavior. Image compute uses only these scoped
operations within namespace+anchor_root+subtree_root+policy; it never falls back
to naked pathname observers. Selected real source/session/root FD authority is
minted before first Need fulfillment, and descriptor-relative no-follow ancestry
checks precede any name/content/link I/O. Capture completion adds normal current
key/used-footprint/frontier proof; constructing a key/scope cannot grant authority.

Image-created paths are ImageChargedPath shared owners. Reserve before join/
copy; dedicated demand construction retains the charged path guard in Demand/
Key/Need, result and final entry/frontier blocks. Credit is nonsemantic in
Eq/Hash/Ord, while source/path/scope/request limits and result data are semantic.
Failed builders/final certificate drop must not release independently retained
key/result/path charges. No raw image-created path/owned row escape in production.
Bounded intermediate observations remain charged through historical DICE values;
foreign workspace budget is rejected on cold/warm consumption and capture.

Observed wrapper equality includes the complete frontier conservatively; its
content/mode projection excludes operational freshness/credit/staging identity.
Initial alias policy is deny or same-root RuntimeContained with complete acyclic
catalog/ancestor targets; cross-root composition remains deferred. Native FD
frontier is DFS depth plus fixed handles, not one retained FD per tree directory.
Preserve exact source modes; final directory chmod is delayed until descendants
are complete. New2GiB workspace live-image credit covers staged regular-file
logical bytes through actual last image Drop, not filesystem metadata/physical
allocation. Empty image metadata remains descriptor-budgeted. This is reviewed
new service policy before measurements, not an increase to any user ceiling.

These corrections supersede worker draft naked PathObservationKey/generic-bound
operation alternatives. Add raced ancestor redirect before first bounded digest
(no outside reads) and failed-builder/demand/history-only path retention gates.
No ordinary native observations may bypass selected authority to manufacture
an image. Complete runtime asset catalog/access attribution and quota-backed
writable scratch/process activation remain unproved after this prerequisite.

Independent DESIGN ACCEPT freezes these corrections over the older drafts. The
Core-private declared-source bridge computes the ordinary image/materialization
keys, checks selected reachable demands/current results, drops transactions and
drains DICE before capture. It restores/discards its transient native attempt
after capture; this primitive publishes no process state and does not persist
image demands into generic accepted preflight. See target/wp888/bridge-review.md.

RuntimeContained in this initial Slug-native substrate rejects any parent (`..`)
component in a link literal before lexical normalization, then resolves only
closed same-root catalog nodes with exact keyed hop/path limits and acyclic
directory/alias edges. Sequential POSIX parent-after-symlink expansion is
unsupported; preserving a literal cannot certify lexical containment alone.
The workspace64MiB claim covers new image-owned descriptor blocks/paths and
producer Need arrays; existing generic DICE/Need bookkeeping remains baseline.
The60s image deadline covers DICE attempts and scoped observer/copy callbacks;
existing materialization progress, blocking syscall time, idle drain and restore
cleanup keep their own owners and are not a hard total wrapper-time guarantee.

Growth review accepts cohesive certificate/budget/scoped observation/native copy
modules rather than appending logic to giant parents. Additional owned path
credits, batching/discovery checks, alias graph proofs and FD/cancel/resource
cleanup account for expansion beyond the initial estimate; no donor/runtime
cache, dependency or unrelated cleanup added. Acceptance: independent FINAL ACCEPT in target/wp888/bridge-review.md.
Workspace36/36 (16 image +20 legacy), Core22/22 (15 native +7 actual wrapper),
and39 protected lifecycle/observer/materializer gates pass; two unchanged extra
UnixListener fixture controls are EPERM-blocked before observation. No ignored
test is counted. Affected Bzlmod/Loading/Core all-target check passes14.35s and
default CLI binary check passes0.62s. Optional CLI integration-test compile fails
at unchanged cli.rs5466/configured_output.rs241 type mismatch; both match baseline.
Pinned compile-only preparation remains capped60s per operation; Core tests use
command-only opt0 for Loading/Analysis/Core after capped dependency preparation.
Final Core preparation12.24s, Workspace8.58s; selected image tests0.107s and
Workspace0.016s. New source modules3121 production/2000 test lines preserve
cohesive ownership. target/wp888/acceptance.json records exact selectors/hashes,
base/toolchain/profile, attributable limits and elapsed receipt lower bound.
No complete runtime asset closure, scratch quota, process activation, root replay
or full-bootstrap memory gate is inferred. Next prerequisite is not yet selected.
