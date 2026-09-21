# Stage 3: Bazel Identity and Layout

## Goal

Define Bazel-shaped identity and path types before package loading and analysis
depend on them.

## Scope

- canonical and apparent repository names;
- labels and package identifiers;
- target patterns;
- execroot and output-root layout;
- external repository paths;
- runfiles and output path string behavior;
- stable serialization for DICE keys and lockfile-facing values.

## Coherent artifact roots (WP766)

This accepted bounded contract has configured, publication and real transport
evidence plus independent final ACCEPT in the WP766 receipt. Canonical Live Status and the current
packet own activation. Exact Bazel output
bytes remain M9; the following coordinates use Slug-native configuration tokens.

BuildAPI owns immutable `DerivedArtifactRoot` and `AnalysisArtifactRoot` values.
A derived root retains the complete existing `SlugConfiguration` and the bin root
kind, shared through Arc/Dupe/Allocative. It does not retain a target, package,
action owner or execution-platform preference. Equal full configurations share
root semantics across targets/repositories; action owner remains independently
part of `AnalysisArtifact::Derived`. Source roots retain canonical repository
identity within one workspace service and display an empty path. Their logical
identity is Slug-native, not physical-root equality across workspaces or checkouts.

One producer computes the derived exec prefix:
`bazel-out/<configuration component>/bin`. The component is the existing complete
`slugcfg-v2-<hex>` projection with `-exec` appended only for typed Exec role;
Target has no suffix. HostLike root production remains deferred because admitted
action owners already require Target/Exec. The role is read from typed configuration,
never parsed from opaque bytes. This preserves unchanged Rust helpers' first-three-
components and `-exec` tests. Configuration display tokens and collision-sidecar
keys stay independent and unchanged; no Bazel-checksum claim follows from the shape.

`ActionOutput` requires a genuine derived root at construction. Its shared immutable
data retains root, kind and one full exec path; `path()` borrows that full path,
`root_relative_path()` borrows its suffix, and `root()` borrows the descriptor.
Constructors take root-relative coordinates, never detect/strip an apparent root
from input strings. Existing validation rejects malformed relative coordinates
before registry publication. Eq/Hash (and ordering where needed) use complete root
semantics, relative coordinate and kind, not just the projected path. Canonical
configuration bytes are an accepted structural encoding, not a digest. Existing
Arc configuration is shared; no per-output configuration copy, interner, cache,
second model or evaluator borrow. Retained strings use CompactString; output/root
handles remain cheaply cloneable and Allocative. Memory is DICE-retained under
existing provider/action lifetimes, while formatting/transport maps are scratch.

All ordinary predeclared/declared/generated-target producers use one repository-
qualified coordinate helper: main `pkg/file`, external `external/<repo>/pkg/file`.
The explicit bin-relative Solib route bypasses that ordinary prefix: external-owned
Solib outputs can live at `_solib_...` under the shared bin root. Runfiles support
outputs inherit the executable's root and extend its relative coordinate.
Derived File.path is output.path; short_path removes the root and transforms an
actual leading `external/` coordinate to `../`, never blindly inspecting owner repo.
Source File.path/short_path keep their accepted repository projections. File.owner
is the source label or derived action-owner label. Root is immutable/hashable,
repr `<source root>`/`<derived root>`, with only read-only noncallable `path`.
Extracted roots lower/materialize through a typed AnalysisValue variant. Context
bin/genfiles getters obey existing active-frame checks and return the same bin root
only when the structural merge-genfiles option is true; split genfiles fails closed.

Action registry and closure conflicts use full root plus relative coordinates,
including same-root/different-owner conflicts. Two unequal roots with colliding
projected prefixes must fail existing in-memory/durable collision checks before
execution/publication. Currently admitted action output roots must equal the action
configuration's bin root; reject mismatches before transport. The representation
keeps root and action owner separate for later passed-configuration declarations,
but does not activate those declarations or cross-root action outputs now.

REAPI Command output paths, ActionResult reconciliation, generated input bindings,
forced paramfile locations and artifact-rendered argv all use full exec paths.
Do not rewrite arbitrary strings in argv: authored rules now receive correct paths
from File/ctx. Solib naming and runfiles tree member locations instead explicitly
select root-relative or runfiles-relative coordinates, retaining their current
algorithms. Generic low-level ActionSpec construction remains available with rooted
outputs; there is no rootless mode. Legacy configuration identity/key-only tests
remain valid, but output fixtures supply explicit structural configurations.

