# Current Slug V2 Packet

Packet: `WP-5-7A-selected-registry-bcr-plan-local-archive-split-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: selected BCR RepoSpec/archive-plan boundary and sole repository
materializer
Base: `17c11505`

Result: relocate the accepted local archive owner/proof unchanged and add one
exact, inactive selected-BCR archive plan. Prove mutual exclusion and return an
explicit deferred-transport terminal for BCR. Do not implement network,
decompression or BCR filesystem effects.

## Architecture and exact plans

Add private `repository_archive.rs` with one `ArchivePlan` enum and sole
`parse_archive_plan`. It owns complete shape validation before physical work:

- `LocalTar`: preserve the accepted exact one-`file://`, hex-`sha256`,
  `type = "tar"`, optional safe `strip_prefix` shape and every existing
  diagnostic/byte/path behavior.
- `SelectedBcrTarGz`: require the producer's exact complete shape:
  nonempty ordered HTTPS `urls`; SHA-256 `integrity` SRI;
  `type = "tar.gz"`; empty-string `strip_prefix`; empty
  `remote_patches`, `remote_file_urls`, `remote_file_integrity` maps;
  `remote_patch_strip = Int(0)`; exactly one HTTPS
  `remote_module_file_urls` entry; SHA-256
  `remote_module_file_integrity` SRI; no missing, wrongly typed or extra
  attribute.

The parser first classifies only exact attribute-key sets; it must not partially
parse one shape and fall through to the other. Decode each SRI to exactly 32
bytes and retain URL order/complete MODULE facts in an immutable private plan.
No physical path, runtime, generation or transport capability enters it.

Relocate the contiguous existing archive production block and archive-specific
proof from `repository_io.rs` into the new module/test file. Preserve function
bodies and tests mechanically except narrow visibility/import/path adjustments.
The sole native and dormant repository dispatches call the new plan owner.
Local plans execute the unchanged materializer. A valid BCR plan returns one
generation-scoped `TransportError` stating that selected-registry BCR transport
is deferred; malformed BCR remains its stable parser `SpecError`. This is an
honest unsupported boundary, not a parity claim.

Pinned Bazel 9.2 commit `8220c619…` owns exact fields. Reuse
`selected_repo_spec.rs:857-889` and its direct `type = "tar.gz"` assertion as
accepted producer evidence. Pinned `../zabel` `c7298478…` guides only keeping
the producer-owned semantic view above physical realization and joining no root
until realization succeeds. Copy no Zabel code or behavior.

## Exact file/dependency authority

Modify exactly:

| File | Entry lines | Entry SHA-256 | Ceiling |
|------|------------:|---------------|--------:|
| `Cargo.lock` | 4,875 | `ee9acebd876bedaf474e28c5f14894aa7dec7afb257e2de4b2da903dd8c39800` | 4,880 |
| `app/slug_core_v2/Cargo.toml` | 45 | `6e91459a3b014d5c43a0be92c184448563cd4d71c34aaf92b05479d1f2bd6169` | 48 |
| `app/slug_core_v2/src/runtime/mod.rs` | 332 | `204fd7510b216b9794b6ce646c29ab30dcf2b453bb42c2b402a76da6f41ac651` | 336 |
| `app/slug_core_v2/src/runtime/repository_io.rs` | 6,140 | `76f03638d41d5f901b762a0e627cd05290f350fcfcd04e28caaa2e708e94ec9c` | 5,000 |
| `app/slug_core_v2/src/runtime/repository_archive.rs` | absent | absent | 1,050 |
| `app/slug_core_v2/src/runtime/tests/repository_archive_tests.rs` | absent | absent | 1,700 |

Enable only workspace `base64`. It is already locked; the exact lock delta is
one `base64 0.21.7` dependency line under the `slug_core_v2` package and no
other byte. Expected candidate lock is 4,876 lines/SHA-256
`29c633ff24208244ef7b33bcf555e36d2f8703cec6471509f893bffff0621dec`.
After that exact update all Cargo commands use `--locked`.

Caps: <=300 new semantic production lines, <=400 new proof lines, <=1,900
relocated unchanged lines and <=2,600 aggregate additions. New production
helpers <=120 lines and tests <=180. Do not move Git/generated/session/dormant
cleanup unrelated to the archive block.

## Required proof

Focused proof must discriminate:

- exact required BCR key set, value types, `type = "tar.gz"`, explicit empty/
  zero fields, ordered URLs, 32-byte SRIs and one MODULE URL;
- every missing/wrong/extra BCR field, non-HTTPS URL and malformed SRI;
- local/BCR mutual exclusion with no partial fallback;
- byte-equivalent local materialization, checksum, strip, path/header/collision,
  staged-failure and cleanup tests after relocation;
- valid BCR reaches only the generation-scoped deferred `TransportError`, while
  malformed BCR reaches stable `SpecError`; neither performs DNS/network/temp-
  root/archive/module work;
- repository session failure generation, stale-token/post-attempt behavior,
  warm reuse and A/B/A remain unchanged; and
- static absence of HTTP/TLS/runtime/task/process APIs, BCR extraction and a
  second materializer in the new module.

Run `slug_core_v2` focused archive/parser/session tests, full
`slug_core_v2 --lib`, direct core compile, formatter/diff hygiene and the
locked base64 feature check serially. Rebuild `slug_cli_v2`, clean exact
`slugd`, then replay one fresh wildcard-removed rules_rust root. It must reach
the explicit deferred BCR transport terminal, proving producer-to-plan wiring;
do not claim materialization.

## Compatibility and STOP

- **Exact:** accepted local archive behavior and produced Bazel 9.2 BCR RepoSpec
  shape/order/SRI parsing.
- **Slug-native:** private plan representation and explicit typed deferred-
  transport terminal.
- **Unsupported/deferred:** all BCR network, redirect/fallback, gzip/GNU-tar,
  MODULE replacement and existing generic breadth.

STOP entry/hash/file/dependency/cap mismatch, local behavior drift, absent-type
BCR admission, parser fallback ambiguity, any BCR I/O/root, public API, DICE/
identity/session change, second materializer, registry/global provider,
subprocess/Java/JVM, fixture mutation, transport implementation, second
successor or milestone closure. `REPLAN` before widening.
