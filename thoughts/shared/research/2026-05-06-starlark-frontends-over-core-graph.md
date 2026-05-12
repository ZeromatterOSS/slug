# Starlark Frontends Over A Shared Build Graph Core

**Date:** 2026-05-06
**Repo:** slug
**Question:** Could Slug/Buck2/Bazel-like systems share a core graph engine while
the config-file and build-file frontend is written in Starlark?

## Short Answer

Feasible, but only if "frontend in Starlark" means a thin policy layer that
describes syntax, visible globals, package markers, config files, and rule-call
lowering into a native typed package IR.

It is not attractive if it means a pure-Starlark graph engine or a pure-Starlark
text parser for BUILD files. The hot path should remain native:

- package discovery and filesystem traversal;
- label canonicalization and repo mapping;
- DICE/Skyframe-style dependency tracking;
- target-node and configured-target storage;
- attribute coercion and dependency extraction;
- configuration transitions and toolchain resolution;
- action graph construction, execution, caching, and query.

The tempting middle ground is a "frontend registry":

```text
config files + package marker rules + Starlark env policy
    -> frontend-specific package evaluator
    -> canonical PackageSpec / TargetSpec IR
    -> native graph core
```

That is viable as a research direction. It is a bad near-term pivot for Slug's
Bazel 9 parity work, because the current risk is semantic exactness, not lack of
frontend extensibility.

## Existing Slug Evidence

Slug is already a useful experiment in this separation.

Compared with the imported Buck2-shaped baseline at `3696b5eb`, current Slug has
moved or added major frontend policy:

- `app/slug_common/src/buildfiles.rs`: default package markers changed from
  Buck2's `BUCK.v2`, `BUCK` plus `[buildfile] name_v2` behavior to Bazel-shaped
  `BUILD.bazel`, `BUILD`.
- `app/slug_bzlmod/`: new MODULE.bazel, module resolution, lockfile, repo-spec,
  and module-extension machinery.
- `app/slug_client_ctx/src/bazelrc.rs`: new `.bazelrc` flag injection path.
- `app/slug_interpreter_for_build/src/interpreter/interpreter_for_dir.rs`:
  `@slug_builtins` and `@rules_cc` autoload policy is now layered on top of the
  existing Starlark evaluator.
- `app/slug_interpreter_for_build/src/interpreter/native_rules.rs` and
  `slug_builtins/`: Bazel-compatible globals/rules have been added without
  replacing the underlying package/load graph.

What did not move:

- `app/slug_common/src/package_listing/interpreter.rs` still performs package
  discovery natively through DICE-tracked directory reads.
- `app/slug_interpreter_for_build/src/interpreter/module_internals.rs` still
  records `TargetNode`s through a native target recorder.
- `app/slug_node/src/nodes/unconfigured.rs` and `app/slug_configured/src/nodes.rs`
  still own unconfigured and configured target nodes.
- Attribute coercion, dependency collection, configuration, toolchains, and
  analysis remain native.

This is the practical split: the syntax and environment policy moved; the graph
core stayed.

## External Reference Shape

### Bazel

Bazel does not expose package loading as a Starlark-defined abstraction.
Relevant source:

- `../bazel/src/main/java/com/google/devtools/build/lib/packages/BuildFileName.java`
  hardcodes the recognized package marker names, including `BUILD` and
  `BUILD.bazel`.
- `../bazel/src/main/java/com/google/devtools/build/lib/packages/PackageFactory.java`
  has `executeBuildFile()` as the single package-creation entry point. It runs a
  compiled Starlark program against a native `Package.Builder`.
- `../bazel/src/main/java/com/google/devtools/build/lib/packages/BazelStarlarkEnvironment.java`
  constructs separate environments for BUILD, `.bzl`, MODULE.bazel, REPO.bazel,
  and builtins.
- `../bazel/src/main/java/com/google/devtools/build/lib/bazel/rules/BazelRuleClassProvider.java`
  wires native rule classes and builtins resources.

Bazel is a precedent for "native package engine with Starlark environments",
not for "Starlark owns the graph engine".

### Bonanza

Bonanza is closer to the abstraction this question suggests, but it still keeps
the graph engine native:

- `../bonanza/pkg/model/analysis/package.go` finds `BUILD.bazel` / `BUILD`,
  compiles the file, injects build-file builtins, and records targets through a
  native `TargetRegistrar`.