Core physical staging still opens the validated configured bin namespace and
addresses leaves by `root_relative_path()`. Published capabilities retain rooted
outputs and expose their physical root; callers join that root with the relative
coordinate, not `path()`. Do not widen secure publication to the workspace root.
Runfiles targets and aliases join the same descriptor's prefix exactly once.
Collision claims, source observation/retry, verified CAS and atomic publication
remain unchanged. No new DICE key, filesystem observation, lock across compute or
semantic side registry is introduced. Existing provider/action publication equality
must include all root facts, including unused configuration payload, and restore A/B/A.

The packet maps its invariant-to-test matrix and protected consumers to configured
tests, verified generated-file/tree transport, selected publication and target/exec
cache/change/restore evidence. Getter tests or compilation alone do not establish
this contract. Sibling layout,
split genfiles, execution-time path mapping and alternative output-root configuration
declarations remain unsupported/deferred. Exact existing source paths, content/CAS
digests, and action-family byte algorithms are not weakened by native output tokens.
Direct external generated-label requests remain behind the existing target-shape
guard; external generated Files are admitted through producer providers. Main-repo
generated-target reconstruction must equal the producer's complete rooted artifact.
This boundary was explicitly retained at the WP766 proof review, not widened by
the new root projection.

## V1 Lessons

V1 accumulated too much Buck cell and output-root residue. V2 should not start
with a compatibility exception for `buck-out`; output and exec paths should be
Bazel-shaped unless a Slug extension explicitly documents otherwise.

Archive inspection rule: paths prefixed with `slug-v1-archive:` are absent from
the active clean root. Inspect them with
`git show slug-v1-archive:<path>` or an external archive worktree; do not search
for or import them from the active root. Use the matching
[Stage 9 extraction-ledger](./09-v1-extraction-ledger.md) row to choose the
import mode, oracle, and validation before editing V2 code.

## Implementation Slices

### 3.1 Identity Types

Define typed values before loading/analysis code exists:

- `CanonicalRepoName` for `@@repo` and root `@@`;
- `ApparentRepoName` for `@repo` and root `@`;
- `RepositoryMapping` from apparent to canonical names;
- `PackageIdentifier` as canonical repo plus package path;
- `TargetName` with Bazel validation rules;
- `Label` as package plus target;
- `TargetPattern` for `//pkg:target`, `//pkg`, `//pkg:all`, `//pkg/...`,
  `@repo//pkg:target`, and canonical forms.

Raw strings may appear at parse boundaries and diagnostics only.

Initial concrete files:

- `app/slug_identity_v2/Cargo.toml`
- `app/slug_identity_v2/src/{repo.rs,repo_mapping.rs,package.rs,label.rs,pattern.rs,layout.rs,serialization.rs,lib.rs}`
- `app/slug_identity_v2/tests/{label_roundtrip.rs,pattern.rs,layout.rs}`

Mine `slug-v1-archive:app/slug_bzlmod/src/repo_mapping.rs` for behavior and
`slug-v1-archive:thoughts/shared/plans/slug-bazel-subplans/26-string-interning.md`
for measured interning lessons, but do not expose V1 types. Split
`ApparentLabel`, `CanonicalLabel`, and `ResolvedLabel`; any DICE key built from
an apparent label must include the `RepositoryMappingId` used to resolve it.

### 3.2 Filesystem Layout Model

Model these paths explicitly:

- workspace root;
- output base;
- execroot;
- `bazel-out/<config>/bin`;
- `bazel-out/<config>/testlogs`;
- `external/<canonical-repo>` or the Bazel 9 equivalent verified by oracle;
- convenience symlinks as optional, non-semantic filesystem artifacts.

Do not derive this model from V1 `buck-out` helpers, `slug info`, or archived
artifact path code such as
`slug-v1-archive:app/slug_execute/src/path/artifact_path.rs`. Those paths may be
inspected as rejection examples only.

### 3.3 Serialization and DICE Keys

- Stable serialization for labels, repos, package ids, and target patterns must
  be independent of display formatting.
- Key equality includes repo mapping version where apparent labels are resolved.
- Lockfile-facing values use Bazel spelling, not V1 cell names.

### 3.4 Negative Coverage

Add tests for:

