# Current Slug V2 Work Packet

Packet: WP-7-101-m7a-canonical-generated-build-bzl-load-r1
Status: ACCEPTED 2026-09-24; M7A remains partial

## Outcome and compatibility

Admit an explicit nonroot canonical `@@repo//package:file.bzl` load from a
canonical external repository BUILD or `.bzl` file. The authentic rules_python
1.7.0 generated `pythons_hub` BUILD must pass its present `@@rules_python+`
LoadLabel stop; a later independently typed stop is acceptable. This is the
source-selected label-resolution edge, not all generated BUILD behavior,
Python toolchain analysis or root-build success.

Exact Bazel 9.2 slice: the `@@` spelling names its canonical repository
without consulting the importing repository's apparent-name mapping; the
existing canonical load route then applies source, containing-package and
load-visibility checks. An invalid canonical label fails before child source;
a valid label for a missing file fails through the source owner. Slug-native:
structural route/module identity and DICE observation epochs instead of Bazel
Skyframe identity bytes. Unsupported/deferred: `@@//` explicit main from a
canonical external importer, broader canonical-label grammar beyond the
ordinary nonroot direct `.bzl` form, generated rule/macro semantics, exact
diagnostic wording, Python repository effects/toolchain analysis and M7A/M8
close. Existing apparent `@repo`, relative `:file.bzl`, mapping and cycle
behavior must remain exact where already accepted.

## Source, owner and decision

Pinned Bazel 9.2 `Label.java:176-191` selects the raw repo for canonical
`@@` and uses `repoMapping.get` only for apparent `@`; `LabelTest.java:123-155`
has a remapping discriminator. `PackageFunction.java:1104-1155` resolves
generated BUILD loads with that package context. `BzlLoadFunction.java:
663-731,797-831,923-925,1282-1318` keeps source package, declaring
mapping and load visibility in their existing owners; no BUILD target export
is required. Authenticated rules_python 1.7.0
`python/private/pythons_hub.bzl:26-45,81-103` generates the direct load
`@@rules_python+//python/private:py_toolchain_suite.bzl`. Its source package
has `python/private/BUILD.bazel`; the suite's transitive apparent loads use
the rules_python mapping, not the generated hub mapping. Bazel may load the
generated BUILD's two direct imports in parallel; source-text order is not an
error-order guarantee.

The accepted `module-extension-use-repo` fixture covers root BUILD loading a
generated `.bzl`, not generated BUILD loading an owner-module `.bzl`. The
fresh Bazel 9.2 scratch at `/tmp/slug-wp801-canonical-load` uses a local
`producer` module with canonical `producer+`; an extension-generated hub
BUILD loads `@@producer+//:suite.bzl`. On an unprimed output base, query of
the generated target succeeded, while a missing canonical `.bzl` failed with
the named missing file. Convert this evidence to a small Stage 1 fixture and
independently replay it. Existing `LabelTest` plus focused Slug resolver
tests distinguish canonical from apparent mapping even when fixture output
does not expose the mapping lookup itself.

`bzl_module.rs::resolve_canonical_external_bzl_load_label` is the semantic
owner. The current canonical-source branch calls `ApparentLabel::parse` for
every nonrelative load; that parser deliberately rejects `@@`. The accepted
route already computes `HostCanonicalRepositoryLoadRoute[Observation]Key` for
a child canonical name, then observed source/module/package keys and merged
path/effect epochs. Add only an explicit canonical branch: parse with existing
`CanonicalLabel`, revalidate its direct `.bzl` target with
`RootPackageBzlTarget`, use no apparent mapping lookup, and return the
existing current route for self-canonical loads or the existing canonical
child route for other nonroot repositories. Keep `@` and `:` branches,
permission/package gates, cycle identity, error provenance, Need and
observation merging unchanged. Malformed `@@` never falls back to apparent
or root resolution. Do not create an apparent alias, DICE key, side registry,
global cache, rewritten source or command-side fix.

The immutable request/workspace and canonical route input already own repo
mapping, materialization, source policy and effect identity. The changed
resolver produces attempt-local labels; retained dependency manifests keep
their existing canonical label, route and source observations. DICE equality
and invalidation follow those tracked dependencies through source
create/edit/delete/recreate and repository mapping/override changes; no
thread-blocking lock is held across DICE compute (see
`docs/developers/dice.md`). New scratch is short-lived; retained memory is
unchanged. The existing bounded registration diagnostic may still project
LoadLabel errors as incomplete; exact diagnostic text is not selected here.

## Evidence and validation

