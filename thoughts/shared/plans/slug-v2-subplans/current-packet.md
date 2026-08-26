# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-provider-doc-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: Stage 4 `package_globals::provider` adapter and retained provider callable
Base: `1a527089`

Result: accept Bazel's named string/`None` `doc` argument on the existing
provider global, preserve the current semantic callable identity/schema and
prove frozen export through recursive `.bzl` loading. Do not implement
documentation extraction or provider-instance breadth.

## Learned facts and authority

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority:

- `StarlarkRuleFunctionsApi.provider` declares named `doc` as `string | None`
  with `None` default. `fields` and `init` are separate named parameters.
- `StarlarkRuleClassFunctions.provider` trims present documentation and stores
  it through `StarlarkProvider.Builder`; `None` stores nothing and a non-string
  fails argument conversion.
- `StarlarkProvider.getDocumentation` serves Java documentation consumers, but
  `ProviderApi` exposes no Starlark attribute. Exported callable equality/hash
  use only the `.bzl` key and exported name.
- `StarlarkRuleClassFunctionsTest.declaredProviderDocumentation`,
  `declaredProvidersDoc` and `declaredProvidersBadTypeForDoc` authenticate
  trimming, acceptance and rejection. `StarlarkProviderTest` authenticates
  stored metadata. `StarlarkDocumentationTest` and
  `ModuleInfoExtractorTest.providerDocstring` prove that retention belongs to
  the separate Bazel documentation surface.

The accepted rules_rust 0.73.0 archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
`rust/private/providers.bzl` makes 18 top-level declarations, each with a
string `doc` and dictionary `fields`; there is no `init`, list schema or
instance construction. Freezing exports all callables. `common.bzl` loads six
and stores them in the already accepted `rust_common` struct before any rule
implementation runs.

Slug already validates dictionary field documentation as strings, reduces it
to sorted semantic field names, and freezes `UserProviderCallable` with a
structural source-label/exported-name `ProviderId`. Bazel provider identity
also excludes documentation. No admitted Slug command extracts docs.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architecture guidance only.
Its complete retained semantics owner and narrow consumer projections support
keeping one global adapter and projecting only build-semantic provider schema
and identity. Copy no Zabel code, representation, fingerprint, scheduler or
behavior; Bazel remains provider authority.

## Decision and non-decisions

In `package.rs`, add named `doc: Option<NoneOr<&str>>` immediately before the
existing named `fields` parameter of `package_globals::provider`. The outer
`Option` represents omission and `NoneOr` distinguishes explicit Starlark
`None` from a string. Consume `doc` in that adapter and continue delegating the
unchanged field map/evaluator to `UserProviderCallable::from_evaluator`.

Do not retain documentation in `UserProviderCallable`, add an accessor or add a
metadata registry. This packet admits build/query loading behavior, where doc
is not observable and does not affect Bazel identity. Bazel doc trimming,
retention and extraction remain explicitly unsupported/deferred; a future
documentation command must `REPLAN` and retain provider plus field docs from
their declaration owner.

Do not change `provider.rs`, `ProviderId`, provider fields/instances, analysis,
globals placement, BUILD/MODULE/REPO behavior, DICE keys, source observations,
events or error translation. Do not admit `fields` list/`None` or `init`.

## Ownership, revision and lifetime

`package_globals::provider` remains the complete call-shape adapter and
`UserProviderCallable` remains the sole owner of admitted semantic field schema
and exported identity. The existing source observation invalidates any doc
edit before module evaluation. Because prose is not an admitted build fact,
the retained callable may remain semantically equal after a prose-only edit.

No request input, revision certificate, overlapping-request behavior,
publication or equality rule changes. No memory is added: globals remain
evaluation-local and the frozen callable retains only its existing
DICE-owned identity/schema. Cancellation, evaluator lifetime and module
ownership remain unchanged. No fallback, cache, task or dependency is added.

## Files and caps

Allowed files, with base SHA-256 and final line ceiling:

| File | Base SHA-256 | Cap |
|---|---|---:|
| `app/slug_loading_v2/src/package.rs` | `f692707b38aea95db52095fd7f650d86a4a3937b4dc4ad67f69b2d1a4fc6a0f0` | 5,120 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `85640fe02edfeffc8a2bfd0a78d9f1c8cbd58b0303d1bcd571cc6723df11e884` | 3,880 |

Production additions are <=5, proof additions <=65 and total additions <=70.
Both files exceed the authoring-guide size trigger, but the production change
is one parameter at the existing sole global adapter and the proof belongs in
the existing recursive external-Bzl harness. Physical splitting would widen
scope without separating a responsibility.

## Proof and validation

Add focused recursive external-Bzl proofs that documented providers with
string and `None` docs bind, export and freeze with the expected source-label
and exported-name identities. Add a non-string `doc` rejection through the
same evaluator boundary. Do not expose documentation merely to inspect it.

Run:

- `cargo fmt --check` and `git diff --check`;
- the focused external-Bzl provider-doc tests;
- full `cargo test -p slug_loading_v2`;
- `cargo check -p slug_core_v2 --locked`;
- `cargo build -p slug_cli_v2 --locked`;
- with clean `slugd` lifecycle and fresh output roots, the existing disposable
  rules_rust query and build, recording the next common internal/public
  terminal.

Pinned source/tests already discriminate the call contract, so no new Bazel
fixture or copied archive is authorized.

## Compatibility and STOP

- **Exact:** named string/`None` `doc` acceptance, non-string rejection, and
  unchanged provider binding/export/freeze identity for the live dictionary
  `fields` loading route.
- **Slug-native:** Rust storage, valid-Unicode strings, internal error
  representation and nonrequired diagnostic wording.
- **Unsupported/deferred:** Bazel doc trimming/storage and Stardoc extraction,
  field-documentation access, `fields` list/`None`, `init`, broader provider
  instances/analysis, later rules_rust toolchains/actions, M8/M7B and exact
  output bytes.

STOP on dirty overlap, edits outside the two-file allowlist, documentation
retention/accessors/side stores, provider identity/schema changes, environment
widening, instance/analysis changes, source vendoring, Java/JVM, dependency
drift, public documentation claims or scope above the caps. `REPLAN` before
crossing a boundary.
