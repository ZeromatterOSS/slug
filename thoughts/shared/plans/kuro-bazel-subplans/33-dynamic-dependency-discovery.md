# Plan 33: Generic dynamic dependency discovery

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Adds a Kuro-specific superset feature while preserving Bazel 9 parity for
> ordinary Bazel APIs and rule behavior.

## Goal

Add a generic DICE-backed discovery substrate for dependencies that are not
known until a scanner, manifest parser, or generator has run.

The substrate must support language and toolchain clients such as:

- C++20 module scanning and BMI ordering.
- Proto import discovery.
- Go package import discovery.
- Haskell module import discovery and SCC ordering.
- Cargo.toml / lockfile based target and dependency discovery like the
  first-class behavior rules_rs approximates today.
- Future generated-source and manifest-driven rules that need the same shape.

The design target is:

```
seed inputs + toolchain + resolver scope
  -> DiscoveryKey
  -> deterministic DiscoveryValue
  -> real DICE deps / action inputs / generated analysis nodes
  -> normal Kuro execution and caching
```

## Compatibility Position

Kuro should treat dynamic dependency discovery as an explicit Kuro extension,
not as a relaxation of Bazel-compatible rules.

Rules that use Bazel APIs must keep Bazel behavior:

- A missing declared dependency that Bazel would reject remains an error.
- Sandboxing continues to catch undeclared input reads for Bazel-compatible
  actions.
- Existing rules_* integrations should not silently start depending on inferred
  deps unless the ruleset opts into a Kuro-specific API.

Kuro-native rules and future Kuro extensions may opt into discovery APIs. Those
builds are a superset of Bazel builds, not a different interpretation of the
same Bazel rule surface.

## Conceptual Model

Discovery is a DICE computation, not executor side metadata.

### DiscoveryKey

The generic key should include stable, hashable, typed fields:

```rust
DiscoveryKey {
    owner,
    name,
    configuration,
    discovery_kind,
    seed_inputs,
    toolchain_fingerprint,
    resolver_scope,
    params,
}
```

The exact representation should follow the string storage discipline in the
primary plan: hot identifiers should be typed and interned, not stored as raw
long-lived strings without justification.

### DiscoveryValue

The value should be deterministic and comparable for DICE early cutoff:

```rust
DiscoveryValue {
    discovered_labels,
    discovered_artifacts,
    generated_targets_or_anon_specs,
    used_inputs,
    negative_lookups,
    provides,
    diagnostics,
    fingerprint,
}
```

The value must be normalized before storage:

- deterministic ordering,
- canonical labels and artifact paths,
- no absolute host paths unless explicitly modeled as configured inputs,
- stable diagnostic ordering,
- explicit negative lookups for failed module/package/import resolution.

## Two Discovery Classes

### Analysis-time discovery

Use when discovery changes providers, targets, or the analysis graph.

Examples:

- Cargo.toml and Cargo.lock mapping to generated crate targets.
- Manifest-driven codegen that creates per-language targets.
- Generated proto target sets.

This class should run before or during analysis and return provider requests,
anon target specs, generated target specs, or other typed analysis inputs.

### Execution-time input discovery

Use when the target graph is known enough, but the exact action inputs or action
ordering are unknown until a scanner runs.

Examples:

- C++20 module requires/provides scans.
- Haskell source module graph scans.
- Go import scans.
- Proto import scans for a compile action.

This class should run before the real compile/action. The action execution path
then materializes `static_inputs + discovered_inputs` and computes the action
digest over the real inputs.

## Correctness Impacts

### Positive impact

Correctly modeled dynamic deps prevent rules from over-declaring broad
conservative input sets while still letting Kuro build the actual required deps
before execution.

DICE suspension is a good fit: a computation can ask for discovery, await the
dependencies discovered by that result, then resume without restarting the user
computation from the beginning.

### New correctness risks

- **Missing discovered deps**: if a scanner omits an input that the real tool
  reads, stale outputs are possible.
- **Incomplete resolver scope**: if discovery only sees part of the package or
  module universe, it may resolve to the wrong producer or miss ambiguity.
- **Negative lookup invalidation**: if an import is unresolved today, adding a
  producer tomorrow must invalidate the failed lookup.
- **Nondeterministic discovery output**: unstable order or host-local paths can
  cause cache misses, divergent graph edges, or inconsistent diagnostics.
