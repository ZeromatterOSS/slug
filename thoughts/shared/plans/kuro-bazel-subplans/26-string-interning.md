# Plan 26: String interning cleanup for Bazel-compat code

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Follows Plans 16 (benchmark telemetry), 17 (optimization sweep), and
> 21 (warm-invocation overhead). This plan targets performance debt
> introduced by Bazel-compat work since the Buck2 fork: new code often
> used `String`, `HashMap<String, ...>`, and ad hoc `ArcStr::from(...)`
> instead of the existing interning patterns.

## Scope

Make string ownership in Bazel-compatibility code match Kuro's existing
performance model:

- Use permanent interners for stable domain identifiers.
- Use scoped interners for per-load/per-analysis data.
- Avoid raw `String` keys in hot `HashMap`/`FxHashMap` paths when an
  interned domain type exists or should exist.
- Preserve Bazel 9 parity. Interning must not change observable label,
  module, repo, attribute, or lockfile string output.

This is a cleanup and prevention plan, not a broad rewrite. Each
implementation phase needs a focused measurement or memory profile before
and after the change.

## Current State Analysis

Kuro already has several interning layers:

1. `shed/static_interner` for permanent domain values:
   - `CellName`
   - `PackageLabel`
   - `ConfigurationData`
   - `Configuration`
   - `PluginKind`
   - `PluginKindSet`
   - `RemoteExecutorUseCase`
   - `Rule`
2. `ConcurrentTargetLabelInterner` for `TargetLabel` allocation reuse.
3. `BuildAttrCoercionContext` scoped interners for coerced attr strings,
   lists, dicts, and selects.
4. starlark-rust heap/frozen-heap string interners.
5. One `internment::ArcIntern` use in action-query projection data.

The gap is that much of the post-fork Bazel code bypasses these:

- `TargetName` still stores `ThinArcStr` and has `// TODO intern this?`.
- bzlmod/module-extension data carries many repeated `String` names:
  module names, repo names, extension ids, tag class names, attr names.
- toolchain, exec-group, and provider metadata introduced for Bazel
  parity stores `Vec<String>` / `HashMap<String, ...>` even where the
  same names recur across thousands of targets.
- several synthetic/default attr paths construct `ArcStr::from(...)`
  directly instead of going through the existing attr coercion interner.
- `FxHashMap<String, ...>` is used in some bzlmod paths for stable,
  faster hashing, but it still hashes full string bytes and its
  iteration order still depends on insertion order. Plan 21 showed that
  deterministic hashing alone is not an ordering fix.

## Design Rules

### Permanent vs scoped interning

Use `static_interner` only for values that can safely live for the
daemon lifetime and whose distinct cardinality is naturally bounded by
loaded workspaces:

- cell names
- package labels
- target names if adopted
- module names
- repository names
- configuration ids
- rule/provider/toolchain kind names

Use scoped interners for data whose lifetime should end with a package
load, module extension evaluation, analysis, or command:

- raw attr string literals
- attr list/dict/select values
- generated intermediate strings
- parser scratch names

Do not put unbounded user file contents, command lines, environment
values, URLs, integrity strings, or arbitrary action args into a static
interner without a profile and an explicit memory cap story.

### New domain types before generic symbols

Prefer typed wrappers over `Symbol`/`u32` used directly:

- `ModuleName`, `RepoName`, `ExtensionName`, `TagClassName`, etc.
- keep `Display`, `Serialize`, `Deserialize`, `Pagable`, `StrongHash`,
  and borrowed lookup support where the surrounding code needs it.

This preserves type boundaries and makes it hard to mix unrelated string
spaces.

### Hash-map policy

For hot maps:

- Prefer interned domain keys where equality is pointer/id based and hash
  is cached or integer-sized.
- Use `BuckHasher`/`FxHashMap` for deterministic fast hashing where string
  keys must remain raw.
- Do not rely on `FxHashMap` iteration order for correctness. Sort at
  winner-picking boundaries, as Plan 21 does.

For tiny maps:

- Prefer `SmallMap`, `SortedMap`, `Vec<(K, V)>`, or existing local
  ordered structures if they avoid allocations and preserve Bazel output
  order.

## Crate Choice

Default choice: keep using the in-repo `static_interner`.

Reasons:

- Already present in the workspace.
- Integrated with `Allocative`, `Pagable`, `Dupe`, `StrongHash`, borrowed
  lookup, and Kuro's `BuckHasher`.
