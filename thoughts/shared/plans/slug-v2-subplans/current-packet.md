# Current Slug V2 Packet

Packet: `WP-4-5-6-7A-root-package-external-bzl-load-owner-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: `04-starlark-loading-and-build-packages.md`,
`05-bzlmod-and-repository-graph.md`, and the ordinary Stage 6/core consumers
whose route outer remains path-only
Base: accepted request-owned selected-registry extension source owner and
accepted root-package external-Bzl owner design

Result: implement one admission-scoped selected-registry projection on the
existing root repository route key and let root package loading consume that
structural route through the existing external-Bzl child. Do not activate
selected-registry routes for query, core source preparation, repository-package
loading or any other ordinary route consumer.

## Accepted behavior and architecture

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is behavioral authority. `PackageFunction` resolves source-ordered direct BUILD
loads through the package repository mapping and demands `BzlLoadValue` children
before package evaluation; `BzlLoadFunction` resolves recursive loads through
the loaded module's mapping. Existing Bazel repository-mapping tests prove root
apparent-name, root `repo_name` and multiple-version resolution. Reuse that
accepted source evidence and the accepted selected-registry source oracle; no
new oracle is required.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` supplies architectural guidance
only. Its package-source layer owns source-ordered resolved direct-load
associations, and its runtime consumes already-observed source plus immutable
resolved modules. Copy no Zig code, representation, scheduling, cache, digest,
path or behavior. Slug remains Rust-native and Bazel 9.2 alone owns behavioral
compatibility.

## Stage 5 producer

Keep one `RootRepositoryRouteKey`; add no second selected key, cache, interner,
registry or path lookup. Add an admission mode that participates in key
equality/hash and has a distinct display only for root-BUILD demand:

- the existing ordinary constructor, display and projection remain exact;
  builtin `bazel_tools`, root direct-local, root, unknown and error bytes do not
  change;
- only the root-BUILD constructor may continue after the original exact
  `Unsupported` result;
- it computes the accepted root apparent mapping, resolves the requested
  apparent name to a canonical selected definition, and admits only
  `SelectedRegistry` by projecting the existing structural selected route;
- missing mapping/definition or a successful root/selected-nonregistry
  definition restores the exact original `Unsupported`; semantic/compute
  failures are typed terminals; Need remains Need;
- the route structurally retains the selected definition, canonical repository,
  `RepoSpec`, local-path policy and ordered producer mapping already owned by
  the accepted route. No physical root becomes semantic authority.

The observed route sibling merges root-module -> root-mapping -> canonical-
definition epochs left-first before semantic projection. Add doc-hidden
projections in the existing selected owners. They must distinguish:

- `Path(ObservedPathFrontierError)`, which remains the existing retry outer;
- `Infrastructure(Arc<str>)`, currently reachable through selected graph ->
  discovered nonregistry closure -> effective-owner DICE compute failure,
  which remains a typed terminal computation error and is never disguised as
  a path frontier.

The projection is exhaustive across every nested non-path compute variant,
including effective, policy, ignore and marker computation failures; it does
not assume the demonstrated effective-owner path is the only reachable one.

Keep typed mapping/definition variants in the route observation error. Ordinary
query/core/source consumers accept only `Path` and invariant-assert that
admission-only selected/infrastructure variants are unreachable. They must not
widen their public outer from `ObservedPathFrontierError`.

## Stage 4 consumer

Preserve `resolve_host_load_label` unchanged for root `.bzl` recursion. Add a
root-package direct-load resolver that distinguishes root/self from apparent
external labels. For each direct load in BUILD source order:

1. root/self uses the existing Host Bzl eval/observation child byte-for-byte;
2. `@apparent//...` computes the root-BUILD-admitted structural route, then the
   existing external Bzl eval/observation child;
3. `@@canonical-nonroot//...` remains explicitly unsupported/deferred.

The child owns Bzl source, recursive evaluation, manifest, event batch and its
own observations. `RootPackageLoadKey` retains only package-evaluation events.
Order is root anchor -> BUILD source -> each external route and Bzl child in
declaration order -> package attempt. Merge epochs left-first before projection;
stop immediately on Need, path outer, typed route/Bzl terminal or package
terminal. Never replay child events.

Retained `LoadedPackage` state may continue owning completed frozen modules and
manifests. Parsing/evaluator scratch stays compute-local. Add no mutable cache,
task, lock, evaluator heap or physical-path authority, and hold no lock across
a DICE compute.

## Exact authority, entry hashes and physical ceilings

Modify exactly these eight files from their accepted dirty entry state:

| File | Entry lines | Entry SHA-256 | Ceiling |
|------|------------:|---------------|--------:|
| `app/slug_bzlmod_v2/src/host_module.rs` | 4,984 | `e58ff6dfdaa0833cf940ef33f3c2bf026e94dfb694e5881ac09460b0efc02b2a` | 5,550 |
| `app/slug_bzlmod_v2/src/lib.rs` | 471 | `fe41b1555821fad001b553ce344b3a9dbfe6308a6a1ac9f5d13961011a315bc5` | 500 |
| `app/slug_bzlmod_v2/src/selected_repo_spec.rs` | 13,668 | `165021dcbe14e7856cf9af1283cc2373d61e049469631793b74f53eb6e46f41f` | 13,850 |
| `app/slug_bzlmod_v2/src/source_preparation.rs` | 16,954 | `266a10a29e308161139e97e683e99225dfbad3959dee04b4f1539d12685f7661` | 17,100 |
| `app/slug_loading_v2/src/bzl_module.rs` | 9,309 | `7d943e5b331d5498948b1505da6fab9d4ff8d9d4bc6a237882ed967fb5f9afac` | 9,700 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | 3,439 | `e021720d5935620ae73a51d9a48ab7acc39355d5222dd82df59f9249a4b4889e` | 4,300 |
| `app/slug_query_v2/src/loading_environment.rs` | 2,168 | `720184b94f4417dba4147e16636bf889976e8ff238afe093456b7f9436c3d5e6` | 2,225 |
| `app/slug_core_v2/src/runtime/dice.rs` | 11,631 | `c10651ec7a5777dbed5db78df57a6d50b5c50f098191a44fe177379a48e8f914` | 11,750 |

Caps are <=900 production, <=1,050 proof and <=1,950 aggregate additions.
Every new helper/test is <=150 lines. Keep route ownership in `host_module.rs`
and package/Bzl ownership in `bzl_module.rs`; extract bounded helpers instead of
materially enlarging `compute_root_package`. `selected_repo_spec.rs` and
`source_preparation.rs` may add only the observation-frontier projections.
Query/core may add only ordinary-mode adapters/invariants. No new file, BUILD
edit, fixture edit or cap waiver is authorized.

Preserve every other dirty or untracked path exactly. If an entry hash differs,
STOP and reconcile the accepted concurrent state before editing.

## Required proof

Direct tests must discriminate:

- admission mode equality/hash/display and ordinary-mode nonactivation;
- exact builtin/direct-local/root/unknown/error behavior;
- root apparent mapping including alias/multiple-version selection;
- selected-registry route identity, `RepoSpec`/policy/mapping invalidation and
  non-selected fallback to the original `Unsupported`;
- root -> mapping -> definition observation order, path versus infrastructure
  outer polarity, merge mismatch, Need/cancellation, warm silence and A/B/A;
- mixed root and apparent-external BUILD loads in source order, route before
  external Bzl child, selected self/mapped recursion, missing/unsupported/cycle
  terminals, child-only events and package-last evaluation;
- one real selected-registry transaction in `host_package_load_tests.rs` using
  the existing public registry-input and materialization seams; add no helper
  visibility merely for tests;
- static proof that query/core/source ordinary constructors do not request
  selected admission.

Run focused and full Bzlmod/loading tests serially, the directly dependent query
tests, and the core check/focused tests. Do not run Cargo commands concurrently
in the shared target. If the V2 CLI path changes, rebuild
`cargo build -p slug_cli_v2` before using `SLUG_V2_BIN`; clean exact stale
`slugd` processes before and after daemon-sensitive checks.

Finally replay the disposable rules_rust command with only the parked wildcard
registration removed. It must advance from the root Host-loader rejection to
the existing unsupported `repository_rule(doc=...)` terminal; it must not be
made successful. Validate formatting, `git diff --check`, authority, entry
accounting, physical ceilings, no public selected-route activation and no
unrelated dirty-state drift. Obtain independent terminal review before
acceptance.

## Compatibility and STOP

- **Exact:** Bazel 9.2 root BUILD apparent external-load resolution through the
  root mapping for already admitted builtin/direct-local/selected-registry
  sources, plus existing root/error behavior.
- **Slug-native:** admission mode/key representation; sequential DICE,
  observation/event/error carriers; structural selected route and heap lifetime.
- **Unsupported/deferred:** canonical nonroot direct BUILD loads; selected
  nonregistry beyond direct-local; generated extension-repository direct BUILD
  loads; wildcard registration; repository-rule `doc`, schemas, invocation and
  effects; toolchains/providers/actions/input trees; crate_universe; M8/M7B;
  exact configuration/output identity bytes.

STOP a second route key/cache, mapping or route reconstruction in loading,
ordinary/public selected activation, outer-type widening, path/infrastructure
flattening, event replay, fixture/oracle edits, `@bazel_tools` invention,
Java/JVM, broader repository/ruleset semantics, milestone closure or a second
successor. `REPLAN` before widening.
