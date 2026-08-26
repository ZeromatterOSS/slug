# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-aspect-toolchain-requirements-loading-r2`

Milestone: M7A command/ruleset bootstrap closure.

Result: admit the source-required mixed aspect toolchain list, retain canonical
label plus mandatory state structurally, freeze the complete
`rust_clippy_aspect`, and stop before the following lazy rule helper.

## Accepted starting point and source stop

Base is `5f8dd852` (`Load clippy aspect attributes`). The exact ordered 11-row
attribute map now freezes through the existing detached schema. Its unchanged
source-shaped declaration terminates at the mixed toolchain argument:

```starlark
toolchains = [
    str(Label("//rust:toolchain_type")),
    config_common.toolchain_type(
        "@bazel_tools//tools/cpp:toolchain_type",
        mandatory = False,
    ),
]
```

Freeze the complete `rust_clippy_aspect` through line 404 of selected
rules_rust 0.73.0 and stop before `_rust_clippy_rule_impl` at line 406 and
`rust_clippy = rule(...)` at line 411. The clippy source SHA-256 is
`a778d2ddc77587ffbffc72efcdaa458a1ffae0763e500da1c876b9b567b2a686`.

## Fixed behavior and architecture authorities

Behavior authority is clean Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`:

- `StarlarkRuleClassFunctions.aspect:1487-1491` consumes the shared parser;
- `parseToolchainTypes:2212-2228` preserves first label order and applies
  strictest-wins duplicate normalization;
- `parseToolchainType:2230-2257` accepts typed requirements, Labels and
  defining-thread Strings and makes Label/String entries mandatory;
- `StarlarkRuleClassFunctionsTest.testAspectAddToolchain` proves default true,
  explicit false and explicit true on a frozen aspect.

The source map has two distinct labels, so this packet does not need duplicate
normalization. Existing rule duplicate rejection remains the admitted
unsupported boundary and may not change here.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is architecture guidance only.
Its `ToolchainTypeRequirement` is declaration-owned label plus mandatory state,
its rule and aspect definitions retain the same slice type, and
`retainToolchainRequirements` detaches evaluator values. Slug adopts that
ownership shape with its own Rust values. Copy no Zig code, layout, diagnostic,
algorithm or behavior; Bazel 9.2 remains sole behavior authority.

The Stage 9 generic compact-utility row and live Buck2 utility review select
the existing `CanonicalLabel`, Boolean, immutable `Arc<[T]>` and
`Allocative`. No map, interner, cache, deep-clone path, utility import or ledger
update is warranted. The slice is retained semantic memory owned and released
with the frozen module; parser `Vec`/`SmallSet` state remains evaluator scratch.

## Decision and compatibility

Rename the rule-specific retained record to `ToolchainTypeRequirement` and use
its existing canonical-label/mandatory fields and accessors for both rules and
aspects. Rule transient/frozen/package storage remains the same immutable Arc
slice. Replace the aspect's `Option<CanonicalLabel>` with
`Arc<[ToolchainTypeRequirement]>` in transient and frozen definitions.

Rename the evaluator-aware rule parser into the shared declaration parser.
Change the aspect binding from `UnpackList<&str>` to an evaluator value and
reuse that parser. Distinct String, Label and typed entries preserve source
order; String resolution uses the defining module; typed entries preserve
mandatory state. Keep duplicate rejection and every configured rule consumer
unchanged. No DICE key, analysis consumer, request overlay, cache, asynchronous
owner or fallback changes.

- **Exact:** distinct String, Label and typed requirements; defining-module
  conversion; default mandatory true; retained explicit false; source order;
  empty and existing singleton String aspects; complete source clippy aspect
  loading.
- **Slug-native:** Rust canonical labels and immutable evaluator-detached Arc
  storage.
- **Unsupported/deferred:** strictest-wins duplicate normalization; configured
  aspect propagation/resolution, optional absence, invocation and actions; the
  following clippy rule; other aspect/config-common breadth.

## Allowlist, proof and caps

Only these files may change:

| File | Base SHA-256 | Base lines | Final ceiling |
|---|---|---:|---:|
| `app/slug_loading_v2/src/package.rs` | `bd44500ef347d7b215baca5ccbb141348514a5a7a5d705732dcfa15bf6ea7621` | 6,222 | 6,305 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `033876a8aacffe0a2741e2553c6a5a6a73b7bafd47b420026d7b0f741da0f665` | 6,735 | 6,875 |

Caps are 80 production, 140 proof and 220 total additions; deletions do not
buy addition budget. No new function may exceed 120 lines. `package.rs`
remains above the physical complexity trigger, but this change unifies its
existing adjacent rule/aspect declaration owner and removes the singular
parallel representation rather than adding another responsibility.

Required proof:

1. Freeze an aspect in a non-root module with distinct String, Label and typed
   requirements crossing an apparent mapping. Assert order and mandatory
   true/false, including that true and false are structurally distinct.
2. Preserve empty and singleton String analyzer/rustfmt aspects. Reject a
   non-list, wrong entry type and duplicate canonical labels before export.
3. Change the pinned clippy source-shaped proof to its real mixed list. Include
   its accepted attributes, providers, fragments and doc; assert both canonical
   requirements and mandatory states; prove implementation and following rule
   helper stay lazy.
4. Keep all rule/config-common requirement, attribute, provider and private-
   toolchain proofs green. Do not weaken current duplicate or optional-rule
   invocation rejections.

No new oracle fixture is needed: pinned Bazel source/tests establish the
declaration contract and the focused source extract discriminates the live
rules_rust boundary.

## Serial validation and STOP

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused mixed-aspect, clippy, rule/config-common and rustfmt tests;
- `cargo test -p slug_loading_v2 --lib --locked`;
- `cargo test -p slug_loading_v2 --test bzl_invalidation --locked`;
- `cargo test -p slug_loading_v2 --test build_file_loading --locked`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2 --locked` before any rebuilt-binary smoke;
- `cargo fmt --all -- --check`, `git diff --check`, and
  `scripts/v2_archive_status.sh` with only its three known archive-only misses.

Independent terminal review must verify the shared retained representation,
rule-consumer nonchange, source stop, exact/Slug-native/deferred boundary,
Zabel guidance-only role, utility choice, validation and caps.

STOP and `REPLAN` for any other file; dirty source/authority; changed rule
semantics or configured consumers; duplicate approximation; DICE/analysis/
repository/oracle/action work; the following clippy rule; Java/JVM work;
copied Zabel behavior; a new utility/ledger need; or a cap violation.

## Immediate predecessor

`5f8dd852` accepted the exact clippy private-label map with 219 loading, 24
invalidation and 31 BUILD-loading tests plus independent terminal review. The
earlier pre-attribute toolchain candidate was fully reverted and supplies no
implementation state.
