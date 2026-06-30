<div class="title-block" style="text-align: center;" align="center">

# Slug

**Bazel-compatible build-tool restart in Rust**

![Status] ![License]

[Status]:
  https://img.shields.io/badge/status-pre--alpha-orange.svg
[License]:
  https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blueviolet.svg

---

</div>

Slug is a Bazel 9+ compatibility project. The active repository root is the V2
clean restart: a small Rust crate set, a Bazel oracle fixture harness, DICE and
Starlark infrastructure, and stage-owned plans for rebuilding behavior from
Bazel source and tests.

## Why Slug?

Slug V2 is a research project to build a Bazel-shaped Rust implementation from
the first architectural boundary. It uses:

- **Bazel source and tests** as the compliance oracle.
- **DICE** for semantic build state that must be cached, invalidated, replayed,
  or shared across requests.
- **starlark-rust** for the Starlark substrate.
- **REAPI-first execution** for future local and remote execution paths.

## Status

Slug is in **pre-alpha**. It is under active development and not yet suitable for
production use. APIs, CLI flags, and behaviors may change without notice.
The project is provided for educational and research purposes, and is in large
part an exercise in experimenting with agentic programming on a substantial
systems codebase.

The canonical roadmap is the V2 clean restart plan:
[thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md](thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md).
The V1 implementation is preserved by the `slug-v1-archive` tag and
`v1-archive` branch recorded in [V1_ARCHIVE.md](V1_ARCHIVE.md). New work should
target the V2 plan and the subplans under
[thoughts/shared/plans/slug-v2-subplans](thoughts/shared/plans/slug-v2-subplans/).

### Active Surface

- Root orientation docs: `AGENTS.md`, this README, `V1_ARCHIVE.md`, and
  repo-local skills under `.codex/skills/`.
- V2 plans and prompts under `thoughts/shared/`.
- Stage 1 oracle harness under `tools/v2_oracle*` and `tests/v2_oracle/`.
- V2 Rust crates under `app/slug_*_v2/`.
- Retained infrastructure crates: `dice`, `starlark-rust`, `superconsole`,
  `allocative`, `gazebo`, `pagable`, and `shed`.

V1 source, tests, root Bazel/Buck metadata, old CI, and old plans are archive
material. Inspect them through the archive refs when a V2 subplan names an
extraction target.

## Installing

Slug V2 is currently a development scaffold. Use the checked-in Rust toolchain
and build the V2 CLI crate:

```bash
git clone https://github.com/ZeromatterOSS/slug.git
cd slug
cargo build -p slug_cli_v2
```

The debug binary is `target/debug/slug` unless `CARGO_TARGET_DIR` is set.

## Quick start

List oracle fixtures and run the CLI identity smoke:

```bash
python3 -B -m tools.v2_oracle list
cargo run -p slug_cli_v2 -- version
```

## Credits

Slug is developed by Zeromatter Inc, with primary authorship by Walter Gray
([walter-zeromatter](https://github.com/walter-zeromatter) /
[yeswalrus](https://github.com/yeswalrus)).

Slug is a fork of [Buck2](https://github.com/facebook/buck2) by Meta Platforms,
Inc. The DICE incremental computation engine, starlark-rust interpreter,
superconsole terminal UI, and remote execution architecture originate from the
Buck2 project. We're grateful for Meta's decision to open-source Buck2 under a
permissive license.

## License

Slug is licensed under both the MIT license and Apache-2.0 license; the exact
terms can be found in the [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE) files, respectively.
