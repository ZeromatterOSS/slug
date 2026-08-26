# Current Slug V2 Packet

Packet: `WP-4-7A-rustfmt-test-aspect-provides-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: loading-owned frozen aspect advertised-provider identity
Base: `df654bfb`

Result: load and freeze accepted rules_rust 0.73.0
`rust/private/rustfmt.bzl:194-216`. Extend the existing frozen aspect owner
with the fixed singleton `provides = [RustfmtTestInfo]` producer identity.
Do not apply the aspect, validate its returned providers, perform provider
matching or advance into the `rustfmt_test` rule.

## Accepted starting point and first absent fact

Commit `275e0b24` freezes the second rustfmt aspect's private schemas and
required producer object. Commit `df654bfb` selects the next source-order
audit. `RustfmtTestInfo = provider(...)` at lines 194-200 already exports its
two-field provider identity. The string-list constant at line 202 evaluates,
and function bodies at lines 204-208 remain lazy.

The third aspect at lines 210-216 reuses accepted implementation,
`attr_aspects = ["deps", "proc_macro_deps", "crate"]`, one exported
`requires = [rustfmt_aspect]` edge and documentation. Its first unsupported
argument is `provides = [RustfmtTestInfo]` at line 214. The provider was first
exported in the same rustfmt module and must retain that producer identity
through aspect export, recursive freeze and an importer alias.

## Bazel authority and Zabel architectural guidance

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole behavior authority.
The accepted rules_rust archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
Use only those pinned objects and archive.

The authenticated Bazel chain is:

- `StarlarkRuleFunctionsApi.aspect` declares `provides` as a named provider
  sequence with an empty default;
- `StarlarkRuleClassFunctions.aspect` calls
  `StarlarkAttrModule.getStarlarkProviderIdentifiers` during declaration;
- that converter casts every item to `Provider`, rejects any unexported
  constructor, projects its producer `Provider.Key` into a
  `StarlarkProviderIdentifier`, and constructs an immutable set, so duplicates
  normalize and set equality is order-insensitive;
- `StarlarkDefinedAspect` retains that set in equality/hash and transfers it
  to advertised providers only during later `buildDefinition`; aspect
  execution and advertised-provider verification occur later; and
- `StarlarkRuleClassFunctionsTest.aspectProvides` and
  `aspectProvidesError` authenticate producer-key retention and element
  validation, while `StarlarkDefinedAspectsTest.aspectAdvertisingProviders`
  proves returned-provider enforcement is an application-time concern and
  therefore outside this packet.

Pinned Zabel commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Its `build_rule_declaration.zig` retains `provides` in the complete
producer-owned `AspectDefinition`, validates provider-definition values, and
follows the value during module freeze while keeping `AspectExportIdentity`
separate. Slug follows only that owner shape using its existing `ProviderId`
and frozen aspect lifetime. No Zig code, representation, evaluator behavior,
cache, analysis algorithm or compatibility claim may be copied; Bazel 9.2
remains sole compatibility authority.

## Compatibility classification

- **Exact:** validation and freeze of the fixed singleton
  `provides = [RustfmtTestInfo]`; preservation of
  `@@dep+//rust/private:rustfmt.bzl%RustfmtTestInfo`; declaration-time
  exported-provider validation; omission as no advertised provider; the third
  aspect's existing implementation, ordered attr-aspects, required producer,
  documentation and first-export identity; lazy implementation behavior.
- **Slug-native:** existing `ProviderId`, Arc-backed singleton storage,
  compact strings, Rust equality/order and diagnostics, complete-module
  fingerprint over-invalidation and memory accounting.
- **Unsupported/deferred:** explicit empty, duplicate or wider `provides`
  lists, native providers, unexported/non-provider values beyond rejection,
  advertised-provider production/matching, aspect application/propagation,
  required-aspect execution, configured dependencies/fragments/toolchains,
  actions, the later `rustfmt_test` rule, M8/M7B and exact Bazel
  configuration/output identity.

## Natural owner, lifetime and utility reuse

`UserProviderCallable::export_as` and `FrozenUserProviderCallable` already
own the structural `ProviderId` consisting of producer module label plus first
exported name. Reuse the existing aspect-provider ID converter rather than
retaining the constructor object or reconstructing text at the consumer.
`AspectDefinitionGen` and `FrozenAspectDefinition` remain the sole
declaration/freeze owners. Retain the singleton in one Arc slice released with
the frozen recursive Bzl module; borrow no evaluator heap or request scratch.