Make one compact Stage 1 Bazel 9.2 fixture with `fixture.toml` provenance and
local-path-overridden `producer` module. Its generated hub BUILD must contain
a literal nonroot canonical load (from the producer's canonical repo), query
a target declared through that loaded `.bzl`, and have a separate missing-file
negative. The fixture must verify the canonical spelling from the local
module mapping and use independent fresh output bases; no upstream source,
archive or credential is copied. Exact target output and missing-file
message-shape suffice. Keep the fixture below ten small files and below the
orchestration growth checkpoint. Reuse the accepted
`module-extension-use-repo` fixture as a protected regression; do not mutate
it merely to make this packet pass. Broader upstream Bzl visibility tests are
not copied because the visibility owner is unchanged; one focused preserved
gate regression is sufficient.

Focused Rust tests must discriminate `@@canonical` versus mapped `@apparent`,
self-canonical versus foreign canonical child, malformed `@@`, missing
canonical route/source, target validation, preserved relative/apparent loads,
cross-package/visibility behavior, complete observed error identity,
Need/retry and A/B/A source/route restoration. At least one generated BUILD
path test must use the same package loader as the authentic stop, not only a
unit parser. Batch selected tests through exact-selector preflight after
pinned-toolchain JSON `--no-run`; compile Loading and direct Bzlmod/Core
dependents serially. Rebuild `slug_cli_v2` before the authentic normalized-PATH
smoke; check/clean stale `slugd` before and after. Run fresh Bazel generation
and independent replay, protected fixture replay, format, diff, archive and
plan-status checks. Require independent DESIGN and FINAL reviews. No broad
suite, remote transport gate or M7A close is selected. BuildBuddy is available
through normal RC discovery only for later remote work; never inspect or
print `~/.bazelrc`, netrc or auth material.

## Scope and stops

Base clean main `d88bec975`. Allowlist: this manifest and canonical Current
paragraph; Stage 4 owner contract only for accepted invariants;
`app/slug_loading_v2/src/bzl_module.rs` for the sole resolver branch;
`app/slug_loading_v2/src/host_package_load_tests.rs` and
`canonical_repository_load_route_tests.rs` for focused tests; and
`tests/v2_oracle/fixtures/module-extension-canonical-owner-load/` for the
new fixture. The already large `bzl_module.rs` remains the cohesive load
parser/route owner; add no duplicate generic label parser or cross-crate
representation. REPLAN if the source-authentic load cannot use the existing
canonical child route, the accepted source/package/visibility owner needs
new semantic state, the fixture contradicts canonical mapping bypass, or the
authentic build cannot pass this precise LoadLabel edge without a separate
repository effect. A later load or macro stop is a new packet, not permission
to broaden this one.

## Immediate predecessor

WP800 at `d88bec975` accepted selected tracked `module_ctx.getenv` with
redacted raw environment identity. Its seven-row Bazel 9.2 fixture, eight
focused Rust tests and independent reviews passed. The rebuilt authentic
smoke moved from undeclared `RULES_PYTHON_REPO_DEBUG` to this generated-hub
canonical LoadLabel stop. M7A remains partial.

## Acceptance receipt

Independent DESIGN and FINAL reviews returned ACCEPT. The eight-file Bazel
9.2 fixture passed its three mapping, generated-BUILD load and missing-source
rows in generation and independent fresh-root replay (4.36 s and 4.40 s);
the protected `module-extension-use-repo` fixture also replayed. The sole
production branch parses nonroot `@@` without apparent mapping, reuses the
current route for self-loads, enters the existing observed canonical child
route for foreign loads, and fails closed for malformed and explicit-root
canonical forms. Pinned-toolchain JSON `--no-run` compiled Loading, Bzlmod
and Core; exact preflight selected eleven focused tests and all eleven passed
on the final executable. The new tests cover a generated BUILD load,
missing-materialization Need→Complete replay, and stable Complete missing
route error; preserved visibility, source/cycle recovery, relative/apparent
mapping and A/B/A tests passed.

Three older single-`@`/legacy route selectors failed outside the new `@@`
branch: `recursive_mapped_child_need_and_route_errors_stop_before_child_source`,
`route_need_suppresses_effect_and_generated_error_preserves_route_effect_prefix`,
and `selected_parent_routes_generated_load_through_canonical_owner`. Their
unchanged fixtures request uninjected path-observation shards or fail prior
route assertions; a local fixture-repair experiment did not resolve the old
assertions and was fully reverted. These are recorded test debt, not waived
evidence for this branch. The rebuilt CLI authentic normalized-PATH smoke
passed the former `@@rules_python+` LoadLabel stop and reached the separate
`bazel_skylib` `native.package_relative_label` unsupported call (exit 2).
No stale `slugd` remained. Format, diff, archive and plan-status checks
passed. The new error-equality test proves stable recomputation, not
discrimination between different errors. M7A and M8 remain open; no remote
transport or BuildBuddy credential path was used.
