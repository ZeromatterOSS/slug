# Current Slug V2 Packet

Packet: `WP-5-m1-external-bzl-module-owner-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: read-only design worker
Evidence: accepted Host repository source identity `980373f9`, accepted
request-local external query package identity from the immediately preceding
packet, the pinned Bazel 9.2 external non-test Starlark-rule/load/cycle probes,
and the existing root Host Bzl-module owner. The 17-row, 598-line
`module-local-override` fixture is frozen.

Design only the private route-keyed external Bzl-module and matching cycle
owner required before the dependency-free non-test external Starlark-rule
query slice can resume. Read `AGENTS.md`, the orchestration skill and design
reviewer reference, `docs/developers/dice.md`, and
`.codex/skills/slug-buck2-utility-reuse/SKILL.md`. Read the accepted owner
appendices, `app/slug_loading_v2/src/bzl_module.rs`,
`app/slug_loading_v2/src/cycle_detector.rs`, the Host repository source owner,
typed labels/routes, relevant loading tests, and direct query consumers.

This packet may edit only the Stage 5 owner plan. Do not edit Rust, tests,
fixtures, oracle assets/harnesses, Cargo metadata, protocol, CLI/server,
canonical scheduling, this manifest, or routing records. Run only read-only
inspection and `git diff --check`; no Cargo or Bazel command is authorized.

Select one private external Bzl key/value family whose semantic request is an
already verified `RootRepositoryRoute` plus one normalized canonical external
Bzl label. Specify exact typed normalization for same-package relative and
absolute load spellings. Root/nonroot mismatch, route/canonical mismatch,
cross-package load, and named/canonical-repository load inputs must stop before
source lookup. Do not reuse or generalize the root-only `HostBzlModuleEvalKey`,
`HostRootBzlLabel`, or its cycle node in a way that conflates root and external
identity.

The source dependency must be `HostRepositorySourceFileKey`; consume its
accepted shared bytes and requested normalized logical path to build an honest
`BzlModuleIdentity`. Do not synthesize an output-base path, observe the
filesystem directly, add another source owner, or weaken the legacy immutable
bytes-only equality boundary. Specify whether the new key retains route,
canonical label, package/target components, module identity, compiled/frozen
module, direct/reachable load manifest, event batch, and errors, with exact
`Arc`/`Dupe`/`Allocative` clone and memory costs.

Freeze the complete compute algorithm and ordering: source observation,
parse/compile, load-label resolution, recursive same-repository evaluation,
cycle entry/exit, environment/frozen lifetime, manifest construction, print
event capture, and terminal publication. Give the matching private cycle
guard/node/detector identity and exact ordered cycle diagnostic for
`BUILD -> defs.bzl -> helper.bzl -> defs.bzl`. Prove a parent never retains a
lock or mutable borrow across a DICE compute.

Specify Need/Complete/error equality and validity for every new or changed
key/value. Cover cold publication, warm silence, equal-byte/path pruning,
BUILD and direct/transitive `.bzl` edits, delete/recreate, missing load,
parse/execution error, route remap, cycle, and recovery. DICE must remain the
sole semantic owner; no request generation, operational materialization root,
or apparent alias may enter semantic module equality unless exact observable
behavior requires it.

Audit the activation boundary precisely. The future loading implementation may
allow `RepositoryPackageLoadKey` to evaluate same-package external loads and
retain direct/reachable manifests, but this design must not activate query
Starlark-rule projection, query external Bzl fake candidates, external
visibility content, or generic query output. State what existing native
external package behavior remains unchanged and how unsupported rule classes,
dependencies, generated outputs, tests/executables, and broader labels reject
before partial publication.

Name exact future production and focused-test allowlists, per-file addition
caps, direct downstream checks, serial native/GNU-Windows/format/archive/diff
commands, and the unchanged known CLI-Windows Unix-socket blocker. The likely
production boundary is private code in `bzl_module.rs` and
`cycle_detector.rs`, but do not authorize it until the review proves no third
production file or public API is required. Reuse accepted Bazel evidence; the
fixture and oracle allowlist remain frozen.

Obtain one independent loading/DICE/cycle/lifetime design review before
scheduling implementation. Stop with **REPLAN** if exactness needs a public
cross-crate identity change, root-key reuse, a third source/observation owner,
direct filesystem access, a lock across DICE, non-local override routing,
cross-package/repository load resolution, unbounded discovery, fixture growth,
or partial activation. Query projection, implicit/user dependencies,
test/executable rules, suites, generated outputs, external patterns,
visibility-content evaluation, configuration, analysis/actions/execution,
repository rules/extensions, `@bazel_tools`, JVM, Java bytecode, and Bazel
delegation remain out of scope.
