# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-label-list-allow-files-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: existing loading attribute and frozen rule schemas
Base: `4bdd64bf`

Result: retain Bazel's Boolean `allow_files` predicate on
`attr.label_list`, allowing rules_rust to construct and freeze
`rust_stdlib_filegroup`. Stop before extension predicates, source-file target
resolution or the later Rust toolchain declaration.

## Accepted starting point and source-order stop

Commit `4bdd64bf` exposes only the exact deprecated
`cc_common.do_not_use_tools_cpp_compiler_present` property as `None`. The
rules_cc exported wrapper freezes, while every other absent native C++ field
and all configured C++ semantics remain deferred. The focused wrapper proof,
existing private bridge and freeze regressions, all 207 loading units,
configured analysis, locked checks, rebuilt CLI and hygiene pass at 4
production and 34 proof additions. Independent terminal review returned
`ACCEPT`.

Recursive source order then resumes rules_rust 0.73
`rust/private/toolchain.bzl`. The first evaluated declaration is
`rust_stdlib_filegroup = rule(...)` at line 111. Its lazy implementation is not
invoked; `rule`, documentation, mandatory label-list schema and function
capture are already admitted. The first absent expression is line 115:

```starlark
"srcs": attr.label_list(
    allow_files = True,
    doc = "The list of targets/files that are components of the rust-stdlib file group",
    mandatory = True,
),
```

Slug's `attr.label_list` currently has no `allow_files` named parameter. Stop
after this declaration freezes. Do not infer admission of `rust_toolchain`, its
larger attribute schema or `config_common.toolchain_type` near line 940.

## Fixed sources and compatibility authority

Relevant fixed inputs:

- rules_rust `rust/private/toolchain.bzl` SHA-256
  `c4b613cee96540a94fbdf4fbdca7b8dc4ef6d3082024c4d3636afc2e9c4d468e`;
- rules_cc `cc/private/cc_common.bzl` SHA-256
  `5e6ab737945b487759c9f039c77a066dc65bbe15cf590b566fe86029cc610762`.

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior authority.
`StarlarkAttrModule.buildAttribute` accepts Boolean, string-sequence or `None`
`allow_files` values. `setAllowedFileTypes` maps true to
`FileTypeSet.ANY_FILE`, false to `NO_FILE`, and sequences to ordered extension
predicates. `StarlarkRuleClassFunctionsTest.testAttrAllowedFileTypesAnyFile`
proves the true row; `testAttrWithList` separately proves extension filtering
and the absence of `SINGLE_ARTIFACT`.

This packet admits only the normalized Boolean row needed by the selected
source. Omitted, explicit `None` and false all retain the no-file predicate;
true retains the any-file predicate. String sequences and every other value
remain fail-closed.

## Zabel and Buck2 architectural guidance

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
is architectural and test guidance only. Its transient attribute declaration
owns `allows_files` separately from `allows_single_file`, and its generic rule
test proves `attr.label_list(allow_files = True)` retains the former without
the single-artifact property. Slug follows that owner and separation using one
Boolean in its existing declaration/frozen/package schemas. No Zig code,
layout, dispatch, diagnostic or behavior is copied; Bazel remains compatibility
authority.

The Buck2 utility reuse audit selects an inline Boolean because the admitted
domain has two normalized states. This adds no collection, allocation,
interner, hash, clone policy, cache or memory-accounting owner and requires no
Stage 9 ledger row.

## Compatibility classification

- **Exact:** `.bzl` evaluation accepts Boolean/`None`
  `attr.label_list(allow_files=...)`; omitted, `None` and false normalize to no
  files, while true normalizes to any file; the fact survives recursive freeze,
  exported rule capture and target schema construction; the source-shaped
  `rust_stdlib_filegroup` declaration freezes without invoking its
  implementation.
- **Slug-native:** Rust/Starlark diagnostic rendering and the inline retained
  Boolean representation.
- **Unsupported/deferred:** extension-list predicates; file-extension checks;
  actual source-file target admission and configured `ctx.files`; `allow_files`
  on other attribute kinds; `allow_single_file` changes; the later
  `rust_toolchain` declaration and `config_common.toolchain_type`; M8, M7B and
  exact output bytes.

