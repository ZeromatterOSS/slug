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