- `../bonanza/pkg/model/analysis/compiled_bzl_file.go` loads exported builtins
  from Starlark `exports.bzl` files and merges them into `.bzl` and BUILD
  environments.
- `../bonanza/pkg/model/starlark/builtins.go` implements native target
  registration, glob expansion hooks, labels, attrs, providers, rule(), and
  native.* functions.
- `../bonanza/starlark/builtins_core/exports.bzl` and
  `../bonanza/starlark/builtins_core/wrappers.bzl` put large compatibility
  layers in Starlark, including provider definitions and ctx wrappers.

This is a strong precedent for a native graph core with Starlark-authored
builtins and wrappers. It is not a precedent for a pure-Starlark package graph.

## What The Core Would Need To Expose

A shared core has to model the superset of operations that the frontends can
restrict:

- repositories/cells/modules, repo mappings, canonical and apparent labels;
- package discovery, package boundaries, package defaults, and source-file
  ownership;
- load resolution and per-file Starlark environments;
- rule definitions, macros, aliases, source targets, output targets, and
  package groups;
- typed attr schemas, attr coercion, selects, configurable values, and
  dependency extraction;
- visibility and license/package metadata;
- configuration fragments, build settings, user transitions, exec transitions,
  split transitions, aspects, subrules, toolchains, and exec groups;
- repository rules and module extensions with tracked file/network/tool inputs;
- dynamic action discovery where a frontend supports it;
- queries over unconfigured, configured, and action graphs;
- deterministic serialization for persistent caches and remote execution.

The Starlark frontend should be allowed to hide or reject pieces of this
surface. The core must not require every frontend to expose every operation.

## Proposed Architecture

### Native Types

```rust
struct FrontendId(String);

struct FrontendDescriptor {
    id: FrontendId,
    package_markers: Vec<FileNameBuf>,
    config_files: Vec<FileNameBuf>,
    module_files: Vec<FileNameBuf>,
    build_file_dialect: StarlarkDialectId,
    bzl_file_dialect: StarlarkDialectId,
    builtin_exports: FrontendBuiltins,
    restrictions: FrontendRestrictions,
}

struct PackageSpec {
    package: PackageLabel,
    buildfile: FileNameBuf,
    defaults: PackageDefaults,
    targets: Vec<TargetSpec>,
    imports: Vec<ImportPath>,
    diagnostics: Vec<Diagnostic>,
}

enum TargetSpec {
    Rule(RuleTargetSpec),
    Alias(AliasSpec),
    SourceFile(SourceFileSpec),
    PackageGroup(PackageGroupSpec),
}
```

### Starlark Contract

Each frontend exposes a bundled Starlark module with explicit exports:

```python
frontend = struct(
    package_markers = ["BUILD.bazel", "BUILD"],
    config_files = [".bazelrc", "MODULE.bazel"],
    build_globals = {...},
    bzl_toplevels = {...},
    native_members = {...},
    restrictions = struct(
        allow_load_in_module_file = False,
        allow_read_config = False,
        allow_buck2_modifiers = False,
    ),
)
```

The key design choice: Starlark should not return arbitrary graph objects. It
should return data that native Rust validates and lowers into `TargetNode`s.

### Package Loading Flow

```text
PackageListingKey(package)
  -> native directory traversal using frontend.package_markers
  -> read selected build file via DICE
  -> parse with frontend dialect
  -> load .bzl deps via native load resolver
  -> eval in frontend environment
  -> Starlark target calls append TargetSpec data
  -> native lowering validates/coerces attrs and builds TargetNode map
```

This preserves DICE invalidation, concurrency, package-boundary checks, and
queryability.

### Config Loading Flow

Do not parse arbitrary config files during every package load. Config frontends
should produce a session/workspace config snapshot once per invocation or once
per workspace state:

```text
FrontendConfigKey(workspace, frontend)
  -> DICE-tracked reads of declared config files
  -> Starlark parser or native parser
  -> typed ConfigSpec
  -> CLI/server config objects
```

For Slug, `.bazelrc` is cheap enough either way. MODULE.bazel/bzlmod is more
semantically heavy and already uses Starlark evaluation; that is a natural
frontend component.

## Performance Assessment

