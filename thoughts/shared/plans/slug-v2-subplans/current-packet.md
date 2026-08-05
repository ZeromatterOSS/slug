# Current Slug V2 Packet

Packet: `WP-6-m2-positive-string-build-setting-transition-oracle`
Milestone: M2 semantic target configuration inputs and transitions
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: Bazel 9.2 oracle-only discriminator before configuration design
Evidence: accepted recursive configured analysis and bounded cquery consumer;
parallel Terra aquery/owner audits; reserved scheduling review selecting the
first provider-visible semantic configuration input instead of a speculative
general checksum representation; stopped full fixture proving that Bazel's
invalid-transition diagnostic exposes the unavailable checksum; reserved
positive-successor review.

Create one isolated fixture:

`tests/v2_oracle/fixtures/string-build-setting-transition/`

The fixture must use Bazel 9.2.0 and a root-repository string build setting,
custom provider, consumer rule, and user transition. Observe configured values
only through `cquery --output=starlark --starlark:file=...`; formatter output
must contain provider values and stable labels but no configuration checksum,
mnemonic, output path, action key, or platform.

Required rows in one retained Bazel server:

1. direct consumer with the build-setting default;
2. direct consumer with an explicit `--//:setting=<value>` command input;
3. a parent whose two dependency edges transition the same child target to
   distinct string values, proving distinct outgoing configured analyses;
4. unchanged warm replay of the parent;
5. a transition implementation edit changing one outgoing value;
6. restoration of that transition value;
7. a build-setting default edit observed without a command override; and
8. restoration of the default plus successful same-server replay.

The exact fixture may add only:

- `fixture.toml` with Bazel 9.2.0 commit/source provenance and generation plus
  verification commands;
- `workspace/MODULE.bazel`;
- root-package BUILD/Starlark sources and one cquery formatter file; and
- generated `expected/oracle.json`.

Cap the fixture at eight regular files, zero links, 450 non-generated text
lines, and twelve commands. Do not edit an existing fixture, harness, Rust,
Cargo manifest, lockfile, server schema, or command implementation. Scheduling
documents may change only when the packet reaches terminal review.

Mandatory evidence:

- generate with `/usr/bin/bazel` 9.2.0 through the existing oracle runner and
  then pass a no-update replay;
- exact successful exit/stdout/stderr plus mutation and manifest recording for
  every row;
- prove command input, two distinct transitioned dependency values, warm
  stability, transition edit/restoration, and default edit/restoration without
  matching or printing a configuration ID;
- pin Bazel 9.2 `BuildOptions`, Starlark build-setting, transition application,
  configured-target-key, and cquery formatter source anchors;
- run fixture list/validation, the focused oracle runner tests, JSON checks,
  file/link/line/cap inventory, provenance and credential-pattern scans,
  `scripts/v2_archive_status.sh`, and `git diff --check`; and
- obtain independent latest-diff fixture review before commit.

Invalid or missing transition programs and every configured-analysis failure
diagnostic are explicitly deferred because Bazel names the configured edge with
its checksum. Do not add, normalize, redact, regex-remove, or wildcard such a
row. Stop and `REPLAN` if any successful discriminator requires default/label
cquery output, configuration checksum text, a hard-coded platform or output
path, native option-universe modeling, exec/host/split/repository transitions,
select or config-setting breadth, toolchain/action execution, REAPI, a second
configured graph, an existing fixture edit, or any Rust change. This packet
authorizes no configuration implementation; its accepted result must first
drive a bounded design packet, and failure semantics remain gated on general
configuration identity.
