# Current Slug V2 Packet

Packet: `WP-5-m1-external-bzl-package-query-activation-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: design worker
Evidence: accepted external query package identity `845e89b7`, accepted dormant
external Bzl owner in the immediately preceding packet, pinned Bazel 9.2
same-package load/missing/cycle evidence, and the frozen 17-row, 598-line
external query fixture.

Design only the bounded activation that replaces
`RepositoryPackageLoadKey::LoadsUnsupported` with the dormant route-keyed
external Bzl owner and projects the resulting external package through the
existing request-local query identity. Read `AGENTS.md`, the orchestration
skill design-worker reference, `docs/developers/dice.md`, the accepted owner
appendix, and the live loading/query consumers before proposing edits.

The design must identify the exact package-loading owner for BUILD direct
loads, macro-created native target provenance, canonical semantic identity,
apparent query rendering, `BzlLoadManifest` and frozen-lifetime transfer,
key-local BUILD versus Bzl event ownership, missing/parse/evaluation/cycle
error preparation, and lifecycle invalidation. Audit every generic query
consumer that becomes reachable when an external package manifest is nonempty;
do not infer safety from the current native-only external graph. Preserve
route-to-canonical verification and the existing Private/Restricted visibility
semantics.

Freeze exact changed-file and addition caps, direct dependents, focused tests,
Windows gates, and discriminating Bazel 9.2 evidence. Explicitly decide whether
the existing fixture proves macro-produced `filegroup` origin and loading-file
projection or whether a bounded oracle extension is required first. DICE
`Reused` activations carry no evaluation data; event design must use
evaluation-only metadata and command-side selection without claiming retained
batch replay.

Do not edit Rust, Cargo, fixtures, or oracle records in this design packet. Do
not activate cross-package or cross-repository Bzl loads, mapping/discovery,
non-local overrides, globs, test/executable rules, suites, implicit/user
dependencies, generated outputs, analysis/actions/execution, repository
rules/extensions, `@bazel_tools`, JVM, Java bytecode, or Bazel delegation. Stop
with **REPLAN** rather than hiding any of those behind the package/query seam.

Obtain one independent latest-text loading/query/DICE review. At `ACCEPT`,
append the exact implementation contract to the owner plan and advance this
manifest to the resulting implementation packet; at `REPLAN`, record the
smallest prerequisite instead.
