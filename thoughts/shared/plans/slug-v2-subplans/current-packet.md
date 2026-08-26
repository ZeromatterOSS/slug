# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-keyword-only-arguments`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: retained Starlark dialect and Stage 4 BUILD/`.bzl` parse boundaries
Base: `2f373248`

Result: Bazel 9.2 keyword-only definition and lambda parameter forms are
accepted through one retained Bazel dialect. Every Stage 4 production
BUILD/`.bzl` parse and the live preliminary root-BUILD evaluator consume that
same value; MODULE evaluation and every unrelated dialect field are unchanged.

## Authority and accepted design

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority. Its `Resolver`, `Parameter`, `StarlarkFunction`,
`FunctionTest.testKeywordOnly`/`testStarArgsAndKeywordOnly`,
`ResolverTest.testParameterOrdering`, and `ParserTest.testLambda` authenticate
the exact syntax, ordering and call-binding slice. No new oracle is needed;
these pinned source regressions discriminate every admitted row.

The accepted rules_rust 0.73.0 route reaches
`rust/platform/triple_mappings.bzl:5` and fails in
`compute_external_bzl_module` at:

```starlark
def _support(*, std = False, host_tools = False):
```

Retained starlark-rust already owns `ParameterP::NoArgs`, `DefParams`
validation, compiled named-only parameters, default evaluation and call
binding. Only `Dialect::Standard.enable_keyword_only_arguments == false`
prevents this path. Add `Dialect::Bazel`, identical to `Standard` except that
this field is true. Do not use `Extended` and do not alter parser/evaluator
logic.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Its injected/session Starlark-semantics design projects one complete typed
value to all relevant evaluators. The retained Bazel dialect follows that
single-owner pattern; copy no Zabel code, representation, fingerprint,
scheduler or behavior. Bazel remains syntax/call authority.

## Exact implementation boundary

In `starlark_syntax::Dialect`, add the documented `Bazel` constant and a test
that freezes every field: def/lambda/load and load reexport enabled,
keyword-only enabled, positional-only/types/top-level/f-strings disabled, and
Unicode string encoding still chosen separately by callers.

Replace only the nine `Dialect::Standard` arguments in `bzl_module.rs` with
`Dialect::Bazel`: Host package attempt, observed Host Bzl, external Bzl, root
package, repository package, legacy Bzl parse/eval and both legacy package
parse/eval calls. Preserve every `StringEncoding::BazelInternal`, source name,
load traversal, event, DICE key and error wrapper byte-for-byte.

The preliminary root-BUILD evaluator in core is live before ordinary loading.
Use `Dialect::Bazel` only when `is_module == false`; retain Standard for its
dormant MODULE branch. Do not change Stage 5 `module_eval`, `repo_file`, test-
only parser helpers or other crates.

## Files and caps

Allowed files, with base SHA-256 and final line ceiling:

| File | Base SHA-256 | Cap |
|---|---|---:|
| `starlark-rust/starlark_syntax/src/dialect.rs` | `35bfaeb5f01ebfea98d20aeec8170858ce0f015a61a9786a5ff2e4d5f016df2f` | 175 |
| `starlark-rust/starlark_syntax/src/syntax/def.rs` | `5f3e9eb9b8bfa872af27807ca64fccea5d5c8c5164469c5b350c1f4234ff20d7` | 510 |
| `app/slug_loading_v2/src/bzl_module.rs` | `c24e225d055f4dee9caf41435e55eba49cbfebb0d907f0a347cc9f1f17e09327` | 9,675 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `0bf30933cb0173ff14f3d97867f386ea62ee1da87db54a3dd50a679cfcf519e0` | 3,850 |
| `app/slug_core_v2/src/runtime/starlark.rs` | `dace8e6ec43a6ea097798a92ea3e3d96285fafd18592a2e7eb7545400da75e71` | 105 |

Production additions are <=35, proof additions <=120 and total additions
<=155. Apart from the retained `Dialect::Bazel` constant, no public API,
dependency, lockfile, DICE key or new source file is authorized.

## Proof and validation

Prove the Bazel dialect accepts bare `*` with required/defaulted named-only
parameters, `*args` followed by named-only parameters and the admitted lambda
form; prove positional delivery/missing names fail and bare/multiple-star,
duplicate/order errors remain rejected. Evaluate the real `_support` shape.

Add one full external-Bzl route proof through source observation, recursive
parse/evaluation and frozen module result. Add a focused core preliminary
root-BUILD test and prove its MODULE branch is not widened. Do not copy the
rules_rust file or archive into the repository.

Run `cargo fmt --check`; focused starlark-syntax, loading-route and core tests;
full `cargo test -p slug_loading_v2`; `cargo check -p slug_core_v2 --locked`;
and `cargo build -p slug_cli_v2 --locked`. With clean `slugd` lifecycle and
fresh roots, rerun the existing disposable rules_rust query and build, prove
both pass this parse/evaluation boundary, and record the next honest internal
and public terminal. Finish with `git diff --check` and independent review.

## Compatibility and STOP

- **Exact:** Bazel 9.2 bare-`*` and `*args` keyword-only definition/lambda
  parsing, ordering, defaults and call binding on BUILD/`.bzl` routes;
  rules_rust `_support`; unchanged load/source/event ordering.
- **Slug-native:** Rust AST/evaluator storage, valid-Unicode source ingestion,
  typed loading errors and nonrequired diagnostic wording.
- **Unsupported/deferred:** positional-only `/`, types, f-strings, new top-
  level forms, MODULE widening, generic Python syntax, later rules_rust
  providers/toolchains/actions, M8/M7B and exact output bytes.

STOP on dirty overlap, any parser/evaluator repair beyond the existing field,
use of `Extended`, a second/scattered dialect value, changed MODULE behavior,
unrelated syntax activation, source vendoring, Java/JVM, dependency drift,
public diagnostic widening or scope above the caps. `REPLAN` before crossing
a boundary.
