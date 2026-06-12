# Bzlmod String/Data-Structure Audit (Phase 64.8)

Date: 2026-06-13 (updated 2026-06-12)

## Summary

Audit of key data structures in `slug_bzlmod` for string interning and
collection type opportunities under Plan 26. Principle: no mechanical
rewrite without evidence; comment fixes are cheap and immediate.

## Audit Table

| # | Type | Key Fields | Lifetime | Cardinality | Duplicate Likelihood | Raw Output Order Matters | Recommendation |
|---|------|-----------|----------|-------------|---------------------|------------------------|----------------|
| 1 | `Module` | `name: String`, `version: Version`, `repo_name: Option<String>`, `bazel_deps: Vec<BazelDep>`, `overrides: Vec<Override>` | per-session (DICE-cached) | 10-1000 | Low (module names unique in resolved graph) | No | **Keep raw** — short-lived, low-to-medium cardinality, unique names |
| 2 | `BazelDep` | `name: String`, `version: Version`, `repo_name: Option<String>`, `dev_dependency: bool` | per-session (owned by Module) | 10-1000 | Medium (same dep from multiple modules) | No | **Keep raw** — ephemeral, embedded in parent Vec |
| 3 | Override variants (`LocalPathOverride`, `SingleVersionOverride`, `MultipleVersionOverride`, `ArchiveOverride`, `GitOverride`) | `module_name: String`, plus URL/path/version/patch strings | per-session (owned by Module) | 1-10 | Low (few overrides per workspace) | No | **Keep raw** — very low cardinality; URL/patch/path fields are arbitrary user values, not interning candidates |
| 4 | `ExtensionUsage` | `extension_bzl_file: String`, `extension_name: String`, `isolation_key: Option<String>`, `tags: Vec<ExtensionTag>`, `imports: Vec<UseRepo>`, `repo_overrides: Vec<(String,String)>`, `injected_repos: Vec<(String,String)>` | per-session | 10-100 | Low-Medium | No | **Keep raw** — `extension_bzl_file`/`extension_name` candidates for typed interned name IF profiling shows pressure |
| 5 | `ExtensionTag` | `tag_name: String`, `kwargs: Vec<(String, TagValue)>` | per-session | 100-1000+ | Low (tag_name repeats but kwargs differ) | No | **Keep raw** — `tag_name` candidate for typed interned name (small set: "parse", "install", etc.) |
| 6 | `UseRepo` | `repos: Vec<String>`, `repo_mapping: Vec<(String,String)>` | per-session | 10-100 | Medium (repo names repeated) | No | **Keep raw** — repo name strings candidates for scoped interner |
| 7 | `BzlmodExtensionAggregationsDataValue` | `extension_aggregations: Arc<HashMap<String, AggregatedExtension>>`, `canonical_repo_to_extension_id: Arc<HashMap<String, String>>` | per-session (DICE injected) | 1 per workspace | Low | No | **Consider FxHashMap for `canonical_repo_to_extension_id`** — lookup-only, no serialization needed |
| 8 | `ModuleVersionsValue` | `module_versions: Arc<HashMap<String,String>>` | per-session (DICE computed) | 1 per workspace | Low | No | **Consider FxHashMap** — lookup-only, cheaper for String keys; no evidence yet |
| 9 | `ModuleExtensionResult` | `generated_repo_specs: FxHashMap<String,RepoSpec>`, `canonical_names: FxHashMap<String,String>` | per-session (DICE computed) | 1 per extension (10-100) | Low | **Yes** — cell dedup is first-wins; code explicitly sorts before iterating (`pending_repo_cells.rs:594-595`, `:743-744`; `lockfile.rs:1577-1578`) | **Keep FxHashMap + sort-before-iterate** — BTreeMap would eliminate the sort at negligible cost for tens of entries, but this is a style preference, not a correctness fix |
| 10 | `RepoMappingSnapshot` / `RepoMappingOverrides` | `BTreeMap<String, BTreeMap<String, String>>` | per-session (DICE injected) | 1 per workspace; inner: 10-100 | High (apparent names repeated across modules) | **Yes** — BTreeMap for deterministic JSON digest | **Keep BTreeMap** — already correct for serialization/digest |

## Deterministic-Output Boundaries (Sort Sites)

Every site where HashMap/FxHashMap iteration feeds into a correctness-sensitive
or output-deterministic path was audited. All currently sort before consuming:

| File | Lines | What iterates | Sorted? | Purpose |
|------|-------|--------------|---------|---------|
| `pending_repo_cells.rs` | 594-595 | `generated_repo_specs` | Yes (`specs.sort_by`) | Cell dedup (first-wins) |
| `pending_repo_cells.rs` | 743-744 | `extension_results` FxHashMap | Yes (`sorted.sort_by`) | Extension-level cell merge |
| `lockfile.rs` | 1577-1578 | `generated_repo_specs` | Yes (`entries.sort_by`) | Deterministic lockfile JSON |
| `lockfile.rs` | 802 | extension entries | Yes (`entries.sort_by`) | Deterministic lockfile JSON |
| `lockfile.rs` | 860 | dep entries | Yes (`entries.sort_by`) | Deterministic lockfile JSON |
| `lockfile.rs` | 1511 | candidate keys | Yes (`candidate_keys.sort()`) | Deterministic lockfile JSON |
| `lockfile.rs` | 3415 | extension IDs | Yes (`ids.sort()`) | Deterministic lockfile JSON |
| `dice_graph.rs` | 225-272 | resolved graph maps | Yes (all 5 sort calls) | Deterministic digest |
| `dice_graph.rs` | 3475 | extension IDs | Yes (`extension_ids.sort()`) | Deterministic cell definitions |
| `extensions.rs` | 237 | extension IDs | Yes (`sorted_ids.sort()`) | Deterministic unique name computation |
| `extensions.rs` | 393 | module names | Yes (`module_names.sort()`) | Deterministic hash input |
| `extension_execution_dice.rs` | 366-367 | repo specs | Yes (`repo_specs.sort_by`) | Deterministic cache digest |
| `extension_execution_dice.rs` | 4287 | names | Yes (`names.sort()`) | Deterministic unique names |
| `lib.rs` | 949 | extension IDs | Yes (`extension_ids.sort()`) | Deterministic extension processing |
| `repo_spec.rs` | 103 | attribute keys | Yes (`keys.sort()`) | Deterministic hash |
| `repository_execution.rs` | 770 | inputs | Yes (`stable_inputs.sort()`) | Deterministic execution |
| `repository_invocations.rs` | 96, 191 | keys | Yes (`keys.sort()`) | Deterministic serialization |

No missing sort sites were found.

## Comment Fixes Applied

Three comments overclaimed FxHashMap's iteration stability. The original
comments implied FxHashMap provides "consistent" or "stable" iteration order,
which is false — FxHashMap uses a fixed seed for deterministic *hashing*
(same key → same bucket), but iteration order is still insertion-dependent
under hashbrown. The downstream sort-before-iterate patterns prove the
comments were misleading.

### Fixed

1. **`resolution.rs:345-348`** — Changed from:
   "`FxHashMap` (fixed-seed, deterministic across invocations) so that
   iterating this map produces a consistent order for the same content"
   →
   "`FxHashMap` for deterministic hashing (same key → same bucket), NOT
   for iteration stability. Iteration order is insertion-dependent under
   hashbrown."

2. **`module_extension_executor.rs:97-100`** — Changed from:
   "`FxHashMap` so that iteration order is consistent across invocations
   for the same content (Plan 21.2 — fixes CellResolver churn)"
   →
   "`FxHashMap` for deterministic hashing (same key → same bucket), NOT
   for iteration stability. Iteration order is insertion-dependent under
   hashbrown."

3. **`extension_execution_dice.rs:1174-1176`** — Changed from:
   "`FxHashMap` so iteration is consistent across invocations for the same
   content (Plan 21.2)"
   →
   "`FxHashMap` for deterministic hashing (same key → same bucket), NOT
   for iteration stability. Iteration order is insertion-dependent under
   hashbrown."

All three now reference Plan 21.2/26 and retain the sort-on-read caveat.

## No-Change Decisions

- **No mechanical HashMap → FxHashMap rewrite**: The audit identified
  `canonical_repo_to_extension_id` and `ModuleVersionsValue` as candidates,
  but no memory or load evidence justifies the change yet. Revisit if
  profiling shows allocation pressure.

- **No String → interned type rewrite**: Top candidates (tag_name,
  extension_bzl_file, extension_name, root_module_name, repo names) are
  all short-lived per-session. Interning would add complexity without
  measurable benefit until cardinality grows or profiling shows otherwise.
  Specifically excluded from interning: URLs (ArchiveOverride.urls,
  GitOverride.remote), env values, lockfile text, arbitrary tag values
  (TagValue::String, TagValue::Label), and user-visible output strings.

- **No FxHashMap → BTreeMap for ModuleExtensionResult**: The current
  sort-before-iterate pattern works and is well-documented after the
  comment fix. BTreeMap would eliminate the sort at negligible cost, but
  this is a style preference, not a correctness fix.

- **No Override structure changes**: Override variants (LocalPathOverride,
  SingleVersionOverride, MultipleVersionOverride, ArchiveOverride,
  GitOverride) have very low cardinality (1-10 per workspace) and contain
  arbitrary user values. No interning or collection type change warranted.