- Supports interning arbitrary domain values, not only strings.
- Matches the existing core types.

Use the existing scoped attr interners for load-time values.

Do not add `lasso` or `string-interner` by default. They are reasonable
if a future profile specifically needs dense numeric ids plus efficient
key-to-string resolution, but that should be justified by a measured hot
map. `internment::ArcIntern` is acceptable for refcounted structural
sharing where daemon-lifetime leaks are undesirable, but avoid adding new
uses where `static_interner` better matches the domain.

## Expected Wins

Rough order-of-magnitude expectations:

- `FxHashMap<String, V>` with short names is already fast. Interning
  mainly saves byte hashing, byte comparison, and repeated allocations.
- Map lookup/insert hot spots keyed by interned ids/pointers can be
  2-5x cheaper than raw string keys in isolation, especially for labels
  and long repo names.
- End-to-end wall-time impact is likely small unless the target is on
  the load/analysis critical path: expect sub-1% to low-single-digit
  wins from broad cleanup, with larger wins only for specific profiled
  maps.
- Memory wins are strongest when repeated names appear 3x or more:
  each raw `String` occurrence is 24 bytes plus allocation and bytes;
  an intern handle is pointer-sized and shares one stored copy. Hot
  metadata maps can plausibly drop 30-70% of their string-key memory, but
  total daemon RSS depends on how much of the graph is string metadata.

## Phases

### 26.1 Audit and classify string storage (OPEN)

Produce `thoughts/shared/research/2026-04-string-interning-audit.md`
with a ranked table of string-heavy structures introduced or heavily
modified during Bazel compatibility work.

For each candidate record:

- file and type
- string fields / map keys
- approximate cardinality
- duplicate likelihood
- lifetime: static daemon / package load / module extension / analysis /
  command
- current hash/equality path
- whether output ordering depends on the structure
- recommended action: keep raw, scoped intern, static intern, use
  existing typed key, or replace map shape

Seed candidates:

- `app/kuro_core/src/target/name.rs::TargetName`
- `app/kuro_node/src/attrs/spec.rs::AttributeSpec`
- `app/kuro_node/src/rule.rs::Rule` string vectors
- `app/kuro_bzlmod/src/types.rs`
- `app/kuro_bzlmod/src/resolution.rs`
- `app/kuro_bzlmod/src/extensions.rs`
- `app/kuro_bzlmod/src/extension_execution_dice.rs`
- `app/kuro_bzlmod/src/repository_invocations.rs`
- `app/kuro_interpreter_for_build/src/module_ctx/*`
- `app/kuro_interpreter_for_build/src/repository_rule.rs`
- `app/kuro_analysis/src/analysis/toolchain_resolution.rs`
- `app/kuro_analysis/src/analysis/env.rs`

Success criteria:

- At least the top 20 string-heavy candidates classified.
- At least five "do not intern" calls documented, to prevent mechanical
  overreach.
- Proposed first implementation target chosen by measurement value, not
  code aesthetics.

### 26.2 Add lintable guidance for new code (OPEN)

Update project guidance so future agents do not add new string-heavy
Bazel code blindly.

Rules to document:

- New stable identifiers should be typed and interned, or the PR should
  explain why raw `String` is appropriate.
- New hot `HashMap<String, ...>` / `FxHashMap<String, ...>` uses need a
  comment if the key is not a user-output string and the map lives beyond
  a single function.
- Use existing attr coercion interners for coerced attr strings/lists.
- Do not use `FxHashMap` iteration order as a deterministic ordering
  mechanism; sort at consumer boundaries.

Possible enforcement:

- Extend Plan 17.1's AI pattern sweep with a string-storage section.
- Add a simple `rg`-based CI/advisory script only if false positives are
  manageable. Start as documentation plus review checklist.

Success criteria:

- Main plan and Plan 17 both link this plan.
- The guidance is concrete enough for future AI agents to follow before
  writing code.

### 26.3 TargetName intern experiment (OPEN)

Evaluate interning `TargetName`, the most explicit existing TODO.

Important design fork:

1. Intern only `TargetName` values used in pattern results and load maps.
   This is low blast radius but does not change `TargetLabel`, which
   stores the name bytes inline.
2. Redesign `TargetLabel` to store an interned `TargetName` beside the
   interned `PackageLabel`. This may reduce label memory and make name
   equality cheaper, but it touches a central type and needs careful
   measurement.

