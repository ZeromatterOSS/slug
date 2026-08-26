# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-empty-header-info-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: private C++ loading capability and empty HeaderInfo value
Base: `f65c9ce0`

Result: complete rules_cc's top-level empty compilation-context row by
admitting only Bazel's no-argument empty
`cc_internal.create_header_info()` value. Keep all non-empty, dependency,
configured-analysis and other C++ semantics unsupported, and stop at the next
top-level expression in the same file.

## Accepted starting point and source-order stop

Commit `f65c9ce0` accepts omitted/`None`, unique string-list and documented-map
provider schemas plus arbitrary optional loading values. One frozen provider
definition owns schema and exported identity; schemaful instances use compact
ordinal slots, schemaless instances retain dynamic names, and only the existing
complete documented-string representation enters configured analysis. Focused
proof, all 203 loading units, configured analysis, locked checks, rebuilt CLI,
formatting and hygiene pass. Independent review returned `ACCEPT`.

Recursive source order now freezes
`cc/private/link/create_extra_link_time_library.bzl` and resumes rules_cc
0.2.17 `cc/private/cc_info.bzl`. Its provider declarations and top-level
documented provider construction are accepted. The first absent expression is
line 134 inside the empty compilation context:

```starlark
_header_info = _cc_internal.create_header_info()
```

Once the empty HeaderInfo exists, evaluation passes the accepted linking and
debug-context constructions plus lazy functions. It then reaches lines
260–269 in the same file:

```starlark
CcInfo, _ = provider(
    doc = "Provider for C++ compilation and linking information.",
    fields = { ... },
    init = _create_cc_info,
)
```

The accepted initializer supports only list schemas. This documented
dictionary-schema initialized provider is the next stop and a separate audit;
`cc_info.bzl` does not yet freeze.

## Fixed sources and compatibility authority

Reuse the accepted rules_rust/rules_cc materialization and graph. Relevant
fixed inputs are:

- rules_cc `cc/private/cc_info.bzl` SHA-256
  `4424bb876c3f8234d7cfce20652e7ab1a7b2fc34cc2c637b1cb4313590d9f1bc`;
- Bazel `CcStarlarkInternal.java` SHA-256
  `143e7e4f63deac9f65ca4e85e2e4d84f3fedf6560428e1dc6f975b2255424f53`;
- Bazel `CcCompilationContext.java` SHA-256
  `580d0af792672b35a4da506dbb837bf99222199d70887855493e7d50963a21cd`.

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole behavior authority.
`CcStarlarkInternal.createHeaderInfo` declares eight named-only parameters and
passes a fresh Starlark-thread identity token, four optional derived-artifact
module fields, four artifact sequences and empty dependency lists to
`HeaderInfo.create`. With no arguments, the module fields are `None`, all four
header sequences become immutable empty lists, and both dependency inputs are
empty. `HeaderInfo` exposes the eight fields, compares by its identity token,
is immutable and is hashable in Bazel.

Only the zero-argument loading row and its empty field observations are
selected. Pinned source is sufficient; no new Bazel fixture is required.

## Zabel and Buck2 architectural guidance

Pinned clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is concept/test guidance only.
`src/starlark_host/engine/builtins_cc_primitives.zig` keeps one evaluator-local
HeaderInfo field row, validates the full constructor before allocation, owns
immutable header sequences with the value, and requires retained provider
lowering to copy the row before the evaluator ends. Its rules_cc-shaped test
separately proves the empty compilation-context call. Slug follows the owned
loading value and later-lowering phase split but does not copy Zig code,
representation, layout, allocator, errors, API breadth or analysis behavior.
Bazel remains compatibility authority.

The selected empty row needs no dynamic collection: retain one frozen empty
Starlark list with the HeaderInfo and project it to the four equal list fields.
The four module fields project `None`. Default starlark-rust value occurrence
identity supplies distinct constructor results and remains shared by aliases
through freeze. Reuse `FrozenValue`, `Trace`, `Freeze` and `Allocative`; add no
counter, map, interner, registry, side store or digest. This is an existing-
utility decision, so no Stage 9 ledger row is required.

## Compatibility classification

- **Exact:** `.bzl` `cc_internal.create_header_info()` accepts zero arguments
  and returns a fresh immutable value of type `HeaderInfo`; its
  `header_module`, `pic_header_module`, `separate_module`, and
  `separate_pic_module` fields are `None`; `modular_public_headers`,
  `modular_private_headers`, `textual_headers`, and
  `separate_module_headers` are immutable empty lists; aliases compare equal,
  separate calls compare unequal; the selected rules_cc empty compilation
  context freezes and evaluation reaches the later `CcInfo` declaration.
