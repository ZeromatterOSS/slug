# Current Slug V2 Packet

Packet: `WP-6-7A-generated-package-host-registry-input-ownership-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design base: independently accepted producer/request input design over retained
bridge candidate `df97f130` and Bazel 9.2 evidence `6fd78a21`

Result: install every already-modeled private Host registry request fact before
the generated-package bridge computes, restoring exact unknown-route fallback
and enabling the accepted generated source build.

## Exact authority and freeze

Write only:

- `app/slug_bzlmod_v2/src/registry_dice.rs`; and
- the exact `NativeDemandRequestInputBundle::normalized_initial` registry line
  in `app/slug_core_v2/src/runtime/dice.rs`.

Keep the remaining retained bridge candidate byte-identical. In particular,
`app/slug_bzlmod_v2/src/host_registry_inputs.rs` is read-only: do not change or
export its private keys, values, normalization or equality.

Before editing require SHA-256:

- `registry_dice.rs` `a4fb383c38ba9860fdfa01b87133b065688cfeb94bf18f3f0d461eac47175df6`;
- `host_registry_inputs.rs` `a253dba09c0c10e51525c268402cb237961130a867e808d0a768c5b7b15feac7`;
- retained `dice.rs` `5645002dafde10a4a532a9257435934f5f0c7031fdd0ed394742c07adb034158`;
- host route `185ec7685abd51851c570762e393df1d59892596854cf6c826603d00a2703c39`;
- generated definition `8166e0c83a0f86e50d251d25b649be18cfd37020434f163a1e06dde723ba27ad`;
- runtime mod `204fd7510b216b9794b6ce646c29ab30dcf2b453bb42c2b402a76da6f41ac651`;
- generated route `27e6ee70e2b95c3b1e48bb6fcca8795fd2ba763cb6b0867ffd7fc9ba87f90818`;
- root definition `a1cf060405c4a5d7be26acc4b23dda542c7c0fad20325fd6fa4b7369f8dc1f3a`;
- build tests `cf96c012f4de303b9b0b0d94d345ecfbc395dc1a81427ea32399503474a067f1`.

The accepted fixture remains byte-identical and read/execute-only.

## Required implementation

Extend the existing public `inject_registry_request_inputs` Bzlmod facade; add
no second facade. Before its first updater mutation:

1. canonicalize an empty `RegistryUrls` to
   `RegistryUrls::default_bazel_registry()` so both Root and Host facts match
   Bazel's admitted empty-argv default;
2. normalize the workspace and call the existing private
   `normalize_host_registry_inputs` over the canonical URL slice with zero
   `HostModuleMirrorOccurrence`s;
3. construct the existing typed `HostRegistryRefreshToken` from
   `generation.0`, keeping the Host token key/type distinct from the legacy
   registry generation; and
4. only after every fallible preparation succeeds, inject Root registry URLs,
   legacy registry generation, Host registry URLs, Host empty mirrors and Host
   refresh token into the same uncommitted updater.

Change core's `normalized_initial` registry value from an empty `RegistryUrls`
to `RegistryUrls::default_bazel_registry()` so retained request equality is the
same canonical value produced by `from_request([])`. Make no other change in
`dice.rs`.

The root-package vendor projection remains separately owned and injected.
Update mode must not read the refresh token; Refresh mode must observe its typed
token. Add no mirror flag, mirror metadata behavior, new key/value, retained
collection, semantic hash, global state or materialization-generation reuse.

This follows architectural guidance from:

- `../zabel/src/load/injected_registry_options.zig` (typed normalized registry
  fact);
- `../zabel/src/request/configured_request.zig` (request-boundary installation);
  and
- `../zabel/src/bzlmod/injected_graph_options.zig` (distinct registry/lockfile
  identities in the graph-sensitive projection).

Do not copy Zig code or representation; Slug remains Rust-native.

## Proof, caps and validation

Add exactly one facade-focused Bzlmod proof covering:

- empty/default and explicit URL agreement;
- ordered first-occurrence normalization and deduplication;
- absence-shaped empty mirrors;
- typed refresh equality plus A/B/A invalidation; and
- the private Host URL/mirror/refresh dependency rows.

Retain the existing Host normalization/equality, vendor independence and
Update/Refresh tests. Mandatory integration gates are the protected
`public_external_single_uses_observed_family_and_full_source_certificate`
unknown-repository lifecycle and rebuilt Slug `module-extension-use-repo`
fixture. Preserve the two bridge command proofs and opaque-outer behavior.

Successor delta caps are <=45 production, <=180 proof and <=225 aggregate.
Cumulative bridge caps from `4d83a829` are <=584 production, <=512 proof and
<=1,096 aggregate. Physical ceilings are `registry_dice.rs` 3,450,
`dice.rs` 11,635 and fixed Host owner 861; every other retained physical size
must remain 4,825/3,985/332/591/1,729/3,941 in the frozen order above. Add no
new `rustfmt::skip`.

Validate serially on Ubuntu 24.04 WSL:

1. the new facade proof, retained Host input tests, classifier, generated-route
   suite, two bridge command proofs and protected external lifecycle;
2. full `slug_bzlmod_v2`; full `slug_core_v2` with only the exact accepted query
   diagnostic baseline; separate runtime with only the accepted
   `PathObservationEpochKey` baseline;
3. clean stale `slugd`, build `slug_cli_v2`, run the unchanged generated fixture
   with the rebuilt absolute `SLUG_V2_BIN`, then clean `slugd` again;
4. direct build/cquery/query command suites, `cargo fmt --all -- --check`,
   `scripts/v2_archive_status.sh` with only its exact three accepted non-V2
   thoughts-path residuals, exact SHA/allowlist/accounting/physical/no-skip/
   utility scans, fixture byte identity, credential scan and `git diff --check`.

## Compatibility and stops

Admitted registry URL normalization, exact fallback diagnostics and generated
source build remain exact Bazel 9 surfaces. Private Host input keys, request-
local refresh generation and bridge identity are Slug-native. Explicit module
mirrors, broader registry metadata application, query/public generated-
repository publication, other platforms and exact configuration/output bytes
remain unsupported/deferred.

STOP Host-owner/lib/CLI/server/command/fixture/harness/Cargo/BUILD edits, public
key/value export, a second registry option representation, empty-default drift,
mirror admission, protected-test weakening, third implementation file, cap or
hash breach, semantic redesign, milestone closure, M8/M7B or exact identity
work. REPLAN before widening. After terminal review, commit all accepted Rust
with the plan outcome. M7 remains partial and M7A -> M8 -> M7B remains.