No DICE key, source observer, repository mapping, I/O, cache, interner, hash
domain, lock, async task or command result changes. Existing recursive module
identity/fingerprint invalidates the declaration. No fallback is introduced.
The Buck2 utility audit selects existing Arc slices, `ProviderId`,
`CompactString` and `Allocative`; no new retained utility, collection or
representation family is admitted.

## Implementation boundary

1. Generalize or reuse the existing transient/frozen exported-provider ID
   projection without changing accepted required-provider behavior.
2. Add a bounded converter that maps omission to an empty Arc slice and accepts
   only one exported user-provider constructor for `provides`. Clone its
   existing `ProviderId`; reject explicit empty, wider, duplicate,
   non-provider and unexported values.
3. Add the named `provides` input to the existing `.bzl`-only aspect global
   and freeze the resulting singleton in the existing aspect definition.
   Preserve earlier omitted state, first-export identity and BUILD absence.
4. Do not retain raw Starlark provider values, create a provider/aspect side
   registry, invoke an implementation, or wire advertised providers into
   configured analysis.

## Discriminating proof

- Extend the accepted recursive rustfmt fixture through
  `RustfmtTestInfo` and `_rustfmt_test_aspect`. Assert the frozen third
  aspect's producer identity, ordered attr-aspects, complete required
  `rustfmt_aspect` identity and exactly
  `@@dep+//rust/private:rustfmt.bzl%RustfmtTestInfo`. Give the lazy body a
  failure that must not run.
- Prove an importer alias does not rewrite either provider or aspect identity,
  and earlier aspects retain empty advertised-provider state.
- Reject explicit empty, duplicate, wider, non-provider and transient
  unexported lists; preserve the existing BUILD-global absence proof. Add no
  fixture, registry response, network request or Bazel run.

## Allowlist and growth caps

Only these files may change from base `df654bfb`:

| File | Base SHA-256 | Base lines | Final line cap | Purpose |
|---|---|---:|---:|---|
| `app/slug_loading_v2/src/package.rs` | `37f3c16a003113b90411e59372f5fa39982b1bf9f6ec9d1cc9674dbbe953c052` | 5,688 | 5,753 | retained advertised provider and bounded conversion |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `c0025b63556349e5bdf9fd651122d8f04cfc90d1a25f6240b972027d1d496c78` | 4,830 | 4,950 | recursive identity/freeze and rejection proofs |

Additions are capped at 65 production lines, 120 proof lines and 185 total
lines. Deletions do not buy addition budget. No touched function may exceed
150 lines. `package.rs` already exceeds the 2,000-line review trigger, but the
existing aspect declaration remains the cohesive owner for this adjacent
field; a new module or registry would split one semantic lifetime. STOP if the
converter cannot remain a small private helper.

## Serial validation and review

Run Cargo commands serially with one shared target directory:

```text
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 rustfmt_test_aspect
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 rustfmt_second_aspect
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 bazel_aspect_definition_validates_admitted_fixed_abi_and_build_absence
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo check --locked -p slug_core_v2
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo build -p slug_cli_v2
cargo fmt --check
git diff --check
scripts/v2_archive_status.sh
```

The archive checker may report only its known three retained thoughts paths
plus active packet files. Recheck hashes, additions, physical lines and
touched-function lengths before review. Independent terminal review is
mandatory before commit and must verify source order, pinned Bazel behavior,
pinned Zabel guidance-only use, provider producer identity, singleton
validation, lazy implementation, frozen lifetime, BUILD absence, caps, serial
validation and absence of a new semantic side owner.

## STOP / `REPLAN`

STOP and `REPLAN` if completion requires a file outside the allowlist; any
`provides` shape beyond the fixed singleton; native or unexported provider
identity; raw evaluator-value retention; provider-ID reconstruction; aspect
application, advertised-provider matching, propagation, configured
dependencies/fragments/toolchains, analysis/actions; a new DICE key, mapping,
cache, I/O path, interner, hash or lifetime owner; Java/JVM work; Zabel code or
behavior adoption; an unpinned source; a new fixture/oracle/network request; a
cap violation; or a public rules_rust success claim. After the third aspect
freezes, stop and select the first unsupported expression in `rustfmt_test =
rule(...)` separately.