### Thin Starlark Frontend

Expected impact: modest.

If the Starlark layer only declares environments and target calls append typed
records, the cost is dominated by work Slug already pays: parse/eval BUILD
files and loaded `.bzl` files. Extra overhead is one more Starlark export module
load per frontend plus small per-package dispatch.

Warm daemon: probably below the noise floor for large execution-heavy builds,
and measurable but likely acceptable for query/load-heavy workloads if target
lowering stays native.

Cold daemon: Slug has no persistent Starlark compile cache
(`thoughts/shared/research/starlark-compilation-persistence.md`). More bundled
frontend Starlark increases cold startup and cold package-load cost. That should
be measured before adding large frontend modules.

### Starlark Config Parsers

Expected impact: low if cached per workspace/invocation.

Parsing `.bazelrc`, `.buckconfig`, or frontend descriptors in Starlark is not
the hot path. It becomes a problem only if package loading repeatedly re-runs
config parsers or if config Starlark can perform untracked filesystem I/O.

### Starlark Package Discovery

Expected impact: bad. Do not do this.

Package discovery walks many directories, must be incremental, and must interact
with ignores/watchers/package boundaries. Slug's current
`PackageListingKey -> InterpreterPackageListingResolver -> DiceFileComputations`
path is the right shape. A Starlark directory walker would lose DICE visibility
or require a complex async callback API that recreates the native path poorly.

### Starlark Target Lowering And Attr Coercion

Expected impact: risky to bad.

Per-target and per-attribute work is hot in query and analysis. Moving attr
canonicalization, dependency extraction, select normalization, or target-node
construction into Starlark would add allocation, dynamic dispatch, and
Rust/Starlark conversion costs exactly where Slug needs to be fast.

Keep this native. Let Starlark decide which public function was called and pass
raw values to native coercion.

### Pure Starlark Parser

Expected impact: bad and unnecessary.

BUILD files are already Starlark. Reimplementing lexical/syntax parsing in
Starlark would be slower and would diverge from starlark-rust/Bazel syntax
semantics. The useful plugin point is the dialect/environment, not text parsing.

## Feasibility By Frontend

### Bazel 9

High feasibility, because Slug is already doing it incrementally. The remaining
work is to make the current hardcoded Bazel policy look like a frontend
descriptor without changing behavior.

Do this only after Bazel parity is stable enough that the abstraction cannot
hide parity regressions.

### Buck2

Medium feasibility for a compatibility frontend, but undesirable for Slug's
current product direction.

The original Buck2 shape is still visible in DICE, package listing, target
nodes, and the interpreter. But Slug has deliberately removed Buck2 surface area
for Bazel parity. Reintroducing Buck2 as a first-class frontend would fight
AGENTS.md's Bazel 9-only rule unless it is kept as a private test harness.

### Bonanza-style Bazel

High feasibility as an architecture reference. Bonanza demonstrates that a
native graph engine plus Starlark builtins/wrappers is workable. It also shows
that target registration and graph storage should stay native.

### Pants / Please.build

Unknown without source inspection in this workspace. Conceptually possible only
for the Starlark-shaped parts. Their scheduler/config semantics would still
need native core support if they differ materially from Bazel/Buck.

## Recommended Next Step

Do not pivot Slug now. Finish Bazel 9 parity first.

If we want to de-risk the idea, run a narrow spike:

1. Introduce a native `BuildFrontend` descriptor trait with exactly one
   implementation: `Bazel9Frontend`.
2. Move package marker selection, build-file dialect choice, and builtins
   autoload paths behind that descriptor.
3. Keep the package loader, target recorder, attr coercion, and DICE keys
   unchanged.
4. Add a private `Buck2LegacyFrontend` only for tests against the original
   imported semantics; do not make it user-facing.
5. Benchmark package-load/query workloads before and after. Require no more
   than low single-digit percent regression on warm daemon package loading.

Only after that should Starlark-authored frontend descriptors be considered.

## Decision

The idea is architecturally sound as "native graph core plus Starlark-authored
frontend policy." It is not sound as "build system implemented in Starlark."

For Slug, the strongest version of the idea is also the least disruptive:
continue moving compatibility builtins and wrappers into bundled Starlark, but
keep the graph core native and make the current Bazel frontend explicit only
when doing so simplifies future parity work.
