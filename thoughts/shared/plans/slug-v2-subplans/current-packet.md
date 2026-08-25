# Current Slug V2 Packet

Packet: `WP-6-7A-generated-repository-package-publication-frontier-audit-2`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design base: promotion accepted 2026-08-24 / Rust `b42b004c`

Result: read-only audit over Rust `b42b004c` deciding the smallest next owner
toward exact generated-repository package source/load — now that core can
construct a routed public `RootRepositoryRoute` for extension-generated
repositories and drive the existing REPO-file/ignore/deleted-package/
BUILD-order package owners unchanged. Linux under WSL is the only platform
target; Windows/macOS remain deferred.

## Required audit decisions

Rust is read-only. The audit may change only scheduling/plan documents. It must:

1. Correct the stale absence scan: `selected_repo_spec.rs`'s lifecycle proof
   still requires that loading's `bzl_module.rs`, core's
   `generated_repository_definition.rs` and `root_apparent_repository_definition.rs`
   contain none of the root-mapping observation names, but commit `2022a7a2`
   legitimately added those names to core's generated_repository_definition.rs
   when it promoted the apparent-mapping owner. Schedule one docs-only or
   tightly-bounded test-only correction packet (allowlist: only the scan block)
   updating the assertion to the accepted composition state.
2. Choose the smallest consumer owner that activates the Generated route:
   which existing command/loading caller first constructs a generated route
   from core's accepted view, with what request/revision behavior; or a formal
   REPLAN if activation requires an unbounded prerequisite.
3. Classify each surface exact / Slug-native / unsupported-deferred.

STOP any Rust edit beyond the named scan-block correction, new key/carrier/
adapter/caller outside the chosen owner, public/crate-root exposure,
fixture/oracle growth, milestone closure, M8/M7B or exact identity work.
REPLAN before widening or baseline drift.

After ACCEPT, schedule exactly one successor. M7 remains partial and
M7A -> M8 -> M7B remains.
