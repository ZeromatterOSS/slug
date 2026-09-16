# Current Slug V2 Work Packet

Packet: WP-7-10-m7a-generated-lock-candidate-validation-r1
Status: restored-authority, no-repin validation design ACCEPT; one check pending

## Outcome and frozen evidence

Decide whether the generated `Cargo.Bazel.lock` candidate is valid against the
exact restored root `Cargo.lock`, without repinning, changing source or
running any build, test or graph query. Only a validated generated-lock
candidate may join the seven static BUILD edits in the next final review.
This packet admits no Bazel declared/configured reachability, action behavior,
exact ActionKey or M7A milestone.

Freeze main `a64259308`; root `Cargo.lock` SHA-256
`a19882e78b50a82ed900fce570f4a6aee778553448790eaf08e2e08a591f76d8`,
`MODULE.bazel.lock` SHA-256
`bcdda78dd82aab5fd7ab1796985a78322b50178dceed7669c1b20173c2634302`,
generated `Cargo.Bazel.lock` candidate SHA-256
`6335564ccbb3c915d0a2fa23b8aec0d69986911e062b586e13c5ec9812c2f35e`,
and the first repin/recovery receipt SHA-256 values
`86995d8a436d74b7db448d5fdbb3aa652a30c7f3ab50477036a3f6dca37b1051`
and `2322f3a8481e3e7393ca0d81f0572153f979a0a5cf2e8d4c44da73aff4806cfb`.
The profiler recovery's pinned Bazel 9.2.0 command exited 0 in 6.722 seconds,
but rules_rust added exactly two blank lines to `Cargo.lock`'s unused-patch
section. Its changed byte hash violated that packet's explicit gate, so
independent review returned `REPLAN`. Exact original bytes were restored
from the pre-run copy. The changed copy remains at
`/tmp/slug-m7a-repin-mutated-Cargo.lock`, SHA-256
`e89b952a48620d1e2096b54c0221e02eb7167f4364c0f7d3e16d98589b8abe17`.
The two TOML documents parse identically; package versions, sources and
checksums did not change.

The structured candidate analysis at
`/tmp/slug-m7a-generated-lock-analysis.json`, SHA-256
`19b365fff8230f71021edf660901769d8a940744879dc2f5d6ac5486b1325997`,
records 35/35 selected local and 310/310 selected external lock keys, exactly
nine additive crate keys (eight selected external and `slug_starlark_v2`),
no removals or existing external-crate drift, and all 448 registry archive
checksums equal to root Cargo.lock. Six changed existing lock entries are
local crates whose added dependencies/features match current Cargo metadata;
`MODULE.bazel.lock` is unchanged. These static facts preserve the candidate,
but do not alone prove the generator will accept it under restored inputs.

## Single validation

First recheck every frozen hash, the exact seven BUILD edits, parsed-TOML
equality, all 448 registry checksum mappings and the structural diff above.
Assert that `/tmp/slug-m7a-lock-validation-output-base` does not exist.
Use pinned Bazel 9.2.0 SHA-256
`7668a95db1250f12c40407251e4e203b4ec8bf39bc495d2f485b2d8c99048694`,
at `/home/wgray/.cache/bazelisk/downloads/sha256/7668a95db1250f12c40407251e4e203b4ec8bf39bc495d2f485b2d8c99048694/bin/bazel`,
rules_rust 0.73.0, and the verified host Cargo cache. Run one no-repin
`mod deps` validation with startup flags
`--batch --ignore_all_rc_files
--output_base=/tmp/slug-m7a-lock-validation-output-base` and command flags
`--repository_disable_download --lockfile_mode=off
--noexperimental_collect_system_network_usage`. Leave
`CARGO_BAZEL_REPIN`, `REPIN` and generator URL/SHA overrides unset. Set
`CARGO_BAZEL_ISOLATED=false`, `CARGO_HOME=/home/wgray/.cargo`,
`CARGO_NET_OFFLINE=true` and put
`/home/wgray/.rustup/toolchains/nightly-2025-09-14-x86_64-unknown-linux-gnu/bin`
first in `PATH`.
Keep the restricted network environment.

A fresh output base prevents Skyframe/repository reuse, while Bazel
`--lockfile_mode=off` skips `MODULE.bazel.lock` extension reuse. Pinned
rules_rust's `determine_repin` calls cargo-bazel `query` when repin is unset
and the generated lock exists. Its digest hashes parsed context, config,
manifest metadata and tool versions rather than raw Cargo.lock bytes; that
is why parsed-TOML equality and checksum census remain separate gates.
This command validates generated-lock coherence only, not target graph
reachability. The one attempt has a 30-second TERM/three-second KILL ceiling
and 16 MiB per-stream cap. Record executable/hash, exact arguments/environment,
fresh-base assertion, output hashes, elapsed time, exit, process cleanup and
pre/post tracked-file hashes in a machine-readable `/tmp` receipt. A cache
miss, failure, timeout, unexpected write or invalid cleanup ends this path
without automatic retry, repin, network fetch or longer ceiling.

## Result gate

Success requires Bazel exit 0, no stream truncation, exact unchanged hashes
for root Cargo.lock, Cargo.Bazel.lock, MODULE.bazel.lock and the seven BUILD
files, and only the new `/tmp` output base outside the repository.
No other tracked file may change. Independently review the complete receipt
and candidate analysis before accepting static graph/lock synchronization.
Run `git diff --check` and `python3 scripts/v2_plan_status.py`.
No Cargo/Bazel build, test, Bazel target query, F3, developer-gate sweep or
BuildBuddy replay is selected. Fresh declared/configured/action reachability
and compilation remain separate M7A prerequisites.
Independent design review `ACCEPT` verified the fresh-base/lockfile-off
extension path, the no-repin cargo-bazel query branch and the separate
Cargo-lock semantic/checksum gates. The single bounded check is selected.
