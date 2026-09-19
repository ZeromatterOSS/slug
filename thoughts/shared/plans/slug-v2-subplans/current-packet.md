# Current Slug V2 Work Packet

Packet: WP-7-47-m7a-runfiles-layout-r1
Status: accepted

## Outcome and basis

Add the missing artifact-owned runfiles layout/completion projection required by ordinary
binary Build. WP746 (7ff4cbb2f) activates requested FileWrite/Spawn execution and publication;
a binary's hidden RunfilesTree remains selected but unsupported. The retained RunfilesSupport
already owns the files, symlinks, empty filenames and four support artifacts. Project that
owner into typed ordered entries and a complete constituent artifact view before adding
execution or filesystem effects. This is a prerequisite for faithful runfiles publication,
not a claim that binary Build or M7A is complete.

Pinned Bazel 9.2 source at 8220c6198837d5c13d53fea211cf3282aa12408a:
Runfiles.java:268-315,343-400,481-489 owns mapping precedence and ancestor filtering;
RunfilesSupport.java:467-490,501-543 owns support artifacts/dependencies;
RunfilesTreeAction.java:107-138 produces rich metadata and leaves physical tree creation to
SymlinkTree. RemoteOutputChecker.java:165-211 and RemoteImportantOutputHandler.java:135-148
expand runfiles into backing-artifact downloads. SourceManifestAction.java:294-315,369-424
requires observed absolute artifact targets; SymlinkTreeHelper.java:87-95,167-187 links those
targets and MANIFEST. RunfilesTest.testFilterListForObscuringSymlinksCatchesBadObscurer and
testFilterListForObscuringSymlinksIgnoresOkObscurer supply the diagnostic/no-op themes;
PathFragment.java:514-515 supplies UTF-16 path ordering. These are primary pinned-source
regressions; no fresh Bazel invocation.

## Contract

Build API's retained RunfilesSupport/RetainedRunfiles remain the semantic owners. Add one
pure ephemeral projection, with sorted logical path to exact AnalysisArtifact or
Empty entries and structured prefix/nested-tree diagnostics. Preserve source labels,
Derived owner/configuration/output kinds and shared depset topology. Same-path overwrite is
not itself a conflict: follow the pinned checkAndPut and filter ordering, not an invented
collision policy. Follow normal symlink insertion, artifact override, obscured-child removal,
empty entries, repository placement, root overrides, _repo_mapping and workspace fallback.
Diagnostics retain the configured conflict policy for later event emission; no events or
side effects are emitted by this projection. Do not normalize symlink targets into paths.

Expose a structurally deduplicated constituent artifact view independent of the filtered
layout, including raw runfile targets and input/public/repository mapping manifests required by later
completion, never the virtual tree itself. This transitive backing view is not the immediate
declared-input list (input_manifest is reached through SymlinkTree). An
obscured/overridden entry does not erase its artifact dependency. Clearly distinguish the
source-manifest mapping from the automatic physical tree MANIFEST link to input_manifest.
Do not discard a user-authored root entry named MANIFEST from the logical mapping. Artifact
identity remains exact; existing derived path spelling/configuration identity stays
Slug-native. Repository placement must use canonical source/owner identity rather than
recovering ownership from a string. Final path ordering and filtering follow pinned Linux
semantics; any unavailable path/namespace comparison must fail explicitly, not guess.

Projection strings/maps/vectors and cloned artifact leaves are call-scoped scratch; the
projection borrows RunfilesSupport and keeps retained depset graphs unchanged. Existing visit
callbacks do not expose root-tied lifetimes, so do not widen the shared traversal API;
no new DICE key/cache, retained representation, evaluator object, filesystem query, source
read, remote result or global registry. Use existing depset traversal and compact maps/sets;
ordered output is an oracle requirement. No new dependencies or interner. Stage 9 retained
SmallMap/SmallSet and shared-depset rows apply, without donor import or memory change.