- **Cycles and phase ordering**: C++20 modules and Haskell modules can have
  language-specific cycles that should produce domain diagnostics, not generic
  DICE cycle errors where possible.
- **Query and target determination precision**: before discovery runs, Kuro may
  only know a conservative possible-deps set.

### Correctness requirements

- Discovery output must be part of the DICE graph and action/cache key path.
- Discovery tools must have declared seed inputs, toolchain inputs, params, and
  resolver scopes.
- Discovery values must record enough provenance to explain why a dependency was
  selected.
- Negative lookups must depend on package/module indexes so newly added sources
  or targets invalidate stale misses.
- Query/aquery/cquery must expose both static deps and discovered deps when the
  latter are available.

## Hermeticity Impacts

### Positive impact

If discovery is modeled as a first-class DICE computation, Kuro can sandbox the
discovery step and the real action separately. This is more hermetic than
letting language tools discover arbitrary files during the compile itself.

This also aligns local execution with remote execution: both treat the input
root as immutable once a process starts. Discovery must therefore happen before
the real action is prepared, not as a late "add more files" operation inside an
already-running action.

### New hermeticity risks

- **Repository-wide scanning by accident**: a scanner that walks the workspace
  without a declared scope can create hidden deps.
- **Host tool leakage**: discovery tools may read compiler installation state,
  module caches, package manager config, or environment variables.
- **Generated source races**: discovery may observe generated files that are not
  modeled as artifacts.
- **Network access**: manifest discovery for ecosystems such as Cargo must not
  fetch implicitly during analysis unless the fetch operation is itself a
  declared repository/module action with lockfile inputs.

### Hermeticity requirements

- Discovery runs under the same or stricter sandboxing as actions.
- Every file read by discovery must be a declared seed input, toolchain input,
  generated artifact, or lookup through a tracked package/module index.
- Environment variables, working directory, platform data, and tool paths must
  be explicit inputs.
- Network access is disallowed in ordinary discovery. Repository/module fetching
  remains in bzlmod/repository-rule style computations with lockfile tracking.
- A strict validation mode should compare scanner output to actual sandbox file
  accesses for early rollout.

## Sandboxing Model

Dynamic dependency discovery must not weaken sandboxing. It adds an earlier
sandboxed step.

### Required execution shape

```
discovery sandbox:
  declared seed inputs + scanner tool + resolver index snapshots
  -> DiscoveryValue

real action sandbox:
  static inputs + discovered inputs + generated discovery artifacts
  -> declared outputs
```

The real action's sandbox is built after discovery. The sandbox must not be
mutated while the action is running. If the real action reads a file that was not
in `static_inputs + discovered_inputs`, that is still an undeclared input bug.

### Sandbox-specific risks

- **Local/remote divergence**: local `--nosandbox` can hide missing discovered
  deps that fail under RE. Dynamic-discovery tests must run with sandboxing and
  with remote execution where available.
- **Scanner over-read**: a scanner that reads the whole source tree can make the
  discovery result look correct while depending on undeclared files.
- **Path rewriting errors**: scanner output may use exec-root, project-relative,
  absolute, symlinked, or virtual include paths. Kuro must canonicalize these to
  artifacts or reject them.
- **Generated input timing**: if discovery needs generated sources, those
  generated artifacts must be built before the discovery sandbox is prepared.
- **Directory and tree artifacts**: discovery over directories must depend on a
  tracked directory listing/digest, not on ambient filesystem enumeration.

### Sandbox requirements

- Local sandbox input roots for discovery and the real action should mirror the
  RE input-root model as closely as possible.
- A scanner may emit only paths that can be mapped to declared seed inputs,
  generated artifacts, toolchain inputs, or resolver-index entries.
- `--nosandbox` remains a debugging escape hatch, not a supported way to make
  dynamic discovery sound.
- Strict mode should optionally fail when the sandbox observes file reads not
  present in the scanner's reported `used_inputs`.
- Symlink handling must match existing dep-file semantics: if a discovered path
  traverses a symlink, the dependency must be on the resolved target or on a
  tracked symlink artifact, not on an untracked host path.

## Remote Execution Impacts

Remote execution makes the dynamic dependency boundary stricter: an RE action's
input root and command digest are fixed before dispatch. A remote worker cannot
ask Kuro/DICE for additional inputs while the action is running.

### Required RE execution shape

Dynamic discovery should use one of these shapes:

