# Current Slug V2 Packet

Packet: `WP-6-m2-bazel-9-target-configuration-input-ledger`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: freeze the complete Bazel 9.2 input and ownership ledger before any
configuration implementation packet.
Predecessor: `WP-6-m2-general-target-configuration-input-chain-design`, which
returned `REPLAN` after pinned source corrected the native registry from
fourteen classes to seventeen classes and 341 cache-key options.

This is pinned-source/documentation work only. Build a complete auditable
ledger for Bazel 9.2 commit `8220c619` that records:

- all seventeen registered native `FragmentOptions` classes in fully qualified
  checksum order, all 341 cache-key options, defaults, converters, repeat/
  expansion/implicit-requirement/old-name behavior, fragment normalization,
  and final option-name ordering and encoding;
- every non-native input before `BuildOptions#checksum`: rc/config expansion,
  explicit command flags, host/environment observations, CPU and platform
  defaults, repository-mapped labels, platform mapping source and result,
  selected-platform flags, canonical Starlark build-setting values and default
  elision, option scopes, `PROJECT.scl` baselines, and loading Needs;
- exactly one eventual owner for each input across typed command
  normalization, one-shot/daemon wire identity, per-attempt DICE injection,
  Host/loading observations, target-configuration production, and analysis
  configuration identity/transitions; and
- the parse/Need/error/invalidation/equality contract, including one-shot C0
  equality with daemon C0 and exact same-daemon `C0 -> C1 -> C0` restoration
  for native flags, platform mappings/flags, Starlark values/defaults/scopes,
  and acyclic recursive transitions.

Freeze a serial implementation sequence and a concrete first implementation
packet only after every ledger row is source-backed and owned. The expected
layers are typed native vocabulary/normalization; shared command/wire request
identity; Host/platform-mapping/platform-flag graph inputs; Starlark values,
defaults, and scopes; and finally one transactional producer plus checksum,
analysis-key, transition, build, and cquery integration. Do not collapse those
layers merely to avoid `REPLAN`.

Documentation allowlist:

- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- scheduling synchronization in this manifest and the canonical plan
- one terminal routing-log row and its required bounded-history rotation

Cap the ledger at 680 formatted documentation lines. No Rust, test, fixture,
oracle, dependency, generated-file, command-wire, or DICE change is authorized.

Stop on any missing or ambiguous registered option, default, converter,
normalizer, ordering/encoding rule, host observation, platform mapping/flag,
Starlark value/default/scope, loading edge, or eventual owner; caller-supplied
checksum or `first-build`; duplicate one-shot/daemon parsing; failed
normalization producing an accepted snapshot; a new runtime key/cache/global/
lock/interner; configured artifacts, execution-platform retention, Bazel
ActionKey, cquery/aquery formatting, execution/cache/materialization; or cap
breach. Configured-target dependency-cycle semantics are explicitly deferred
with user approval; this packet specifies only acyclic recursive configuration
transitions.
