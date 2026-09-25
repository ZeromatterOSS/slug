# Current Slug V2 Work Packet

Packet: WP-7-150-m7a-default-apple-dotted-version-r1
Status: ACCEPTED; M7A remains partial

## Outcome and compatibility

Advance the rebuilt authentic Rust-toolchain `cquery //:probe --output=label`
past the WP849 stop at pinned `@@apple_support+//xcode:xcode_config.bzl:400`,
where `apple_common.dotted_version(field)` lacks a method. Admit only the
five SDK default strings actually passed before provider construction on
the Linux `local_config_xcode` empty-version path: `8.4`, `10.11`, `9.0`,
`2.0`, and `1.0`. Return a genuine immutable, truthy `DottedVersion`
Starlark value, preserving its original spelling for `str`, `repr` and
the `to_string` field. Equality and hash must distinguish the five
numeric tuples consistently; trailing-zero normalization for other
spellings is outside this finite public domain.
The constructor remains Bzl-only through the existing `apple_common`
global. A wrong type or any other string fails closed; no raw-string or
`None` substitute is acceptable.

The five source-demonstrated inputs and their resulting value type,
truthiness, spelling, and pairwise equality/hash behavior are `exact` for pinned
Bazel 9.2 and apple_support 1.24.2. The evaluator-local Rust value and
its retained-memory representation are `Slug-native`. General
DottedVersion parsing (alpha/descriptive/overflow/Unicode), `compare_to`,
non-default versions, non-null Apple configuration options, Xcode provider
construction, generated-repository bytes, and M7A closure remain
`unsupported/deferred`. The finite admission does not assert the entire
public Bazel `apple_common.dotted_version` API.

## Pinned source, owner and boundary

The WP849 authentic replay first stopped at `xcode_config.bzl:76,399-400`.
The pinned file SHA-256 is
`7db1f74ab1dfac13e3988d8ab29578e6fd4ccddf82beb0e2f98d232b57e482ca`.
Its `xcode/private/providers.bzl:36-54` initializer supplies the five
literal defaults to calls at `xcode_config.bzl:76-85`; the first argument
is `8.4`, not `None`. Bazel 9.2 `AppleCommonApi.java:146-152` and
`AppleStarlarkCommon.java:50-56` define the string constructor and
`DottedVersion.java:151-218,227-294,336-400` owns parsing, semantic
equality/hash, original spelling and truthy object behavior. Pinned
`DottedVersionTest.java:30-67,127-157` and
`ObjcStarlarkTest.java:1268-1338` are regression anchors; broad grammar
cases in those tests are deferred, since this packet's only source
consumers pass the five defaults. `DottedVersion.Option` has distinct
raw-string equality and must not be conflated with this Starlark value.

`slug_loading_v2/src/apple_common.rs` owns the Bzl-only facade and
method table. Add the finite value there or in one narrow sibling module;
do not change configuration's retained native option vector or WP849's
non-null gate. The constructor result is evaluator-local and can be
frozen into a loaded module; a frozen value retains only its compact
immutable numeric components and original literal. No new DICE key,
cache, interner, host observation, lock, or global registry is needed.
The existing loaded-source certificate and configured graph still own
source/configuration invalidation. The accepted Stage 9 retained-utility
policy applies: use a simple small Allocative value, no duplicate option
store or unrelated Buck2 utility port.

## Evidence and validation

Update the protected `apple_common` reflection and unsupported-member
test only for the newly admitted method. Add a focused Bzl Loading test
for all five values, truth/type/rendering/`to_string`, pairwise
equality/hash, unsupported string/type refusal, and survival after
freezing/dropping globals. Use the pinned source and upstream tests as
the accepted discriminating evidence; no new oracle fixture is needed
for the finite literals. Rebuild `slug_cli_v2`, check/clean stale
`slugd`, then run one bounded authentic one-shot cquery and record the
first new stop. This connected replay must show the old line-400 stop
absent; it does not prove provider or repository parity.

Use JSON `--no-run`, exact-name preflight and focused Loading tests with
serialized Cargo; compile the downstream CLI. Run `cargo fmt --all --
--check`, `git diff --check`, plan-status and archive checks. Independent
DESIGN and FINAL reviews gate implementation and acceptance. Update a
compact Stage 4/Stage 9 owner note only after acceptance.

Allowlist: this manifest and canonical Current; Loading
`app/slug_loading_v2/src/apple_common.rs` and a narrow sibling module
plus `lib.rs` only if needed; compact Stage 4/Stage 9 owner notes after
acceptance. No configuration, Analysis, Bzlmod, Core, provider,
repository generator, remote-cache/action or fixture edit. `REPLAN` if
the authentic source passes a value outside these five before the next
observable stop, if value truth/rendering/equality cannot be represented
without a broader parser, or if provider construction is required to
validate this finite constructor. Ordinary compile/test failures remain
within this packet.

## Immediate predecessor

WP849 at `f69efff42` accepted the structurally configuration-backed
default Apple fragment with independent DESIGN/FINAL review. The rebuilt
authentic cquery passed its former `ctx.fragments.apple` stop and first
stopped at the separate `apple_common.dotted_version` call. WP848 at
`4a57c89fe` accepted the finite built-in Xcode extension definition
source. Non-null Apple options, Xcode provider/repository semantics and
M7A closure remain open.

## Acceptance receipt

Independent DESIGN and FINAL reviews returned ACCEPT. JSON `--no-run`
compiled the Loading lib tests; exact-name preflight selected the four
`apple_common::tests`, all 4/4 passed. The downstream `slug_cli_v2` build,
`cargo fmt --all -- --check` and `git diff --check` passed. A rebuilt
authentic one-shot cquery exited 2 at the first new stop,
`xcode_config.bzl:98`: `apple_common.XcodeVersionConfig(...)` is a
declaration-only provider key and cannot yet be called. The old line-400
`dotted_version` error is absent. No stale `slugd` was present before or
after. The finite value is evaluator-local and Allocative, with no new
retained graph value, cache, interner or DICE key. Compilation and focused
testing took approximately 25 seconds, the downstream CLI build about
20 seconds and the authentic replay about 55 seconds; no performance
gain is claimed.