Absolute manifest target rendering, repository-mapping byte encoding, virtual action result
integration, backing-output publication and confined link staging remain the coupled next
execution work. RunfilesTree must not become an ordinary Directory or fabricated remote
Execute result. Preserve planner/Build rejection until all required backing artifacts and
links can be published under native freshness validation. Generic backend symlink outputs,
Run activation, Windows and production compiler/toolchain closure remain outside this packet.

## Scope and evidence

Allow Build API runfiles.rs plus new runfiles/layout.rs and child tests, lib.rs exports;
Core requested_artifacts test wiring plus a new focused runfiles-layout integration child;
current/canonical plans and Stage 7/bootstrap owner notes. No Core production semantics,
REAPI transport/publication, Starlark binding or provider schema changes. Expected 250-450
production and 300-550 proof lines; estimates prompt review, not hard caps. Existing large
files receive only exports/test wiring; implementation and tests live in focused children.

Pure source-derived tests cover main/external source and generated artifacts, last-write
precedence, obscuring prefix filtering, empty entries, root overrides, mapping manifest,
workspace fallback, ordering, same text with distinct owners, complete constituents and
shared depset identity. A tiny ordinary configured executable fixture proves hidden
RunfilesTree → retained support → typed layout/constituents, a generated backing file absent
from DefaultInfo, immutable held A/B/A results and continued requested-execution rejection.
Use the existing hermetic source-staging workspace writer. Tests exercise the public owner
projection and ordinary native analysis, with no invented test-only semantic path.

Independent design and final review. Pinned nightly-2025-09-14; compile separately in bounded
60s preparations, serial shared-target Cargo, exact selector preflight and serial native
fixture runs. Prefer subsecond pure tests and a few-second configured proof; >30s needs strict
necessity. Preserve passed evidence. Format/diff/plan/archive checks, then checkpoint commit,
main fast-forward and authorized push. Receipts target/wp747. Change the design only if
retained metadata cannot establish layout/constituents without new semantic ownership.

Independent design review: ACCEPT. Exact pinned sources, owned scratch leaves over unchanged
retained depsets, complete constituents and separate MANIFEST identity were checked.

## Acceptance receipt

The public RunfilesSupport::layout projection returns typed sorted entries, complete
constituents, structured diagnostics and a separate borrowed MANIFEST link. Exact artifact
identity survives same-path overwrite, obscured entries and structural deduplication. The
ordinary configured executable proof retains its source and generated runfile even though
neither appears in DefaultInfo.files; held A/B/A layouts remain immutable. Requested action
planning still rejects RunfilesTree before execution/materialization.

Seven exact selectors pass in 0.776s total: five runfiles::layout::tests selectors (0.002s)
cover canonical repositories, UTF-16 ordering, overwrite/ancestor/empty/root ordering,
authored versus automatic MANIFEST, diagnostics, malformed paths, constituents and shared
depsets; two Core selectors (0.774s) cover the new configured A/B/A fixture and the existing
hidden-tree/empty/unsupported selection control. Exact selector preflights passed. The API
and Core compiled with pinned nightly-2025-09-14 offline. Initial combined preparation
reached its 60s cap after producing the API test executable; the remaining Core preparation
passed in 27.900s. Pure fixture labels initially omitted canonical @@ prefixes, then were
corrected without changing assertions/production; the API rebuild took 2.133s.

Production is 347 added Rust lines including exports/wiring; proof is 610 lines including
Core test wiring. The modest proof-estimate overrun covers independent public configured
ownership plus pure source-derived negatives in focused child modules. No dependency,
retained data, Core production or REAPI code changed. Receipts target/wp747 preserve commands,
outputs, timings and exact pinned-source hashes. Formatting, diff, plan and archive checks
pass. Independent final review: ACCEPT, confirming the implemented owner projection, pinned
ordering/diagnostics, manifest separation and all seven runtime checks.

This closes the layout prerequisite only. Next connect native source/configured-output
path authority to deterministic manifest projection, then integrate virtual support results,
backing-artifact completion and confined symlink publication together. Do not admit partial
runfiles execution or synthesize remote ActionResults for rich metadata. M7A remains partial
and M8 unproved; WP746's documented external-module-cycle diagnostic defect remains open.
