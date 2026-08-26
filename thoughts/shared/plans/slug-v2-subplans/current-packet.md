# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-bzl-struct-builtin`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: Stage 4 complete BUILD/`.bzl` loading globals
Base: `54d28477`

Result: expose retained starlark-rust `StructType` in every Bazel `.bzl`
evaluation and nowhere else. The real rules_rust `_support` shape constructs,
reads and freezes across a recursive external load; BUILD, MODULE and REPO
environments remain unchanged.

## Learned facts and authority

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority:

- `StarlarkGlobalsImpl.getFixedBzlToplevels` installs
  `StructProvider.STRUCT`; fixed BUILD, MODULE and REPO methods do not.
  cquery and SCL also contain it but are outside this packet.
- `BazelStarlarkEnvironmentTest.buildAndModuleBzlEnvsDeclareSameNames` proves
  BUILD-loaded and MODULE-loaded `.bzl` files expose the same names.
- `StructProvider.createStruct` and `StarlarkRuleClassFunctionsTest` rows
  `testStructCreation`, `testStructFields`, `testStructEquality`,
  `testStructIncomparability`, `testStructPosArgs`, `testStructStr` and the
  export row authenticate the wider Bazel value surface.

The live rules_rust 0.73.0 load needs only a smaller slice. During evaluation
of `rust/platform/triple_mappings.bzl`, `_support` constructs two named bool
fields, the comprehension reads `support.std`, the structs remain dictionary
values, and the completed module freezes/exports those values. Loaded
`triple.bzl` defines more named-field constructors but does not invoke them at
this terminal. Comparison, concatenation, provider identity, formatting, JSON
and struct-key hashing are not required here.

Retained starlark-rust already implements named-only construction and no
positional arguments in `register_struct`, immutable field access and
order-insensitive equality/hash in `StructGen`, and frozen storage through its
derived `Freeze`. Do not claim the whole implementation exact: its `compare`
orders structs although Bazel rejects ordering, it lacks Bazel struct
concatenation/provider identity, and its display spacing differs.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architecture guidance only.
`session_analysis_starlark_semantics.zig` projects one typed semantics value to
the relevant consumers, while `injected_repository_starlark_semantics.zig`
retains and authenticates a complete request value. Follow the corresponding
single complete globals-owner pattern; copy no Zabel code, representation,
fingerprint, scheduler or behavior. Bazel remains builtin/value authority.

## Decision, owner and non-decisions

`package.rs` remains the sole complete loading-global owner. Add one private
common builder, keep `loading_globals()` as the `.bzl` environment and include
exactly `LibraryExtension::{Print, StructType}` there. Add a distinctly named
`build_file_loading_globals()` that preserves the current Print-only extension
set. Both receive the same existing package/select/native/attr/config/platform
and bounded provider symbols.

In `bzl_module.rs`, switch only the Host package-attempt and legacy
`PackageLoadKey` BUILD evaluations to `build_file_loading_globals()`. The Host,
external and legacy recursive `.bzl` evaluations keep `loading_globals()` and
therefore share one complete value. Repository-package and root-package BUILD
evaluation already flow through the Host package-attempt owner.

Do not touch the preliminary core BUILD evaluator, Stage 5 MODULE/REPO globals,
cquery, parser/dialect logic, source/load traversal, DICE keys, events or error
translation. Do not enable all `LibraryExtension`s and do not implement a new
struct value or Bazel's deferred broader struct semantics.

## Ownership and lifecycle

The environment selection is a constant semantic fact owned at the Stage 4
evaluation boundary; it has no request input or filesystem observation and
adds no DICE identity/invalidation field. Each `Globals` value remains
evaluation-local. Successful struct instances freeze with the module into the
existing DICE-retained `FrozenBzlModule`; they borrow neither command scratch
nor an evaluator heap. Cancellation, overlapping requests, equality cutoff,
events, source observations and shutdown behavior are unchanged.

No fallback, new cache, global registry, task, async transfer, dependency,
lockfile entry or public API is authorized. No performance measurement is
needed because this is a fixed environment member and the admitted real route
already performs its construction.

## Files and caps

Allowed files, with base SHA-256 and final line ceiling:

| File | Base SHA-256 | Cap |
|---|---|---:|
| `app/slug_loading_v2/src/package.rs` | `650a5784681ac1ba5f8a20e3eb08cb4831d27fa7cb36ef439f5b425b10be08ef` | 5,140 |
| `app/slug_loading_v2/src/bzl_module.rs` | `05de7358109c2a7a017522fdc7b685e9bbf518fb5b93c8ada0526f6cb8289034` | 9,650 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `ac91348377a41bb9ddc901890dbb3e2442eafc8b702d3bb9a7b1f0fbaf345a00` | 3,845 |
| `app/slug_loading_v2/src/host_package_attempt_tests.rs` | `41365b2b59e145b447053ac1b42d68fc3cc7baf1071a486d9cb32142c76b4687` | 550 |

Production additions are <=30, proof additions <=80 and total additions <=110.
The two production files exceed the authoring-guide size trigger, but the
change is cohesive with the existing sole globals owner and changes only two
evaluation selections in the orchestration file. A broader physical split
would cross packet scope.

## Proof and validation

Extend the recursive external-Bzl proof with the real `_support` form:
construct named `std`/`host_tools` bool fields, read both fields, export the
result through its parent and inspect the frozen value. Add one Host package
attempt proving `struct` remains absent in BUILD evaluation. Do not assert
diagnostic text beyond the stable missing-variable discriminator.

Run:

- `cargo fmt --check` and `git diff --check`;
- focused external-Bzl and Host package-attempt tests;
- full `cargo test -p slug_loading_v2`;
- `cargo check -p slug_core_v2 --locked`;
- `cargo build -p slug_cli_v2 --locked`;
- with clean `slugd` lifecycle and fresh output roots, the existing disposable
  rules_rust query and build, recording the next common internal/public
  terminal after `struct`.

Pinned source/tests already discriminate environment placement and the value
surface, so no new Bazel fixture or copied archive is authorized.

## Compatibility and STOP

- **Exact:** Bazel 9.2 `.bzl` availability, named-only bool construction,
  immutable field access and frozen recursive export exercised by the live
  rules_rust load; absence from BUILD/MODULE/REPO.
- **Slug-native:** Rust storage/layout, valid-Unicode strings, evaluator/error
  representation and nonrequired diagnostic wording.
- **Unsupported/deferred:** cquery/SCL activation, struct ordering,
  concatenation, provider identity, exact display/JSON/hash bytes and other
  unexercised value breadth; later rules_rust providers/toolchains/actions,
  M8/M7B and exact output bytes.

STOP on dirty overlap, any new struct representation, exposure outside the
authenticated `.bzl` environment, all-extension activation, per-evaluator
symbol reconstruction, parser/dialect changes, DICE/source/event changes,
source vendoring, Java/JVM, dependency drift, public diagnostic widening or
scope above the caps. `REPLAN` before crossing a boundary.
