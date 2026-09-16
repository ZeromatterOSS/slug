# Current Slug V2 Work Packet

Packet: WP-7-10-m7a-external-tokio-feature-probe-r1
Status: design ACCEPT; one bounded Cargo unit-graph observation pending

## Question and frozen inputs

Determine whether the frozen Cargo metadata's `tokio 1.53.1` feature
`windows-sys` is actually an enabled feature of the selected Linux CLI
production compilation unit. This is a read-only classification packet; it
cannot edit external crate rules, repin the lock, compile, test or admit M7A.

Freeze main `1998461eb`, `Cargo.lock` SHA-256
`a19882e78b50a82ed900fce570f4a6aee778553448790eaf08e2e08a591f76d8`,
`Cargo.Bazel.lock` SHA-256
`6335564ccbb3c915d0a2fa23b8aec0d69986911e062b586e13c5ec9812c2f35e`,
`MODULE.bazel` SHA-256
`109585b7b40d274ea912d32c73f54c25cc4bef1bc5e60526e18ac9bfe48d210c`,
and frozen selected-Linux Cargo analysis SHA-256
`dad20c2240aa421f5d99fe30190cbe7fa9035b4e388ad64aa30e762afaf9cd10`.
The prior packet accepted exact local rule feature attributes against that
snapshot, not external effective feature parity or compilation.

Static comparison of 310 selected external Cargo metadata package feature
sets with generated `Cargo.Bazel.lock` gives 303 exact common sets. Six of
seven differences are present in the lock's `x86_64-unknown-linux-gnu`
feature select: bitflags `std`, errno `default`, lalrpop-util `lexer,regex`,
libc `extra_traits`, phf_shared `default` and rustix `termios`. The seventh,
Tokio `windows-sys`, is in Cargo metadata but only the lock's Windows
select. Tokio's cached Cargo.toml lists `windows-sys/Win32_*` in `net`,
`process` and `signal` feature definitions while declaring the optional
`windows-sys` dependency under `cfg(windows)`. Metadata resolution may not
represent the effective Linux compiler feature flags for this one unit.

## One bounded observation

Use the pinned local Cargo 1.91.0-nightly binary and matching rustc for one
`cargo build --unit-graph` invocation with exact selection
`--manifest-path app/slug_cli_v2/Cargo.toml --bin slug --target
x86_64-unknown-linux-gnu --frozen -Z unstable-options` and an isolated
temporary target directory. Cargo's `--unit-graph` prints a JSON graph
without invoking build actions. A supervisor starts a new process group,
caps stdout and stderr at 16 MiB each and wall time at 10 seconds, sends
TERM on a cap, gives two seconds to drain, then sends KILL to the group if
needed. Save raw stdout/stderr and a receipt with the exact command,
exit/stop reason, elapsed time, byte counts, hashes, tracked before/after
status and preflight hashes. Do not retry if the unit graph is unsupported,
needs unavailable inputs or hits a cap.

On exit 0, parse only Tokio's normal target-platform library unit(s),
excluding host/build/test units. Compare that selected Linux feature set to
the frozen Cargo metadata and the lock's common plus Linux select set.
Report whether `windows-sys` is present, absent or not classifiable, and
identify any other observed difference without claiming whole-closure
parity. If the target-kind/platform identification or feature set is not
unambiguous, stop and select a narrower successor. No BUILD or lock edit is
authorized by this packet.

The tracked allowlist is this manifest, canonical plan status, Stage 10 and
bootstrap readiness. Independent design review precedes the one command;
independent result review checks the raw unit row and scoped inference.
Generated inputs, configured actions, compilation, external feature parity
and the first M7A readiness row stay open.
Independent design review first requested exact binary selection, finite
stream/process cleanup and target-platform unit filtering. The corrected
design was independently re-reviewed `ACCEPT` with those controls.
