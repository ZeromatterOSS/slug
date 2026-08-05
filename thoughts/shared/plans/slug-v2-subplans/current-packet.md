# Current Slug V2 Packet

Packet: `WP-6-m2-action-query-identity-evidence`
Milestone: M2 configured action-query prerequisites
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: pin Bazel 9.2 source and isolated oracle discriminators for the four
identity owners required before action-query implementation.
Predecessor: action-query identity boundary `REPLAN`, accepted root action
closure `afd2a606`, and the protected recursive action-ownership fixture.

This is source/oracle evidence only. Do not modify the protected
`recursive-custom-rule-providers-actions` fixture. Add one isolated Bazel 9.2
fixture and use one retained Bazel server to distinguish:

- `C0 -> C1 -> C0` target-configuration and output-root identity;
- default-exec-group selected platform `P0 -> P1 -> P0`, including its
  ActionKey and structured platform field;
- FileWrite content-only ActionKey change and restoration; and
- FileWrite declared-output-path change and restoration.

Capture raw text plus Bazel's `--output=jsonproto` action graph only to identify
stable fields and equality/invalidation inputs. The fixture has exactly 18
commands: paired text/jsonproto rows for baseline `C0/P0/content-A/path-A`,
configuration `C1` and restored `C0`, selected platform `P1` and restored
`P0`, content B and restored A, and output path B and restored A. Mutations run
before the text row; its immediately following jsonproto row observes the same
state. Do not freeze one observed checksum,
path fragment, platform label, or ActionKey as a Slug algorithm. Pin Bazel
9.2 source anchors for `BuildOptions` / `BuildConfigurationValue` /
`OutputDirectories`, `StarlarkActionFactory` execution-platform selection,
`ActionKeyComputer#getKey`, and `FileWriteAction#computeKey`.

The packet may only name the serial semantic owners supported by the evidence:
complete target configuration, configured artifacts/output roots, per-action
execution platform, Bazel ActionKey, and then the aquery consumer. After this
evidence, design the general target-configuration substrate first; do not jump
directly to action-query implementation.

Scope and cap:

- one isolated Bazel 9.2 oracle fixture and its generated record;
- Stage 6 evidence/acceptance text and scheduling synchronization;
- no production or test Rust, Slug command, daemon, wire, DICE, or dependency
  change;
- at most 340 formatted authored fixture/documentation lines.

Stop and return `REPLAN` on a non-9.2 source anchor, inability to isolate the
default-platform discriminator, treating unstable identity bytes as fixed
values, modifying protected evidence, any Slug implementation or public
formatter, reuse of REAPI identity, scope beyond FileWrite and the default
execution group, execution/cache/materialization work, credential exposure,
or cap breach.
