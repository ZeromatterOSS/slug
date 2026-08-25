# Current Slug V2 Packet

Packet: `WP-4-5-6-generated-repository-file-effect-owner-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: `04-starlark-loading-and-build-packages.md`,
`05-bzlmod-and-repository-graph.md`, and
`06-analysis-toolchains-and-actions.md`
Base: retained generated-package selected-owner candidate, retained one-file
source-preflight correction, accepted Bazel 9.2 fixture, and two independent
materialization-frontier audits

Result: design one selected-call reload/effect producer for the admitted
`repository_ctx.file` subset and its immutable handoff into the existing
generated-repository materialization lifecycle.

## REPLAN evidence and frozen state

The source-preflight implementation is a valid prerequisite. Its pure polarity
proof and protected direct-local lifecycle/module-error tests pass, the CLI
rebuild passes, and the rebuilt `module-extension-use-repo` fixture advances
past the prior direct-local MODULE route failure. It now reaches the existing
source/materialization path and stops with:

`Materialization { repo_relative_path: "REPO.bazel", error: Spec("unsupported repository override rule") }`.

The terminal is a real missing semantic owner:

1. the generated-definition chain selects the authenticated generated
   repository and retains its owner certificate plus unique call ordinal before
   `GeneratedPackageRouteKey` projects the current route;
2. `HostSelectedExtensionOwnerCertificate` retains the instantiated call and
   `RepoSpec`, but its public iterator deliberately projects only canonical
   name, spec, generated name and mapping;
3. `RepoSpec` retains only rule ID and converted attributes, not the frozen
   callable or defining module lifetime;
4. Bzlmod `request_kind()` and core `materialize_native_attempt()` each admit
   only `local_repository`, `http_archive`, and `git_repository`; and
5. no production repository execution context, `repository_ctx.file` value,
   file-effect manifest or custom-rule materializer exists. The fixture's
   `_repo_impl` must execute two ordered `ctx.file` calls for `BUILD.bazel` and
   `generated.txt`.

Changing either three-rule fallback into a string-based admission would still
have no callable or output bytes and would not be exact. Reconstructing the two
fixture files from attributes is also forbidden. Formally `REPLAN` the
source-preflight implementation packet before widening.

Freeze every dirty Rust path and the fixture byte-for-byte. In particular,
retain the reviewed `host_package.rs` polarity correction as non-writable and
do not commit or discard any selected-owner state. This packet is docs-only.

## Required design decisions

Produce one reviewed implementation design that closes all of these boundaries
without a reverse crate dependency:

- **Selected identity.** Start from the existing authenticated
  `HostSelectedExtensionOwnerCertificate` plus unique repository ordinal. Do
  not rescan the global generated-spec set, infer an owner from `RepoSpec`, or
  execute unrelated extension owners.
- **Callable reload and authentication.** A private loading key must demand the
  defining observed Host `.bzl` module named by the retained call projection,
  select the exported `repository_rule`, and compare its defining label,
  exported name and ordered attribute schema with the certificate before any
  implementation call. The loaded module, `FrozenValue` and Starlark heap are
  compute-local and never enter a retained key value.
- **Admitted execution.** Invoke the selected frozen implementation exactly
  once with an invocation-local repository context admitting only the string
  slice of `repository_ctx.file(path, content="", executable=True,
  legacy_utf8=False)` required by the pinned fixture and Bazel 9.2 source;
  `legacy_utf8` is a typed no-op. Reject Label/path arguments and every other
  repository-context field, method or effect with a typed unsupported terminal.
  The design must settle exact positional/named/default/type/error order and
  repository-relative path normalization before implementation is authorized.
- **Effect representation.** Define the compact shared representation below
  loading, in Bzlmod, so loading can return it and core can consume it without
  dependency inversion. Retain an ordered `Arc` slice of normalized
  repository-relative path, exact content bytes and executable polarity.
  Repeated paths, parent traversal, absolute paths, directory effects and
  invalid Unicode/path coercions are outside the first admitted slice and fail
  with typed unsupported/invalid terminals. Every retained byte participates
  in Eq/Hash/DICE invalidation and `Allocative` accounting.
- **Demand and handoff.** Execute the effect owner only from generated package
  source demand. Thread the authenticated plan through the existing generated
  route/source capability into one generated materialization request; never
  widen canonical-definition lookup or the global aggregate carriers. Core
  applies the ordered plan inside its existing fresh immutable root,
  generation publication and observation-instance lifecycle.
- **Events and epochs.** The observed Host-Bzl child keeps its existing load
  events. The effect invocation owns one complete local Starlark batch. Source
  observations merge left-first in dependency order. Need carries no partial
  effect value; observed outer failure retains only completed predecessors;
  cancellation publishes neither effect plan nor root. Warm reuse emits no
  duplicate batch.
- **Materialization integrity.** The plan and `RepoSpec` remain distinct
  structural inputs. Core must validate workspace/canonical repository/kind,
  create files under an exclusive temporary root without following escaping
  symlinks, set the admitted executable bit, derive source identity from the
  complete ordered plan, then publish through the existing immutable success
  path. Partial roots are never visible.

The design must freeze exact future Rust paths, visibility, nominal types,
legacy/observed key count, key identity, error graph, construction/call order,
line caps and physical ceilings. Prefer new cohesive child modules over growing
the already-large `source_preparation.rs` or retained selected-owner files. It
may select one implementation successor or one uniquely smaller prerequisite;
it may not authorize both.

## Evidence and compatibility

The immutable `tests/v2_oracle/fixtures/module-extension-use-repo` fixture is
the terminal exact evidence. It pins Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` and requires exit zero, canonical
`@@+ext+generated//:generated.txt` identity, source-file classification and
successful completion. The design must add the precise Bazel 9.2 repository
execution anchors before activating Rust. The pinned source already confirms
`RepoRule#instantiate`, `StarlarkRepositoryContext`'s implementation context,
and `StarlarkBaseExternalContext#createFile`: string/Label/path input, string
content default `""`, executable default true, `legacy_utf8` default false and
no-op, output-directory check, parent creation, replacement write using Bazel
internal-string bytes, then executable-bit set. Add no fixture or oracle unless
those sources reveal a discriminating gap not covered by the existing two-file
rule.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is concept-only guidance.
`session_generated_repository_materialization` keys one selected call by
origin/canonical repository/ordinal, reloads its evaluated `.bzl`, executes
effects, and publishes one immutable result under the natural producer. Slug
may adopt that producer/effect split, but copy no Zig code, representation,
scheduler, manifest format, digest, root layout or output vector.

