# Current Slug V2 Packet

Packet: `WP-4-5-6-generated-source-oracle-tool-specific-message-shape-correction-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: `04-starlark-loading-and-build-packages.md`,
`05-bzlmod-and-repository-graph.md` and
`06-analysis-toolchains-and-actions.md`
Base: retained generated-effect handoff candidate and independently accepted
terminal harness correction design on 2026-08-25

Result: make the existing Bazel 9.2 generated-source fixture discriminate the
same successful semantic boundary through each tool's admitted presentation,
without changing Rust, oracle evidence or fixture workspace bytes.

## Retained Rust acceptance and freeze

The prior implementation retry is structurally accepted but formally
`REPLAN`ned because the mandatory fixture harness exits nonzero. Freeze its
seven files at these exact final values:

| Path | Lines | SHA-256 |
|---|---:|---|
| `app/slug_bzlmod_v2/src/host_module.rs` | 4,872 | `56a7ffe34f8f26c3e70b02deed12268198599060cc455127f2edd3bddab22506` |
| `app/slug_bzlmod_v2/src/source_preparation.rs` | 16,954 | `266a10a29e308161139e97e683e99225dfbad3959dee04b4f1539d12685f7661` |
| `app/slug_bzlmod_v2/src/host_package.rs` | 5,009 | `1921abc6f0fedc0f7c0d14504168980f1063deec82bcfff9b64c2c3c6b8cc5b8` |
| `app/slug_core_v2/src/runtime/generated_repository_definition.rs` | 4,083 | `a87b856c9bc8b279d134f01229cb3bc240f451f41c8741beefffb7aca7df3566` |
| `app/slug_core_v2/src/runtime/root_apparent_repository_definition.rs` | 1,735 | `033d545f96e571f1fcfb628ec7c9e90813f6662a5623a5e12ed5c6fa0fc256e1` |
| `app/slug_core_v2/src/runtime/generated_package_route.rs` | 996 | `4d8e6de9d88f0172dc85d242b3a226406c3fda2b805206183d7702b4831d76a8` |
| `app/slug_core_v2/src/runtime/repository_io.rs` | 6,140 | `76f03638d41d5f901b762a0e627cd05290f350fcfcd04e28caaa2e708e94ec9c` |

The retry itself is +21/-1 production and +646 proof, +667/-1 aggregate.
Conservative cumulative accounting is within 1,500 production, 1,250 proof and
2,750 aggregate. Full Bzlmod and loading suites pass; full core has only its
recorded generic-vs-wrong-kind query diagnostic baseline. The rebuilt Slug
command exits zero with exact terminal:

`{"success":true,"command":"build","target_count":1,"loaded_package_count":1,"analyzed_target_count":0,"declared_action_count":0,"runtime_mode":"one-shot","completed_boundary":"dice_exported_source_file"}`

## Exact write authority and caps

Only these four paths are writable:

| Path | Entry lines | Entry SHA-256 | Authority | Physical ceiling |
|---|---:|---|---|---:|
| `tools/v2_oracle_lib/fixture.py` | 539 | `850a257294a7cd091a95d62b92ad4ac146c5e9a3ef352bb3e202332f5ca60a74` | command shape representation/parser | 575 |
| `tools/v2_oracle_lib/compare.py` | 155 | `6d2195c3e2fc641ac4d29b8385e9a3545ecd7970e7a6281050ca62466a368586` | matching-tool selection/fail-closed comparison | 205 |
| `tests/v2_oracle/test_v2_oracle.py` | 2,650 | `c6d3f4b68809880426ed145a626bb706c0d73820f4112fd012833882aba6e718` | focused parser/comparator proof only | 2,770 |
| `tests/v2_oracle/fixtures/module-extension-use-repo/fixture.toml` | 29 | `2bf8b8fd1ea0effa1a2628f6ac8f1ee7b4132e1871beb4b127492de0e0ae8b48` | migrate message-shape assertions only | 40 |

Caps are <=70 production, <=120 proof and <=190 aggregate added lines. Every
other file is retained and non-writable, including all Rust, fixture workspace
files and `expected/oracle.json`.

## Required correction

Extend `FixtureCommand` with narrow Bazel- and Slug-specific stdout/stderr
contains and regex-pattern fields while preserving the existing fields as
common assertions. Parse every field with the existing strict string-list
grammar.

Comparison must apply common assertions plus only the actual tool's assertions.
If any tool-specific assertion is present, both the Bazel and Slug contracts
must each contain at least one assertion; reject a half-specified parsed fixture
and fail closed for a programmatically constructed half-contract. Unknown tool
values must not select either contract.

In `module-extension-use-repo`, move the existing canonical generated target,
source-file classification and successful-completion assertions unchanged into
the Bazel-specific fields. Keep `expected_exit = 0` common. Add one anchored
Slug-specific stderr regex matching exactly the accepted one-shot success JSON
and optional final newline. Do not weaken either tool contract into a shared
substring and do not compare Slug presentation bytes with Bazel oracle bytes.

## Validation and compatibility

Add focused proof that Bazel sees only common+Bazel assertions, Slug sees only
common+Slug assertions, cross-tool text fails its selected contract, a missing
selected contract fails closed, and parser half-contracts are rejected. Run
the focused tests, then the full V2 oracle test module. Replay the unchanged
fixture once with pinned Bazel 9.2 and once with the rebuilt Slug binary; both
harness invocations must exit zero. Run Python formatting/lint used by the
repository if present, archive status, exact scope/hash/cap checks and diff
hygiene before independent terminal review.

The fixture's Bazel exit, canonical generated target, source-file
classification and successful completion remain **exact Bazel 9.2**. Slug's
successful generated-source boundary is exact behavior with **Slug-native**
JSON presentation. Other repository-context APIs, non-POSIX modes, public query
breadth, broader platforms and exact configuration/output bytes remain
**unsupported/deferred**.

Pinned Bazel 9.2 is behavioral authority. Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` remains concept-only guidance for
the already-frozen owner/effect architecture. It supplies no harness,
comparison, fixture or output semantics; copy no Zig code or representation.

STOP Rust, expected/oracle JSON, workspace, argv, exit contract, fixture
generation, normalization, unrelated comparison mode, generic tool-map API,
third tool, credential access, Java/JVM, M7 closure, M8/M7B or identity-byte
work. `REPLAN` before a fifth file, cap excess or any semantic widening.
