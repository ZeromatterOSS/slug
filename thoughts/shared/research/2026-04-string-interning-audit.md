# Bzlmod String/Data-Structure Audit (Phase 64.8)

Created: 2026-06-13
Last verified: 2026-06-26

## Summary

Audit of key data structures in `slug_bzlmod` for string interning and
collection type opportunities under Plan 26. Principle: no mechanical
rewrite without evidence; comment fixes are cheap and immediate.

Phase 64.8 status: closed as an audit/guardrail slice. The current bzlmod
structures do not justify a code rewrite without memory or load evidence. The
next implementation owner, if profiling later supports it, is Plan 26.4
(`ModuleName`, `RepoName`, `ExtensionName`, `TagClassName`, and possibly
`ExtensionId` typed-name interning).

## Audit Table

| # | Type | Key Fields | Lifetime | Cardinality | Duplicate Likelihood | Raw Output Order Matters | Recommendation |
|---|------|-----------|----------|-------------|---------------------|------------------------|----------------|
| 1 | `Module` | `name: String`, `version: Version`, `repo_name: Option<String>`, `bazel_deps: Vec<BazelDep>`, `overrides: Vec<Override>` | per-session (DICE-cached) | 10-1000 | Low (module names unique in resolved graph) | No | **Keep raw** — short-lived, low-to-medium cardinality, unique names |
| 2 | `BazelDep` | `name: String`, `version: Version`, `repo_name: Option<String>`, `dev_dependency: bool` | per-session (owned by Module) | 10-1000 | Medium (same dep from multiple modules) | No | **Keep raw** — ephemeral, embedded in parent Vec |
| 3 | Override variants (`LocalPathOverride`, `SingleVersionOverride`, `MultipleVersionOverride`, `ArchiveOverride`, `GitOverride`) | `module_name: String`, plus URL/path/version/patch strings | per-session (owned by Module) | 1-10 | Low (few overrides per workspace) | No | **Keep raw** — very low cardinality; URL/patch/path fields are arbitrary user values, not interning candidates |
| 4 | `ExtensionUsage` | `extension_bzl_file: String`, `extension_name: String`, `isolation_key: Option<String>`, `tags: Vec<ExtensionTag>`, `imports: Vec<UseRepo>`, `repo_overrides: Vec<(String,String)>`, `injected_repos: Vec<(String,String)>` | per-session | 10-100 | Low-Medium | No | **Keep raw** — `extension_bzl_file`/`extension_name` candidates for typed interned name IF profiling shows pressure |
| 5 | `ExtensionTag` | `tag_name: String`, `kwargs: Vec<(String, TagValue)>` | per-session | 100-1000+ | Low (tag_name repeats but kwargs differ) | No | **Keep raw** — `tag_name` candidate for typed interned name (small set: "parse", "install", etc.) |
| 6 | `UseRepo` | `repos: Vec<String>`, `repo_mapping: Vec<(String,String)>` | per-session | 10-100 | Medium (repo names repeated) | No | **Keep raw** — repo name strings candidates for scoped interner |
| 7 | `BzlmodExtensionAggregationsDataValue` | `root_module_name: String`, `extension_aggregations: Arc<HashMap<String, AggregatedExtension>>`, `declared_extension_cells: Arc<Vec<_>>`, `canonical_repo_to_extension_id: Arc<HashMap<String, String>>` | per-session (DICE value) | 1 per workspace | Low-Medium | No | **Keep raw HashMap** — lookup-only map; no serialization/output ordering; no profile evidence for typed or Fx keys |
| 8 | `ModuleVersionsValue` | `module_versions: Arc<HashMap<String,String>>`, `invalidation: Arc<_>` | per-session (DICE value) | 1 per workspace | Low | No | **Keep raw HashMap** — lookup-only selected-version map; no evidence yet for typed/interned module names |
| 9 | `ModuleExtensionResult` | `extension_id: Arc<str>`, `input_hash: String`, `generated_repo_specs: FxHashMap<String,RepoSpec>`, `canonical_names: FxHashMap<String,String>`, `metadata`, `recorded_inputs: Vec<String>` | per-session (DICE computed) | 1 per extension (10-100 repos) | Low-Medium | **Yes** — cell dedup and lockfile output are order-sensitive | **Keep FxHashMap + sort-before-iterate** — current consumers sort; typed names need measurement |
| 10 | `RepoMappingSnapshot` / `RepoMappingOverrides` | `BTreeMap<String, BTreeMap<String, String>>` | per-session (DICE injected) | 1 per workspace; inner: 10-100 | High (apparent names repeated across modules) | **Yes** — BTreeMap for deterministic JSON digest | **Keep BTreeMap** — already correct for serialization/digest |
| 11 | `ExtensionSpokesValue` | `extension_id: Arc<str>`, digest `Arc<str>` fields, `spokes: BTreeMap<String, ExtensionSpoke>`, `lockfile_extension_data`, `lockfile_facts`, recorded-input context | per-session (DICE computed) | 1 per extension (10-100 spokes) | Medium | **Yes** — callers iterate spokes and emit lockfile data | **Keep BTreeMap for spokes** — deterministic by construction; lockfile fields are Bazel-visible JSON/text and not interning candidates |