- **Slug-native:** one frozen empty list may back the four observationally
  equal empty list fields; Rust/starlark-rust storage, display text and
  nonrequired diagnostics remain native. The value is loading-only and carries
  no configured semantic projection.
- **Unsupported/deferred:** every named constructor argument and non-empty
  HeaderInfo; Bazel HeaderInfo hashing; dependency DAGs and
  `create_header_info_with_deps`; Files/artifacts; HeaderInfo returned from or
  consumed by configured analysis; CcCompilationContext semantics; the
  dictionary-schema initialized `CcInfo`; every other C++ private
  method, provider, toolchain and action surface; later rules_cc/rules_rust
  source, M8/M7B and exact output bytes.

## Ownership, lifetime and implementation boundary

Keep `CcInternalModule` as the capability returned only by the accepted
owner-checked `.bzl` bridge. Add a method table containing only
`create_header_info`; do not install a public global or expose the method in
BUILD evaluation.

Add one loading-only immutable HeaderInfo value in `cc_common.rs`. It owns a
frozen empty list allocated in the module frozen heap and projects that value
to the four header-list fields; module fields return `None`. Its heap occurrence
is its identity, so the value must not be a zero-sized singleton and aliases
must freeze to the same occurrence. Do not add a numeric identity counter or
retain evaluator scratch. The value never crosses into the configured provider
decoder.

`BzlModuleEvalKey` and the recursive source observations remain the sole
invalidation owner. There is no DICE, request, command, async, cache,
publication, cancellation or shutdown change.

## Discriminating proof

- Evaluate from a canonical `rules_cc+` `.bzl` owner, create two empty
  HeaderInfos, and prove exact type, eight attributes, empty values, immutable
  list behavior, alias equality and distinct-call inequality before and after
  module freeze.
- Evaluate the source-shaped `CcCompilationContextInfo(...,
  _header_info = cc_internal.create_header_info())`, freeze it, and read the
  nested HeaderInfo fields through the loading-only provider instance.
- Prove positional, unknown named and selected known named arguments fail
  closed; the known named row is explicitly deferred rather than claimed
  exact.
- Keep the private bridge owner/BUILD boundary test green.
- Keep the provider-schema and initialized-provider regressions green.
- Do not add a full rules_cc fixture or claim `cc_info.bzl` succeeds.

## Allowlist and caps

Only these files may change from base `f65c9ce0`:

| File | Base SHA-256 | Base lines | Final cap | Purpose |
|---|---|---:|---:|---|
| `app/slug_loading_v2/src/cc_common.rs` | `047f0c9464092f0ec4cfc58671dc4c3033a9d19f48828752b98034e68d6a9d59` | 89 | 180 | empty HeaderInfo and one private method |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `7bc1f6f974da7cf9bb3fab2163e88445a7797194183f5458a6f97740493689c3` | 5,597 | 5,720 | identity, fields, freeze and source stop proof |

Production additions are capped at 90, proof additions at 115 and total
additions at 205. Deletions do not buy addition budget. No new or touched
function may exceed 120 lines. The test file exceeds the 2,000-line review
trigger, but adding the focused regression beside the existing private-bridge
and provider tests preserves their shared evaluator helper and source-order
narrative; moving it would require widening `lib.rs` and the packet allowlist.

## Serial validation

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused empty-HeaderInfo/source-shaped test;
- existing private bridge, provider-schema and initialized-provider tests;
- one focused configured string-provider analysis regression;
- `cargo test -p slug_loading_v2 --lib`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2` after Rust changes;
- `cargo fmt --all -- --check`;
- `git diff --check`;
- `scripts/v2_archive_status.sh`.

The broad daemon-sensitive loading integration remains 30/31 only for its
known stale `@external` diagnostic-order row and need not rerun unless focused
evidence exposes integration risk. Recheck base hashes, caps, allowlist,
function sizes, configured-analysis non-widening and the clean Zabel pin.

The new retained Starlark value requires independent selection and terminal
implementation reviews. Both must verify Bazel authority, Zabel's guidance-
only role, immutable-list lifetime, occurrence identity through freeze,
configured fail-closed behavior, compatibility classes and every cap.

## STOP / REPLAN

STOP and REPLAN for a file outside the allowlist; analysis/build-api/DICE edit;
a global or numeric identity registry; mutable or evaluator-borrowed retained
fields; public BUILD exposure; any named/non-empty HeaderInfo admission;
HeaderInfo dependencies, hashing or configured lowering;
`create_header_info_with_deps`; another `cc_internal` method; initialized
dictionary-schema providers; source/mapping/materializer/network/fixture
change; Java/JVM work; copied Zabel code or behavior; cap violation; or a claim
beyond the empty compilation-context row. After it succeeds, stop and audit
the dictionary-schema initialized `CcInfo` at lines 260–269 separately.
