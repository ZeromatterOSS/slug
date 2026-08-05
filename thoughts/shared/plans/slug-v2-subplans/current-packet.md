# Current Slug V2 Packet

Packet: `WP-6-m2-host-conversion-inputs-schema-implementation`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: config-only, producer-free immutable Arc-backed
`HostConversionInputs` schema.

## Goal

Implement the accepted configuration schema only. Core remains the future owner
and producer of process, request, and per-occurrence observations.

## Required implementation

Add `native/host.rs`, expose it through `native/mod.rs`, and define no
producer or converter. The schema must contain:

- optional `AutoCpuToken`, exactly the 15 accepted Bazel auto-CPU output
  values when demanded: `darwin_x86_64`, `darwin_arm64`, `freebsd`, `openbsd`,
  `x64_windows`, `arm64_windows`, `piii`, `k8`, `ppc`, `arm`, `aarch64`,
  `s390x`, `mips64`, `riscv64`, and `unknown`;
- optional Unix/Windows `HostPathFlavor` when path conversion demands it;
- optional `HostCapacity { host_cpus: i32, host_ram_mib: i32 }` containing the
  post-ceiling values when a HOST resource expression demands either source;
- strictly occurrence-ordered, unique
  `HomeFact { occurrence: u32, home: CompactString }`; and
- raw-UTF-16, sorted/deduplicated Windows option-path facts, with
  `Resolved(Arc<[u16]>)` structurally distinct from the fallback outcome.

The exact aggregate data fields are the three optional scalar facts plus
`Arc<[HomeFact]>` and `Arc<[WindowsOptionPathFact]>`; each Windows fact owns
one shared raw `Arc<[u16]>` and
`WindowsOptionPathOutcome::{Resolved(Arc<[u16]>), IOExceptionFallback}`.
`HostConversionInputs` is a one-Arc wrapper around that immutable data and
exposes read-only accessors for later pure converters. It and its leaves have
structural `Eq`, `Ord`, `Hash`, and `Allocative`; `Dupe` is permitted only for
Arc-backed wrappers. Do not introduce maps, caches, interners, raw source
copies, or any source error representation. Local constructors must reject
out-of-order/duplicate home occurrences and out-of-order/duplicate Windows raw
spellings.

## Tests

Use inline tests in `native/host.rs` or `native/tests.rs` where clearer. Cover
all 15 CPU tokens, both path flavors, full-range `i32` capacity storage,
accepted and rejected home ordering, raw UTF-16 ordering/deduplication including
unpaired code units, direct out-of-order and duplicate Windows rejection,
distinct resolved/fallback outcomes, and aggregate structural equality/order
changes.

## Preconditions

`WP-6-m2-host-input-lifetime-partition-design` is ACCEPT. Its source-error and
lifetime rules are binding. Reuse only existing `allocative`, `compact_str`,
and `dupe` dependencies.

## Allowed paths

- `app/slug_configuration_v2/src/native/host.rs`
- `app/slug_configuration_v2/src/native/mod.rs`
- `app/slug_configuration_v2/src/native/tests.rs` only if tests are not inline
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`

## Stop conditions

Do not add Host I/O, environment/process access, lazy cells, process ownership,
daemon scanning, workspace outcomes, DICE, option conversion, command or
configured-target activation, cross-crate dependencies, Cargo changes,
fixtures, or generated artifacts. If the schema cannot remain configuration-only
and producer-free, stop and REPLAN before widening scope.

## Validation

Run `cargo fmt --all -- --check`, `cargo test -p slug_configuration_v2`, and
`cargo check -p slug_configuration_v2`, plus the Stage 6 GNU-Windows no-run
check when it covers this crate. Inspect final diff, allowlist, caps, and
Cargo/dependency status. Do not run daemon, oracle, Bazel, or configured-target
tests.

## Completion

Complete only when the schema validates ordered facts and has the specified
traits and discriminators. Later serial work is core process-owner/capture with
exact source errors, core request pre-scan/fresh projection, then configuration
converters. A mandatory REPLAN precedes configured-target or command
activation; configured-target cycle deferral remains in force.

## Diff budget

- Production Rust: at most 240 net lines.
- Test Rust: at most 340 net lines.
- Total net change, including terminal records: at most 620 lines.
- No Cargo, dependency, fixture, generated, baseline, or unrelated changes.