The fixture-visible user-defined repository rule and its ASCII, distinct-path
`ctx.file` calls are **exact Bazel 9**. Private keys, compact effect-plan
representation, valid-Unicode content encoding, event carrier and root/source-
identity plumbing are **Slug-native**. Repeated paths, Label/path arguments,
all other `repository_ctx` APIs, arbitrary repository rules, generated query
breadth, other platforms and exact configuration/output identity remain
**unsupported/deferred**.

## Authority and stops

Docs write authority is exactly canonical/current/Stages 4, 5 and 6/routing,
under net caps <=40/<=280/<=180/<=180/<=240/<=30 and <=950 aggregate lines.
Rust, tests, fixtures, oracles, Cargo/BUILD, generated artifacts and public APIs
are read-only.

STOP any Rust edit; fixture-specific output synthesis; retained Starlark value,
heap or module; Bzlmod -> loading dependency; new global aggregate; eager
extension execution; direct core Starlark execution; direct Host I/O outside
the accepted observed loader; side cache/store/lock/task; native-rule behavior
change; other `repository_ctx` API; Java/JVM; cap/proof waiver; M7 closure;
M8/M7B; or identity-byte work. `REPLAN` if exact `ctx.file` execution cannot be
bounded to one selected call and one immutable file-effect plan.
