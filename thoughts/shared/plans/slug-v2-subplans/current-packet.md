# Current Slug V2 Work Packet

Packet: WP-7-10-m7a-cargo-cache-acquisition-r1
Status: acquisition and offline verification ACCEPT; Bazel graph work remains open

## Outcome and authority

Stage the exact root `Cargo.lock` dependencies needed for a later, separately
reviewed Bazel crate-universe repin. This is preparation only: it changes the
host Cargo cache, not source or repository build metadata, and admits no
package reachability, action behavior, exact ActionKey or M7A milestone.

Freeze clean main `5ee216bd8`, root `Cargo.lock` SHA-256
`a19882e78b50a82ed900fce570f4a6aee778553448790eaf08e2e08a591f76d8`,
the accepted Linux-closure receipt SHA-256
`b3886868867d6b2c6f8d1d49575fbc0f4322ab83216e1da456ae7e2f1e2c5e2`,
and the cache-gap inventory at `/tmp/slug-m7a-cache-gap.json`, SHA-256
`5c8294dcbebbefea8602f891c28ee2545ff701d572084ae236049fdd0eba3189`.
That inventory lists 448 locked registry packages and 54 missing host-cache
archive/source pairs by exact name, version and lock checksum. Sixteen missing
archives exist in Bazel's downloader cache; 38 do not. The selected Linux CLI
closure had its 309 registry packages locally, but rules_rust repin invokes a
full-workspace Cargo update and cannot rely on that smaller closure.

One read-only full-workspace preflight of pinned Cargo `metadata --locked
--offline` exited 101 in 0.15 seconds with zero stdout: `anstyle-wincon
v3.0.11` was unavailable offline. Its stderr SHA-256 is
`529cdff554a4b7edbf576fef4ff48ef9d4ec8e8f68e22e61c57dae9826c30dde`.
The previously drafted offline graph-sync packet is superseded, unaccepted and
unexecuted; no BUILD or Bazel lock file was changed. The static M7A graph gaps
remain the missing `slug_configuration_v2` BUILD owner, six normal consumer
edges, and the 34/35 local and 302/310 external selected lock-key coverage.

## Allowed work

First verify the frozen hashes, current main and gap rows against root
`Cargo.lock`. Then run exactly one pinned nightly-2025-09-14 Cargo command
from the repository root:

```text
cargo fetch --locked --manifest-path Cargo.toml
```

Use the absolute executable under
`/home/wgray/.rustup/toolchains/nightly-2025-09-14-x86_64-unknown-linux-gnu/bin/`.
Leave `--target` unset so Cargo fetches all platform dependencies recorded by
the lock; use the existing `/home/wgray/.cargo` cache. Allow outbound access
only for this locked fetch. It may populate registry archives, extracted
sources, sparse-index entries and locked Git sources in the host cache.
`--locked` and the frozen lock checksum prohibit a new version or changed
Cargo authority. Do not copy archives out of Bazel's cache into Cargo's cache
or hand-edit sparse-index entries.

The fetch has a 120-second TERM/three-second KILL ceiling and 16 MiB cap per
captured stream. Record the exact executable, environment, command, elapsed
time, exit status, stream hashes, process cleanup and pre/post tracked-file
status in a machine-readable `/tmp` receipt. A sandbox-only network denial
may be corrected by one authorized unsandboxed execution of the same locked
command; a network/download error, timeout, changed lock, unexpected tracked
file mutation or invalid cleanup stops without an automatic retry. This is
dependency acquisition, not a test. Do not run Bazel, repin, builds, tests,
F3, payload acquisition unrelated to `Cargo.lock` or any graph query.

## Verification and result

After a successful fetch, verify that the root `Cargo.lock` SHA-256 is still
frozen; all 448 registry `.crate` archives are present and hash to their
individual lock checksums; and the locked Git checkout for
`sorted_vector_map` is at commit
`84a82026bc1d1a89e7f6ec86e0e52d5479f12ccc`. Record exact counts and
any missing/mismatched inputs. Then run one full-workspace, locked, offline
`cargo metadata --manifest-path Cargo.toml --locked --offline
--format-version 1` under a ten-second TERM/two-second KILL ceiling with
captured output and cleanup. Its success is the gate for local source and
sparse-index sufficiency; compare its registry package keys to the 448 locked
keys and check extracted source directories for those keys. Do not infer that
this proves the Bazel repin or graph will succeed.

The only tracked edits allowed for this packet are its result/status in this
manifest, the canonical plan, Stage 10 and bootstrap readiness. Run
`git diff --check` and `python3 scripts/v2_plan_status.py`. Independent design
review precedes fetch; independent final review checks the receipt, lock and
cache verification before this acquisition packet closes. Failure returns
`REPLAN` with the exact missing input or environmental limit. On success,
select a new graph-sync packet for BUILD owner/edges, generated lock and fresh
Bazel evidence. That future packet must permit fresh local repository
materialization while downloads stay disabled; `--nofetch` alone can expose
stale generated repository content after a repin.

## Observed result

Independent design review returned `ACCEPT` before acquisition. The one
locked, all-target fetch completed in 3.117 seconds with exit 0, no timeout or
stream truncation, clean process-group cleanup, unchanged tracked status and
unchanged `Cargo.lock`. Fetch receipt SHA-256:
`861309ac82fcf02d8d8385b25622757ea0f4c04ffd4e568122532db0031f31b6`.
All 448 registry archives are now present with exact lock checksums; all 448
extracted source directories exist, and the locked `sorted_vector_map` Git
checkout is at the exact commit above.

The sole locked/offline full-workspace metadata command produced valid JSON
with all 448 registry keys, no extra keys and no missing sources in less than
0.224 seconds. Its 638-byte stderr contains only unused optional
`perf-event` patch warnings. The receipt wrapper then exited 1 during
postprocessing because it called `startswith` on a local workspace package's
null `source` field. The saved command stdout/stderr and the wrapper's branch
show Cargo itself exited 0; a separate read-only analysis verified the JSON,
locked keys, unchanged tree/lock and no surviving metadata process without
rerunning Cargo. Recovered metadata receipt SHA-256:
`f5163a1827114cdc71acc9c325369db93f23a3cbc807cf821203e5486fbb5bea`.
Independent final review `ACCEPT` confirmed the saved JSON and warnings,
rehash-verified all 448 archives and source directories, and verified from the
wrapper's guarded branch that Cargo exited 0 before postprocessing failed.
The wrapper defect is preserved in the receipt; no rerun, Bazel command or
test ran. This closes the acquisition prerequisite only.
