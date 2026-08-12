# Zabel-Derived Slug V2 Adoption Roadmap

Date: 2026-08-12

Zabel donor commit reviewed:
`c7298478e2e56262a2f438e9c065325744c9f0fc`.

## Decision and scheduling boundary

Adopt the reviewed contracts, fixture themes, and operational ideas through
Slug-owned Rust and the existing Bazel 9.2 oracle harness. Do not port Zabel's
custom DICE scheduler, Starlark runtime, identity bytes, or monolithic module
layout.

This roadmap is a cross-stage adoption checklist, not Stage 11 and not a new
scheduling authority. The canonical Live Status and compact current-packet
manifest remain authoritative. Finish the active M7 packet without widening
it; select the work below only through a later bounded packet.

## Adoption matrix

| Workstream | Owner | Compatibility | Prerequisite | Status |
|------------|-------|---------------|--------------|--------|
| Plan/prompt readiness guide | canonical/orchestration | Slug-native process | none | adopted in planning |
| Starlark/provider/action/toolchain oracle wave | Stage 1 with Stages 4/6/8 | exact observable behavior | Bazel 9.2 provenance | planned |
| Immutable request overlays, source certificates, and provisional publication | Stage 2/M1 | exact isolation/invalidation; Slug-native representation | accepted mutation oracle plus Buck2 DICE audit | planned |
| Six-class memory-lifetime review | all stages | Slug-native architecture | packet touches retained/cache/async state | adopted in planning |
| Natural evaluated `.bzl` producer and repository owner/materializer split | Stages 4/5 | exact semantics; Slug-native Rust boundaries | active M7 graph owners | planned audit |
| Immutable action-owner context | Stage 6 | exact platform/exec-group semantics; Slug-native storage | broader action registration | planned |
| REAPI concurrency/interoperability oracle wave | Stage 7 | exact REAPI behavior | broader action kinds/input trees | planned |
| Sparse AC/CAS repository-output cache | Stages 5/7 | exact only after Bazel-compatible initial identity; otherwise Slug-native | generated-repository owner and recorded inputs | deferred |
| Producer-owned truthful progress | Stages 7/8 | Slug-native presentation | stable execution observations | deferred |
| Explain/provenance command | Stages 6/8 | Slug-native diagnostic format | immutable action owner and revision provenance | deferred |
| Dependency-driven query/cquery/build watch | Stages 2/7/8 | Slug-native command feature over exact invalidation | source certificates and atomic output generations | deferred |
| Real-workspace LLVM ratchet | Stage 8 | supplemental stress evidence | focused fixtures green | deferred |
| Complexity and measured-performance gates | orchestration/all stages | Slug-native process | applicable packet trigger | adopted in planning |

## Planning integration checklist

- [x] Preserve concise worker/root prompts and route durable requirements into
      a shared plan-authoring guide.
- [x] Require learned facts, decisions, non-decisions, proof, producer/key
      ownership, compatibility class, request behavior, memory lifetime,
      fallback deletion, and stop conditions.
- [x] Require Bazel and Buck2 upstream-test mining before local-only semantic or
      DICE coverage.
- [x] Preserve Slug's stronger `fixture.toml` provenance and comparison modes.
- [x] Add donor classification, complexity triggers, active-document hygiene,
      and measured rejected-experiment discipline.
- [x] Record all agreed architecture, fixture, product, and non-adoption items
      in their owning stage plans.
- [x] Keep the active M7 packet unchanged.

## Fixture adoption checklist

Translate fixture themes into the Slug oracle harness. Zabel output is design
evidence only; generate acceptance results from the pinned Bazel 9.2 oracle.

### Wave A: before broader analysis/action work

- [ ] Port the Starlark call/error-order matrix: callable lookup, positional
      and named expansion, duplicate/error precedence, `*args`/`**kwargs`,
      provider initializers, dict/comprehension evaluation, and `ctx.actions`.
- [ ] Add provider schema ordering, missing-versus-`None`, large-schema,
      initializer, key-immutability, hashability, forwarding, cross-owner
      identity, and depset-topology cases.
- [ ] Add action-conflict/error precedence, including failures after action
      registration and execution-time argument expansion failures.
- [ ] Expand aquery structured artifact, depset, param-file, command-filter,
      quoting, and stable ID-topology coverage.
- [ ] Add zero-requested-toolchain versus broken-registration, direct-platform
      deduplication, exec-config `config_setting`, dependency-aspect, and native
      test implicit-input cases.

### Wave B: after the relevant execution/repository owners exist

- [ ] Add REAPI execute concurrency, upload coalescing, ByteStream read
      interoperability, tree-artifact input, and authenticated progress cases.
- [ ] Add remote repository cache mutation/reversion, alternative recorded
      inputs, missing CAS data, transport retry, dependent materialization,
      and sparse control/`.bzl` fetch cases.
