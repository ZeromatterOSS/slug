# Current Slug V2 Packet

Packet: `WP-6-7A-host-root-apparent-repository-source-observation-observation-carrier-visibility-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design base: pending docs commit / audit `3d865737` / Rust `b3eba6df`

Result: implement only the minimum same-crate handoff for the accepted terminal
legacy/observed source-observation owner. Linux under WSL is the first and only
platform validation target; Windows and macOS work remain deferred.

## Active implementation contract

Implementation `b3eba6df` is the semantic write base. Change visibility only;
the legacy source-observation value/view/result/key and observed key/carrier/
typed outer currently remain same-file private with zero production callers.

Change only the minimum `pub(super)` surface required by a future runtime
sibling:

- the existing semantic certificate, borrowed view/disposition, their read-only
  accessors, opaque semantic error, concrete Result alias, legacy key and its
  existing three-argument `Option<Self>` constructor;
- the existing observed key and constructor, Result-Arc+epoch carrier with
  borrowed concrete accessors, and one opaque field-private typed outer; and
- exactly one test-only sibling smoke in `runtime/mod.rs` proving that another
  runtime module can name both keys, associated Values, concrete Result/view,
  carrier accessors and opaque outer without constructing private state or
  computing either key.

Keep every field, semantic error kind, observed outer inner/variant, driver
mode, reducer and helper private. Rename the current observed outer enum to a
private inner and wrap it only at the observed Key projection. Add no
constructor, conversion, inspector, alias beyond the existing semantic Result,
crate-root export, adapter, copied carrier, semantic caller or compute edge.

Preserve exact legacy and observed key identity, root-name rejection and
Display. For `/workspace`, `@first`, `pkg/file.bzl`, the observed Display remains
exactly:

`observed-HostRootApparentRepositorySourceObservationKey { workspace: NormalizedAbsolutePath { path: "/workspace" }, apparent_repo: ApparentRepoName("first"), requested_path: "pkg/file.bzl" }`.

The future sibling smoke may construct only keys, assert their existing
Display/root rejection, and use nonexecuted exact function-pointer/type checks.
It must not construct a certificate, error, carrier or outer; inspect a private
kind/variant; call `compute`; or activate package, command or bootstrap work.
Existing three observed tests, legacy tests, helpers and semantic assertions
remain frozen except the minimum wrapper/source-shape spelling.

The private inner must be named exactly
`RootApparentRepositorySourceObservationObservationError` and retain the sole
`SourcePath(HostRootApparentRepositorySourcePathInputObservationError)` variant.
The field-private wrapper keeps the existing
`HostRootApparentRepositorySourceObservationObservationError` name and matching
derives/manual `Dupe`. Driver outcome and lower mapping use only the inner;
observed Key `Complete(Err(inner))` is the only wrapper construction site.

The neutral sibling smoke is named exactly
`root_apparent_repository_source_observation_surface_is_sibling_usable`. It is
the only new test. Add no production helper and no owner test. Existing owner
source-shape proof may change only to require one private-inner `SourcePath`
mapping and one opaque observed-Key wrapper projection.

## Consumer-frontier decision

Do not publish or activate the raw terminal carrier. Existing public
`RootRepositoryRoute` and its package source/load owners admit only builtin or
direct-local roots and cannot represent the extension-generated
`@rust_toolchains` chain. Exact generated BUILD loading also still requires a
separate owner for canonical deleted-package policy, `REPO.bazel` plus
`.bazelignore`, and ordered `BUILD.bazel` then `BUILD` selection with complete
epoch composition.

After the visibility implementation is accepted, return only to
`WP-6-7A-generated-repository-package-publication-frontier-audit`. That audit
must choose the smallest policy/lookup/source/load owner or prerequisite; it
must not assume that raw-source publication or the current narrow public route
is sufficient.

## Research and ownership

Accepted commit `1b573d5c` is the same-crate opaque-wrapper and sibling-smoke
precedent; it is concept/test input, not a new semantic donor. The exact Bazel
9.2 source/Host-observation evidence and lifecycle proof accepted by
`b3eba6df` are reused because this packet changes no user-visible behavior.
Add no fixture, oracle or upstream test.

The existing source-observation keys remain the natural DICE producers and the
existing Result Arc plus transaction-local epoch remains the sole retained
value. `docs/developers/dice.md` confirms that visibility alone adds no
dependency, equality, invalidation, lock or publication state. Request inputs,
revision validation, overlapping sessions, event ownership, cancellation and
shutdown behavior remain unchanged. The sibling smoke is test scratch only.
There is no fallback or temporary bridge.

## Authority, caps and Linux-first validation

Rust authority is exactly the source-observation owner and test-only
`runtime/mod.rs` named below. Every third Rust file, production caller, public/
crate-root API, documentation, fixture, oracle, Cargo/BUILD file and other plan
is read-only during implementation.

Entry baselines are exactly:

- `app/slug_core_v2/src/runtime/root_apparent_repository_source_observation.rs`,
  baseline 1,866 physical lines/tests at 562/SHA-256
  `a4b89ce073f70454be89cf17df35fc52d513210d0b075733902be58ee897e993`;
- test-only `app/slug_core_v2/src/runtime/mod.rs`, baseline 251 physical lines/
  SHA-256
  `c52a11c0e082e76cb604ea30798600f07ddbf023b7abfd96f590d515335093a4`.

Implement within <=90 owner production, <=40 owner proof, <=80 sibling proof and
<=210 aggregate additions; physical ceilings are 1,970/340. Add no production
helper, owner test or `rustfmt::skip`; add exactly one sibling smoke below 100
lines. Both files remain cohesive and the owner stays below 2,000 lines. This
is visibility-only and does not trigger hot-path representation work.

The successor must run serially under Ubuntu 24.04 WSL: exact sibling smoke;
the three observed source-observation tests; protected legacy source-
observation and observed source-path/source-input suites; full core with only
the byte-identical accepted query diagnostic baseline; separate runtime with
only the `c8d2d0b5`-identical accepted failure and 12 passes; direct commands
check; formatting; exact two-file baseline/SHA/allowlist/accounting/physical/
visibility/wrapper/source-shape/no-skip and diff hygiene. Add no Windows or
macOS gate, platform abstraction or conditional implementation in this packet.

Legacy semantic values, errors, staging, lower events and equality remain
**exact** Bazel 9 compatibility. Observed Result-Arc+transaction-local epoch
identity/invalidation and same-crate opaque visibility remain **Slug-native**.
Package policy,
lookup/source/load, public command/bootstrap activation, other platforms and
exact identity bytes remain **unsupported/deferred** for this packet.

STOP a third file, public/crate-root exposure, private field/kind/variant
exposure, new key/carrier/adapter, compute/caller/event/semantic/equality/
retention/lifecycle change, package/public/bootstrap activation, Cargo/BUILD,
fixture/oracle, cap/format/test waiver, changed/additional validation failure,
milestone closure, M8/M7B or exact identity work. REPLAN before widening or
baseline drift.

Implementation ACCEPT returns only to
`WP-6-7A-generated-repository-package-publication-frontier-audit`. M7 remains
partial and M7A -> M8 -> M7B remains.
