# Current Slug V2 Work Packet

Packet: WP-7-10-m7a-num-traits-build-script-env-aquery-r1
Status: result ACCEPT; configured action environment observed, inherited wrapper state unknown

## Question and frozen boundary

Classify whether the selected CLI root's `num-traits 0.2.19` build-script
run action passes a rustc wrapper into the `autocfg` probe whose stdout is
inherited. The previous source audit accepted that the script/helper itself
constructs only bare `has_total_cmp`, while the inherited probe-child path
remains opaque. This packet observes one configured action environment. It
cannot inspect generated `_bs.flags` bytes, execute a build script, compile,
change features or admit M7A.

Freeze clean main `a8a7adbc7`, `Cargo.lock` SHA-256
`a19882e78b50a82ed900fce570f4a6aee778553448790eaf08e2e08a591f76d8`,
`Cargo.Bazel.lock` SHA-256
`6335564ccbb3c915d0a2fa23b8aec0d69986911e062b586e13c5ec9812c2f35e`,
`MODULE.bazel` SHA-256
`109585b7b40d274ea912d32c73f54c25cc4bef1bc5e60526e18ac9bfe48d210c`,
and accepted source audit SHA-256
`0befd1d6b4187984859ee3a07769a50ff314f092217d8b1d303f5bc14251bb17`.
The accepted configured cquery contains
`@@rules_rust++crate+slug_crates__num-traits-0.2.19//:_bs` in the root
closure. The generated BUILD names its `cargo_build_script` `_bs` and declares
`_bs.flags` as output. The prior five-target aquery action argument references
that exact generated file.

## One bounded analysis-only command

Use pinned Bazel 9.2.0 SHA-256
`7668a95db1250f12c40407251e4e203b4ec8bf39bc495d2f485b2d8c99048694`
in batch mode with reused
`/tmp/slug-m7a-configured-offline-cquery-output-base`, verified nightly
six-file distdir, nightly toolchain channel, download disabled, lockfile
error mode and the established network-profiler disable flag. Run one
CLI-root `aquery` of
`mnemonic("CargoBuildScriptRun", outputs(".*num-traits-0.2.19/_bs.flags", deps(//app/slug_cli_v2:slug)))`
with `--output=jsonproto`. Filter saved output by the exact canonical `_bs`
owner and its non-tool target configuration. The output regex is a bounded
candidate selector; extra matches do not count.

A supervisor caps wall time at 15 seconds and stdout/stderr at 16 MiB each,
uses a new process group with TERM then two-second KILL cleanup, and saves
raw output plus a receipt with command, exit/stop, hashes/bytes,
configuration/owner, frozen file hashes and tracked before/after status.
One Bazel spawn only; failure, cap, ambiguity or hidden environment stops
this packet without retry. No build or test action may execute.

On exit 0, require exactly one `CargoBuildScriptRun` action for canonical
`@@rules_rust++crate+slug_crates__num-traits-0.2.19//:_bs` in the non-tool
root configuration. Record its full configuration checksum, aquery-visible fixed action
environment keys and `RUSTC`, `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER` and
`CARGO_ENCODED_RUSTFLAGS` values or absence, plus command arguments.
Distinguish aquery-visible fixed environment from inherited/default shell values;
if the latter cannot be resolved, report the wrapper state as unknown.
Even confirmed wrapper absence does not alone prove actual rustc probe stdout
or `_bs.flags` content. A separate narrowly reviewed observation would be
needed for that claim.

The tracked allowlist is this manifest, canonical plan status, Stage 10 and
bootstrap readiness. Independent design review precedes the command;
independent result review checks raw owner/configuration/environment mapping.
Other generated flags, execution, compilation and M7A stay open.
Independent design review `ACCEPT` confirmed the mnemonic/output owner
filter and bounds. Bazel aquery's environment dump may omit inherited
default-shell values; the packet's unknown classification is required when
a wrapper key is absent from that dump.

## Accepted result

The sole Bazel action query exited 0 in 4.154 seconds, executed no action and
left tracked inputs unchanged. It returned two `CargoBuildScriptRun` actions:
the exact canonical `_bs` owner in the non-tool root configuration (full
checksum `72a5446837369cd26452deae28ca8b1789d1ab4a7c21b1e160e1632e5f6b4b66`)
and one separate tool-configuration owner. The selected action names the
expected `--flags_out=.../num-traits-0.2.19/_bs.flags`. Its aquery-visible
fixed environment includes `RUSTC` and `CARGO_ENCODED_RUSTFLAGS`, but neither
wrapper key. Some visible keys, including `PATH`, can derive from the default
shell environment. Inherited wrapper values and generated `_bs.flags` bytes
remain unknown. Receipt SHA-256 is
`ffba1990df9f5a6051cf180859f562a949f632d9949851abd0461570e12696dd`;
parsed analysis SHA-256 is
`b3e727f268ad66c7ae9c6ba7cd90c067d5ee041a5d69054bc29f2f3375240ae4`.
Independent final review `ACCEPT` confirmed the owner, configuration, bound,
clean status and narrow conclusion. This packet does not establish probe stdout,
complete compiler flags, compilation or the first M7A readiness row.