## Deterministic-Output Boundaries (Sort Sites)

Every site where HashMap/FxHashMap iteration feeds into a correctness-sensitive
or output-deterministic path was audited. All currently sort before consuming:

| File | Lines | What iterates | Sorted? | Purpose |
|------|-------|--------------|---------|---------|
| `pending_repo_cells.rs` | 590-596 | `generated_repo_specs` | Yes (`specs.sort_by`) | Cell dedup (first-wins) |
| `pending_repo_cells.rs` | 740-745 | `extension_results` FxHashMap | Yes (`sorted.sort_by`) | Extension-level cell merge |
| `lockfile.rs` | 263-275 | `generated_repo_specs` | Yes (`entries.sort_by`) | Deterministic lockfile JSON |
| `lockfile.rs` | 1260-1266 | extension/fact entries | Yes (`sort_by`) | Deterministic lockfile JSON |
| `lockfile.rs` | 1883-1898 | extension-id canonical spelling | N/A | Canonical string before lockfile use |
| `dice_graph.rs` | 243-285 | resolved graph source fields | Explicit field order | Deterministic digest |
| `dice_graph.rs` | 4686-4702 | canonical repo match candidates | Yes (`matches.sort_unstable`) | Stable conflict winner |
| `extensions.rs` | 228-237 | extension IDs | Yes (`sorted_ids.sort()`) | Deterministic unique name computation |
| `extensions.rs` | 391-398 | module names and tags | Yes (`sort`) | Deterministic extension input hash |
| `extension_execution_dice.rs` | 364-370 | repo specs | Yes (`repo_specs.sort_by`) | Deterministic replay-cache digest |
| `repo_spec.rs` | 103 | attribute keys | Yes (`keys.sort()`) | Deterministic hash |
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

All three live comments still reference Plan 21.2/26 and retain the
sort-on-read caveat as of 2026-06-26.

## No-Change Decisions

- **No mechanical HashMap -> FxHashMap rewrite**: The audit previously flagged
  `canonical_repo_to_extension_id` and `ModuleVersionsValue` as candidates.
  They remain lookup-only, low-cardinality maps and do not feed deterministic
  output. Revisit only if profiling shows allocation or lookup pressure.

- **No String -> interned type rewrite**: Top candidates (tag_name,
  extension_bzl_file, extension_name, root_module_name, repo names) are
  all short-lived per-session. Interning would add complexity without
  measurable benefit until cardinality grows or profiling shows otherwise.
  Specifically excluded from interning: URLs (ArchiveOverride.urls,
  GitOverride.remote), env values, lockfile text, arbitrary tag values
  (TagValue::String, TagValue::Label), and user-visible output strings.

- **No FxHashMap -> BTreeMap for ModuleExtensionResult**: The current
  sort-before-iterate pattern works and is well-documented after the
  comment fix. BTreeMap would eliminate the sort at negligible cost, but
  this is a style preference, not a correctness fix.

- **No Override structure changes**: Override variants (LocalPathOverride,
  SingleVersionOverride, MultipleVersionOverride, ArchiveOverride,
  GitOverride) have very low cardinality (1-10 per workspace) and contain
  arbitrary user values. No interning or collection type change warranted.

## Next Implementation Trigger

Plan 26.4 should only start when there is before/after evidence to collect:

- a bzlmod-heavy target or fixture with repeated module, repo, extension, or tag
  names;
- RSS/allocative/load timing before the change;
- a typed-name design that preserves Bazel-visible string output, lockfile JSON,
  DICE keys, and action/cache identity.