1. **Local or remote discovery action, then remote real action**
   - Run the scanner as its own action/DICE computation.
   - Parse its output into `DiscoveryValue`.
   - Resolve discovered labels/artifacts locally in DICE.
   - Build/upload the final RE input root for the real action from static and
     discovered inputs.

2. **Pure DICE parser discovery, then remote real action**
   - For formats such as Cargo.toml or simple proto imports, parse directly in
     DICE without spawning a scanner.
   - The real action is still prepared only after discovery has produced its
     normalized value.

Do not rely on a single remote compile action discovering missing inputs and
continuing after Kuro uploads them. That is not compatible with the RE action
model.

### RE eligibility

A discovery step can be remote-execution eligible only if:

- all seed inputs, generated inputs, scanner tools, and toolchain files are
  represented in the RE input root;
- the scanner does not need local-only services, ambient package manager state,
  undeclared compiler caches, or network access;
- scanner output is deterministic across platforms or the execution platform is
  part of the discovery key;
- path output can be mapped back to Kuro artifacts independent of the remote
  worker's absolute filesystem layout.

Otherwise, discovery should be local-only while the real action may still run
remotely after discovery completes.

### RE caching risks

- **False action-cache hits**: if the real action digest omits discovered inputs
  or omits generated discovery artifacts such as module maps/importcfg files.
- **False discovery-cache hits**: if discovery keying omits scanner toolchain,
  execution platform, resolver scope, environment, or negative lookup index
  version.
- **CAS availability gaps**: discovered inputs produced by prior actions may be
  available in CAS but not on local disk. The dynamic input path should preserve
  deferred materialization and upload/download only when needed.
- **Remote dep-file confusion**: remote dep files are post-execution pruning.
  They can optimize later builds, but they cannot replace pre-execution dynamic
  discovery for correctness.
- **Platform-dependent scanner output**: scanners that print system include
  paths, case-variant paths, or worker-local temp paths can fragment remote
  caches or make results non-replayable.

### RE requirements

- Discovery fingerprints must be included in remote debug metadata and in any
  persistent action-cache validation path.
- If the scanner runs as an RE action, its output artifact must be downloaded or
  parsed from CAS before Kuro resolves discovered deps.
- The real action's RE input root must contain all discovered inputs before
  action digest computation.
- Remote and local execution of the same discovery step must produce the same
  normalized `DiscoveryValue` or fail with a deterministic diagnostic.
- Remote-only builds must fail early if a dynamic-discovery client requires a
  local-only scanner.

## Caching Behavior Impacts

### Positive impact

Fine-grained discovery can reduce unnecessary invalidation:

- a changed unused proto import need not invalidate a compile action;
- a Cargo.toml edit that normalizes to the same crate graph can early-cutoff;
- C++ module consumers can depend on the exact BMI producers they require;
- Go/Haskell actions can avoid broad package or source-tree inputs.

### New caching risks

- **Action digest instability** if discovered inputs are not normalized.
- **False cache hits** if discovered deps are not included in the action digest
  or remote action metadata.
- **Remote cache fragmentation** if discovery emits host-specific paths or
  non-canonical labels.
- **Large DiscoveryValue memory pressure** if full closures are stored in every
  node instead of sharing interned summaries or nested keys.
- **Persistent cache compatibility**: remote dep files and future on-disk action
  caches must distinguish the discovery fingerprint.

### Caching requirements

- DiscoveryValue equality must be meaningful and cheap enough for DICE early
  cutoff.
- Real action digests must include either:
  - the discovered input artifacts directly, or
  - generated command inputs derived from the discovery value, such as importcfg
    files, module maps, response files, or proto descriptor sets.
- Remote cache uploads should include discovery fingerprints in debug metadata
  so mismatches are explainable.
- Persistent action cache entries from Plan 31 must be invalidated by changes to
  discovery fingerprints.
- Discovery results should be cached separately from action results so a
  discovery hit can avoid running scanners even when a compile action must run.
- Discovery scanner action-cache keys and real action-cache keys must remain
  distinct. A scanner cache hit is not evidence that the real action output is
  valid; it only supplies the input graph needed to check or execute the real
  action.

## Performance Impacts

### Potential wins

- Less over-building from broad conservative deps.
- More precise invalidation on warm builds.
- More parallelism once exact edges are discovered, especially for independent
  module/package compiles.
- Better remote/cache hit rates when action inputs stop including irrelevant
  source trees or package closures.