- [ ] Add source-symlink manifest path, target, and executable metadata.
- [ ] Promote LLVM Support/Demangle first, then larger LLVM slices, as stress
      ratchets only after every discovered mismatch has a focused fixture.

## Architecture adoption checklist

### Request revisions and source certificates

- [ ] Define a complete immutable request overlay with narrow projections;
      command-local expressions and output modes never become semantic keys.
- [ ] Replace production whole-workspace authority with lazy typed file,
      directory, environment, registry, lockfile, mapping, and repository-rule
      observations.
- [ ] Record exact observed inputs in source certificates.
- [ ] Keep source-facing computation request-private until final reobservation
      accepts one effective revision.
- [ ] Atomically promote compatible provisional values or discard/retry; never
      return mixed-version output.
- [ ] Permit cross-request sharing only when the relevant key, projection, and
      effective revision are compatible.
- [ ] Treat watcher events as invalidation candidates, never correctness proof.
- [ ] Implement this contract using Buck2 DICE; do not import Zabel's custom
      engine or scheduler.

### Action ownership

- [ ] Retain configured owner, semantic configuration identity, admitted
      checksum/display projection, explicit default or named exec group,
      selected execution platform, merged exec properties, toolchain context,
      and aspect provenance at action registration.
- [ ] Make the default exec group an explicit identity, not missing context.
- [ ] Do not reconstruct an action's platform later from only a label or by
      rerunning toolchain resolution.
- [ ] Feed aquery, execution, action identity, progress, and explain output from
      this one immutable owner context.

### Repository and operational extensions

- [ ] Keep bzlmod semantic identities, mappings, descriptors, recorded inputs,
      and repository views above physical archive/Git/rule realization.
- [ ] Keep paths and cache availability out of semantic repository identity.
- [ ] Make any repository-output cache a validated accelerator with sparse
      metadata/control-file demand and lazy ordinary source bytes.
- [ ] Make progress presentation process-owned and producer-observed; never add
      it to DICE identity, action digests, or retained providers.
- [ ] Make explain output cite retained configuration, revision, platform,
      toolchain, action-owner, and reuse/rebuild provenance.
- [ ] Add watch only after exact observations, final validation, fresh
      command-memory iterations, and atomic accepted/provisional output
      generations exist.

## Product improvements over Bazel

- [ ] Native dependency-driven `query --watch`, `cquery --watch`, and
      `build --watch` with exact final source validation.
- [ ] Atomic watched-build output publication and repair of damaged requested
      outputs.
- [ ] Cross-workspace sparse remote repository reuse through validated AC/CAS
      records.
- [ ] Truthful bounded terminal progress without guessed totals or unsupported
      remote queue states.
- [ ] Slug-native explain/provenance output for configuration, toolchain,
      action ownership, and incremental reuse.
- [ ] Preserve remote-only deterministic execution and fail loudly rather than
      introduce semantic local fallback.
- [ ] Preserve separate semantic configuration, display/path, Bazel checksum
      and ActionKey, and REAPI/CAS digest domains.

## Explicit non-adoptions

- [x] Do not port Zabel's custom DICE engine, scheduler, or source-certificate
      implementation wholesale.
- [x] Do not replace `starlark-rust` with Zabel's interpreter/bytecode runtime;
      take tests and performance methods only.
- [x] Do not require all tests to be colocated in production modules.
- [x] Do not create giant central key, provider, action, evaluator, daemon, or
      command files.
- [x] Do not copy giant active worklogs, exhaustive review choreography, or
      prompts that keep agents busy speculatively.
- [x] Do not claim Bazel remote-repository-cache interoperability until exact
      initial identity and wire behavior are demonstrated.
- [x] Do not let a cache, watcher, progress renderer, or explain surface become
      semantic authority.

## Donor evidence index

The reviewed Zabel files are non-hermetic design inputs at the pinned donor
commit above:

- `plans/AGENTS.md` and `plans/01-prior-art-review.md`;
- `plans/04-memory-lifetime-architecture.md`;
- `plans/07-concurrent-sessions-and-input-revisions.md`;
- `plans/08-transitions-configurations-and-action-ownership.md`;
- `plans/11-remote-repository-contents-cache.md`;
- `plans/15-watch-mode.md` and `plans/16-terminal-progress-reporting.md`;
- `plans/21-pre-remote-build-performance.md` and
  `plans/24-starlark-runtime-unification-and-specialization.md`;
- `tests/bazel_oracle/starlark_call_order_test.sh`;
- `tools/verification/analysis_*_bazel_fixture_test.sh`;
- `tools/verification/aquery_*_differential_test.sh`; and
- `tools/recording_reapi_server/*_test.py`.

Each implementation packet must replace these donor references with the exact
Bazel 9.2 source/test anchors and Slug fixture provenance needed for acceptance.