## Ownership and implementation boundary

Add one normalized `allow_files: bool` fact to the existing transient
`AttributeDefinitionGen`, frozen `RuleAttributeSchemaGen` and public
`AttributeSchema`. The existing defining evaluator owns construction; freeze
copies the Boolean, and target invocation copies it into the package schema so
equality and invalidation remain structural. Do not add a registry, side map,
second schema owner or source-file resolver.

`attr.label_list` accepts an optional Boolean/`None` named argument and sets the
fact to true only for Boolean true. Reject sequences and non-Booleans. Keep
`attr.label`, module-extension tags and repository-rule schemas unchanged; if
the new fact reaches those restricted paths, they must reject rather than drop
it.

There is no DICE, request, command, async, mapping, repository, publication,
cancellation or shutdown change.

## Discriminating proof

- Construct omitted, explicit `None`, false and true label-list descriptors;
  prove only true differs in retained file allowance after freeze/export.
- Prove a string sequence and scalar non-Boolean remain rejected.
- Construct and freeze a source-shaped `rust_stdlib_filegroup` rule with
  `mandatory = True`, documentation and `allow_files = True`; prove its `srcs`
  schema is a mandatory label list, allows files and is not single-artifact.
- Invoke the frozen rule with no explicit `srcs` only far enough to prove the
  mandatory schema remains present; do not admit a file target or run the lazy
  implementation.
- Keep existing rule-schema, label-default, provider and configured-analysis
  regressions green.

## Allowlist and caps

Only these files may change from base `4bdd64bf`:

| File | Base SHA-256 | Base lines | Final cap | Purpose |
|---|---|---:|---:|---|
| `app/slug_loading_v2/src/attrs.rs` | `01775c536539550b28da11876ce41f05c0c65219dc7709e5b8eb06f49c574b34` | 1,475 | 1,490 | package-schema Boolean and accessor |
| `app/slug_loading_v2/src/package.rs` | `bc03a8277cd8795919f8f158ec54bd831c3de5cd04794d341de44c85380ba7bc` | 5,833 | 5,875 | declaration, freeze and projection plumbing |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `5625394550fb198a8cb87bbcbb930de27f14b45f3e001725bc947cf179f6d47d` | 5,860 | 5,950 | Boolean matrix and source-shaped rule proof |

Production additions are capped at 45, proof additions at 85 and total
additions at 130. Deletions do not buy addition budget. No new or touched
function may exceed 120 lines. The existing production/test files exceed the
2,000-line trigger, but the fact crosses their existing schema pipeline and
the proof belongs beside the recursive rules_rust loading tests; splitting
would create a second owner and widen the allowlist.

Plan-only selection edits are limited to the canonical plan, Stage 4 subplan
and this manifest and are excluded from implementation caps.

## Serial validation

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused label-list file-allowance/source-shaped rule proof;
- existing Rust analyzer and rules_cc wrapper/loading proofs;
- one configured provider/rule analysis regression;
- `cargo test -p slug_loading_v2 --lib`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2` after Rust changes;
- `cargo fmt --all -- --check`;
- `git diff --check`;
- `scripts/v2_archive_status.sh`.

The broad daemon-sensitive loading integration remains 30/31 only for its
known stale `@external` diagnostic-order row and need not rerun absent
integration risk. The attempted full rules-rust oracle fixture is not packet
evidence: command flag and wildcard-registration boundaries reject before the
selected source loads.

Independent selection and terminal reviews must verify Bazel authority,
Zabel's guidance-only role, normalized Boolean identity, non-single-artifact
separation, source-order stop, compatibility classes, hashes and caps.

## STOP / REPLAN

STOP and REPLAN for a file outside the implementation allowlist; extension
predicates; another attribute kind; a second schema owner or side registry;
actual file-target admission, extension checking or configured `ctx.files`;
the later `rust_toolchain` declaration or `config_common`; module-extension or
repository-rule widening; DICE/analysis/repository/source changes; Java/JVM
work; copied Zabel code or behavior; cap violation; or a claim beyond freezing
`rust_stdlib_filegroup`. Once that rule freezes, audit the next evaluated
`rust_toolchain` schema expression separately.
