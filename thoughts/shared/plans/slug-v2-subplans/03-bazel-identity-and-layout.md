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

Mine `app/slug_bzlmod/src/repo_mapping.rs` for behavior, but do not expose V1
types. Split `ApparentLabel`, `CanonicalLabel`, and `ResolvedLabel`; any DICE
key built from an apparent label must include the `RepositoryMappingId` used to
resolve it.

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
artifact path code. Those paths may be inspected as rejection examples only.

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