Start with option 1 unless profiles show `TargetLabel` name storage is a
major RSS contributor.

Acceptance:

- Existing target parsing and display output unchanged.
- Pattern/load tests pass.
- Memory profile on llvm-scale load shows whether duplicate target-name
  storage drops enough to justify the change.

### 26.4 Bzlmod typed-name interning (OPEN)

Introduce interned typed names only where bzlmod repeatedly stores the
same identifiers across resolution, lockfile, extension execution, and
pending cell setup.

Candidate types:

- `ModuleName`
- `RepoName`
- `ExtensionName`
- `TagClassName`
- possibly `ExtensionId` if canonicalization preserves exact Bazel
  string forms.

Keep serde/lockfile output as strings. Do not change
`MODULE.bazel.lock` schema.

Acceptance:

- Lockfile JSON is byte-for-byte stable for existing fixtures unless
  unrelated Plan 21 ordering fixes require sorted output.
- Warm/cold bzlmod resolution benchmarks show no regression.
- Memory profile shows reduced duplicate string storage in bzlmod data.

### 26.5 Attribute/rule metadata cleanup (OPEN)

Tighten the attr and rule-definition layer:

- Audit direct `ArcStr::from(...)` in attr paths and route package-load
  string values through `BuildAttrCoercionContext::intern_str` where
  a context is available.
- Consider an interned attr-name type for `AttributeSpec` only if the
  audit shows duplicated attr names dominate memory. Be careful:
  `AttributeSpec` currently uses ordered maps and attr ids; output order
  and lookup semantics matter.
- Review `Rule` fields added for Bazel parity:
  `provides`, `toolchain_types`, `exec_group_defs`, `fragments`,
  `build_setting_type`. Intern or type only the fields that repeat
  heavily across rule objects.

Acceptance:

- Starlark rule signatures and docs unchanged.
- Attribute lookup remains by Bazel-visible string.
- No regression in package-load benchmarks.

### 26.6 Toolchain and exec-platform metadata cleanup (OPEN)

Review string-heavy toolchain/exec-platform result structs introduced by
Plans 11, 12, 19, 24, and 25.

Likely fields:

- toolchain type labels
- exec group names
- platform labels
- constraint labels
- execution property keys

Do not intern arbitrary execution property values by default; values can
be user-controlled and high-cardinality. Keys and labels are better
candidates.

Acceptance:

- Remote execution action keys and platform property serialization are
  unchanged.
- Plan 24 clang remote E2E still routes the same actions remote/local.

### 26.7 Regression harness and reporting (OPEN)

Use Plan 16 tooling to keep this plan honest:

- Add a benchmark target for package-load/analysis memory where duplicate
  string metadata is visible.
- Record before/after under `benchmarks/<date>-<sha>/`.
- Include at least one allocative profile or RSS measurement for each
  non-trivial interning change.

Acceptance:

- Every Plan 26 implementation commit has a short before/after note.
- Changes without measurable benefit are reverted or explicitly kept for
  design consistency.

## Dependencies and ordering

```
26.1 audit
    |-> 26.2 guidance/checklist
    |-> 26.3 TargetName experiment
    |-> 26.4 bzlmod typed names
    |-> 26.5 attr/rule metadata
    `-> 26.6 toolchain/exec-platform metadata
            `-> 26.7 benchmark/reporting
```

26.2 can land immediately after the audit begins. The implementation
phases can proceed independently once their candidate data is classified.

## Open questions

- Should `TargetLabel` keep storing target-name bytes inline, or should
  it become `(PackageLabel, Intern<TargetNameData>)` with a cached hash?
- Should bzlmod names be daemon-lifetime interned, or should module
  resolution have its own scoped interner that is discarded when the
  resolved graph is replaced?
- Can `static_interner` expose enough metrics to report cardinality and
  retained bytes per interner in allocative output?
- Is `internment::ArcIntern` worth replacing in action query projection
  data for consistency, or is refcounted structural sharing the correct
  choice there?

## Success criteria

- Future Bazel-compat code has a clear string-storage rule of thumb.
- The audit identifies the top string-heavy post-fork structures.
- At least one high-confidence interning cleanup lands with measured
  memory or load/analysis improvement.
- No Bazel-visible strings, lockfile schema, BEP fields, query output, or
  action keys change except where a separate parity plan explicitly
  requires it.