- DICE early cutoff can stop changes in manifests or scan outputs that normalize
  to the same dependency graph.

### Potential regressions

- Extra scanner actions or parser computations on cold builds.
- More DICE nodes and edges, increasing graph memory and validation work.
- Delayed parallelism if discovery creates a serial bottleneck before many
  compile actions can start.
- Larger action setup cost when dynamically materializing discovered inputs.
- Query/TD may become more expensive if it must compute discovery for precision.

### Performance requirements

- Discovery must be incremental and keyed at the smallest practical granularity:
  per source, module, package, manifest, or proto file as appropriate.
- Batched discovery is allowed only when it reduces total cost without creating
  large false serial dependencies.
- The scheduler should expose timing and cache-hit metrics for discovery keys.
- Discovery should support cheap parser implementations before external scanner
  processes when the format permits it.
- Plans 31 and 32 benchmarks should be extended with dynamic-discovery
  workloads before declaring this feature production-ready.
- RE benchmarks should measure both scanner-local/compile-remote and
  scanner-remote/compile-remote modes to catch scheduler, CAS, and input-root
  overhead separately.

## API Direction

Names are placeholders; exact spelling should be chosen when implementation
starts.

### Starlark analysis-time API

```python
discovery = ctx.discover(
    name = "cargo_manifest",
    kind = "cargo_manifest",
    inputs = [ctx.file.cargo_toml, ctx.file.cargo_lock],
    resolver_scope = ctx.attr.resolver_scope,
    params = {...},
)

anon = ctx.actions.anon_target_from_discovery(
    discovery = discovery,
    rule = crate_rule,
    map = ...,
)
```

### Starlark execution-time API

```python
scan = ctx.actions.discover_inputs(
    name = "module_scan",
    kind = "cxx20_modules",
    inputs = [src],
    tool = scanner,
    resolver_scope = module_index,
    params = {...},
)

ctx.actions.run(
    args,
    inputs = static_inputs,
    dynamic_inputs = [scan],
    outputs = [obj.as_output()],
)
```

### Native Rust extension points

- `DiscoveryKey` and `DiscoveryValue` types.
- `DiscoveryResolver` trait for mapping import/module/package strings to
  labels, artifacts, providers, and negative lookups.
- `Action::dynamic_input_specs()` defaulting to empty.
- `ActionCalculation::build_action_no_redirect()` extension point that computes
  dynamic inputs before executor preparation.
- Query/aquery representation for discovered edges.

## Implementation Phases

### 33.1 Discovery data model and DICE keys (OPEN)

Define the core Rust data model, equality, hashing, serialization, and
normalization rules.

Deliverables:

- `DiscoveryKey` and `DiscoveryValue` prototypes.
- Normalization helpers for labels, artifacts, diagnostics, and lookup misses.
- Unit tests proving stable equality across differently ordered scanner output.
- Documentation of which fields are action-cache relevant.

Success criteria:

- Discovery values are deterministic and DICE-cacheable.
- Equal logical discovery output early-cuts off even if scanner raw output order
  differs.

### 33.2 Execution-time dynamic input path (OPEN)

Extend action calculation so an action can request dynamic inputs before
execution.

Deliverables:

- `Action::dynamic_input_specs()` or equivalent.
- `build_action_no_redirect()` computes discovery keys, resolves artifacts, and
  materializes discovered inputs before calling the executor.
- Action digest and command debug output include discovered inputs.
- aquery exposes dynamic discovered inputs after discovery has run.

Success criteria:

- Existing actions with no dynamic specs are byte-for-byte behavior compatible.
- A test rule can scan a source file for imports, discover one generated input,
  build that input, and compile successfully.
- Removing the dynamic import removes the DICE edge and avoids rebuilding the
  former dependency after the next discovery run.

### 33.3 Analysis-time discovery path (OPEN)

Add a generic way for analysis to suspend on discovery and produce anon targets,
generated target specs, or provider requests from the result.

Deliverables:

- Starlark-facing prototype for manifest-driven discovery.
- DICE keys for manifest parse results and generated analysis specs.
- Integration with anon target or equivalent generated target mechanism.
- Query behavior documented for pre-discovery and post-discovery states.

Success criteria:

- A Cargo.toml-style fixture can produce crate targets from a manifest without
  hardcoding them in BUILD.bazel.
- Manifest edits that normalize to the same graph do not invalidate downstream
  analysis unnecessarily.