- `@repo//pkg:target` and `@@repo//pkg:target` distinction;
- root apparent repo versus canonical root repo;
- invalid target names and package paths;
- labels crossing package boundaries;
- generated repo aliases that should not collapse to the owner module;
- output paths for source files versus generated files.


## Checkpoint Evidence

Stage 3 initial identity/layout checkpoint:

- Added the `labels-and-output-paths` oracle fixture as the first Stage 3
  fixture. Expected Bazel output remains a documented placeholder until a local
  Bazel 9 binary/source build is available.
- Added `slug_identity_v2` with typed apparent/canonical repo names, repository
  mappings with mapping ids, package identifiers, target names, apparent and
  canonical labels, target patterns, stable serialization, and Bazel-shaped
  execroot/output layout helpers.
- Local validation passed: `cargo test -p slug_identity_v2`, `py -3 -B
  tools/v2_oracle list`, and the Stage 3 forbidden-surface grep over
  `app/slug_identity_v2` returned no matches.
## Exact Test Criteria

- Unit tests parse and round-trip at least 50 label/pattern examples copied from
  Bazel tests or generated from Bazel `query` output.
- Oracle fixture `labels-and-output-paths` builds one source file, one generated
  file, one tree artifact, and one external-repo file and compares path strings.
- `rg -n "buck-out|CellResolver|cell resolver|cell name" app/slug_identity_v2`
  returns no matches outside comments explicitly explaining V1 rejection.
- `rg -n "CellName|CellResolver" app/slug_identity_v2` returns no matches in
  production code.
- Same apparent label under two repo mappings resolves to distinct canonical
  labels and distinct DICE keys.
- Display formatting preserves user-facing apparent labels where Bazel would,
  while storage/debug formatting can show canonical labels.

## Acceptance Criteria

- Round-trip tests for canonical labels, apparent labels, repo mappings, and
  target patterns pass against Bazel oracle fixtures.
- Output path tests cover generated files, tree artifacts, source artifacts,
  external repos, and runfiles.
- Identity structs make illegal states difficult: do not pass raw strings for
  hot graph identifiers.

## Validation

```bash
cargo test -p slug_identity_v2
slug-v2-oracle run --fixture labels-and-output-paths
rg -n "buck-out|CellName|CellResolver|cell resolver|cell name" app/slug_identity_v2
```

## Target-name validation gap blocks loading label normalization (2026-07-23)

The package-context loading label foundation stopped before editing because
`CanonicalLabel::parse` splits at the first colon and `TargetName::parse`
accepts additional colons. Consequently V2 accepts a canonical spelling
constructed from Bazel-invalid relative `pkg:target`, while the accepted
`query-labels-attribute-metadata` oracle requires Bazel's
“absolute label must begin with `@` or `//`” conversion failure.

Reject a loading-local special case. The next packet reviews the central
`TargetName` validator against pinned Bazel
`LabelValidator.validateTargetName` and `LabelParser.Parts.parse`, including
colon/backslash/control/path-segment rules and Bazel's temporary `.`/`/.`
exceptions. Loading resumes only after the core identity boundary is exact.

## Exact target-name validation design accepted (2026-07-23)

Pinned Bazel `LabelValidator.validateTargetName` and
`LabelParser.validateAndProcessTargetName` establish one central value
boundary. `TargetName::parse` must reject colon, backslash, ASCII controls/DEL,
leading/trailing slash, doubled slash, and exact `.`/`..` path segments except
for Bazel's temporarily accepted target `.` and trailing `/.`. The trailing
form is normalized before storage; printable punctuation and Unicode remain
accepted.

Terra's source/V2 audit and root verification found no need to change
`PackagePath` or `split_package_and_target`; Sol accepted that boundary. Raw
relative `pkg:target` classification remains the next loading parser's job.
The implementation packet changes only `package.rs` and focused identity tests,
with no new oracle, interner, repository mapping, DICE owner, or persisted
format.

## Exact target-name validation accepted (2026-07-23)

Commit `22313daa` implements the pinned matrix and trailing `/.` normalization.
Source-derived tests pin exact diagnostic precedence plus value/label equality,
ordering, hashing, display, and stable serialization. Root passed all 10
`slug_identity_v2` tests; loading and query test targets compiled. Sol accepted
the diff after one focused test-strength correction. Package-context loading
may now resume without constructing illegal target identities.