### 33.4 Hermetic scanner sandboxing and validation (OPEN)

Make discovery trustworthy before enabling it broadly. This phase owns the
local sandbox semantics; remote execution integration is called out separately
in 33.7.

Deliverables:

- Discovery steps run in an isolated environment with declared inputs only.
- Real-action sandboxes are prepared only after dynamic inputs are known; no
  in-place sandbox widening while an action is running.
- Optional strict mode that compares scanner-declared used inputs to sandbox
  observed reads where platform support exists.
- Clear diagnostics for scanners that read outside their declared scope.
- No network access for ordinary discovery.
- Cross-platform test fixtures for Linux namespace sandboxing where available
  and symlink-based fallback sandboxing elsewhere.

Success criteria:

- A scanner that reads an undeclared file fails under sandboxing.
- A scanner that emits a host absolute path is rejected or canonicalized with a
  precise diagnostic.
- A dynamic-discovery action that passes locally under sandboxing also has an
  input set that can be represented as an RE input root.

### 33.5 Language-client prototypes (OPEN)

Build thin clients on top of the generic substrate. These are validation
clients, not one-off infrastructure.

Suggested order:

1. Proto import scanner fixture.
2. Cargo.toml manifest fixture.
3. C++20 module scan fixture.
4. Go or Haskell import fixture.

Deliverables:

- At least two clients using different discovery classes:
  - one analysis-time,
  - one execution-time.
- End-to-end tests for cache hit, cache miss, negative lookup invalidation, and
  sandbox failure.

Success criteria:

- The generic substrate survives both classes without language-specific hooks in
  DICE or the executor.

### 33.6 Observability and performance guardrails (OPEN)

Make the feature measurable before production rollout.

Deliverables:

- Event spans and summary metrics:
  - discovery count,
  - discovery cache hits/misses,
  - scanner wall time,
  - discovered edge counts,
  - negative lookup counts,
  - dynamic input materialization time.
- Benchmark fixtures for cold, warm, and no-op dynamic-discovery builds.
- Memory reporting for DiscoveryValue storage.

Success criteria:

- A warm no-op dynamic-discovery fixture spends near-zero time in scanner
  execution.
- Discovery graph memory and validation cost are visible in `kuro log summary`.

### 33.7 Remote execution and cache integration (OPEN)

Make dynamic discovery work with remote execution without changing the RE action
model.

Deliverables:

- Discovery steps can be marked local-only, remote-capable, or remote-required
  using the same executor preference machinery as actions.
- If a discovery scanner runs remotely, Kuro downloads or CAS-reads the scanner
  output before resolving discovered deps.
- The real action's RE input root includes static inputs, discovered inputs, and
  generated discovery artifacts before the action digest is computed.
- Action cache debug metadata records the discovery fingerprint and discovery
  key identity.
- Persistent action-cache validation from Plan 31 is extended so changed
  discovery fingerprints invalidate cached real-action results.
- Tests cover:
  - scanner local + real action remote,
  - scanner remote + real action remote,
  - scanner local-only under `--remote_only` producing an early diagnostic,
  - discovery output path normalization across local and remote scanners.

Success criteria:

- A dynamic-discovery fixture can run with `--remote_executor` and report remote
  execution for the real action.
- A remote scanner cache hit avoids rerunning the scanner but still validates
  the real action through its own action digest.
- Remote-only builds fail before dispatch when the scanner is not
  remote-execution eligible.

## Open Questions

- Should analysis-time discovery reuse anon targets exclusively, or should Kuro
  add a first-class generated target spec that is query-visible before execution?
- How precise should query and target determination be before discovery has
  executed?
- Do we need a persistent on-disk discovery cache separate from DICE and the
  action cache?
- Which scanner output schema should be blessed as the stable generic format:
  JSON, proto, or typed Rust-only adapters?
- How should language-specific ambiguity rules be represented without baking
  language logic into the generic resolver?

## Non-Goals

- Do not make undeclared dependencies legal for Bazel-compatible rules.
- Do not replace existing dep files. Dep files remain useful for post-execution
  pruning; they are not the core pre-execution discovery mechanism.
- Do not make discovery a generic workspace filesystem walk.
- Do not add network package-manager behavior to normal analysis or action
  discovery.
- Do not implement full C++20, Go, Haskell, Cargo, or proto rule stacks in this
  plan. This plan creates the substrate and validation clients.
