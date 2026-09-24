# Current Slug V2 Work Packet

Packet: WP-7-99-m7a-private-repository-rule-attrs-r1
Status: ACCEPTED 2026-09-24; M7A remains partial

## Outcome and compatibility

Admit ordinary one-underscore private `repository_rule` attributes of the
source-selected `attr.string` and `attr.label` kinds. A declaration's typed
default must survive export, module-extension invocation, repository-spec
conversion and `repository_ctx.attr` access. Concrete call-site overrides
remain legal and type checked; direct `None` behaves as omission. The original
rules_rust 0.73 workspace with Bazel's resolved rules_python 1.7.0 must pass
the current `_rule_name` definition-time Loading stop when replayed under
the already validated normalized `PATH`. A later separately typed stop is
acceptable. This is source closure, not Python toolchain or root-build success.

Exact Bazel 9.2 observable slice: public Starlark spelling `_x` for a private
repository-rule descriptor; default and concrete override values appear as
`repository_ctx.attr._x`; `None` (even on an unknown supplied name) is ignored
before lookup, while an unknown concrete name and wrong concrete type fail.
Slug-native: retaining the public `_x` spelling structurally rather than
Bazel's native `$x` storage spelling, and retained DICE/package/repository
identity rather than Bazel checksum bytes. Unsupported/deferred: additional
private kinds, computed/late-bound defaults, nonordinary schema options,
arbitrary name grammar beyond the admitted one-underscore ASCII form,
repository effects not already admitted, Python toolchain analysis, exact
diagnostic wording and root bootstrap. No all-repository-rule parity claim.

## Source, owner and decision

Pinned Bazel 9.2 `StarlarkRepositoryModule.java:75-90` converts descriptor
names and builds attrs. `AttributeValueSource.java:21-29,44-71` maps `_x`
to native `$x`; `RepoRule.java:139-150` indexes the public Starlark name;
`AttributeUtils.java:57-118` skips `None`, type-checks concrete kwargs and
installs defaults; `RepoDefinition.java:40-60` exposes them through `rctx.attr`.
The source-authentic rules_python 1.7.0 BCR `python/private/python_repository.bzl`
(SHA-256 `a977ffc6ed258301330a993864762e2bbb9f64cad5caf1519d02c24e68f228ff`)
declares `_rule_name: attr.string(default = "python_repository")` at line 352,
while `pythons_hub.bzl:186` declares a private `attr.label` default that its
implementation reads. The row-53 stop loads `python_repository.bzl` as an
import; it does not prove that repository was instantiated or downloaded.

`package.rs::repository_rule` is the definition-time owner. Extend only its
name predicate to admit a single leading `_` followed by an ASCII letter and
then ASCII alphanumerics/underscores; keep the existing public grammar and
built-in collision checks. After the existing `attr.*` descriptor conversion,
admit private descriptors only for `AttributeKind::{String, Label}` through
the existing schema checks. Do not special-case `_rule_name`, fabricate native
`$` names, or alter regular Starlark rule/implicit attr policy.

The existing `RepositoryRuleAttribute` and frozen definition/call projection
retain raw Starlark names and typed defaults. `instantiate_call_with_conversion`
already skips `None` before name lookup, type-checks concrete overrides and
keeps only supplied overrides in `RepoSpec`; the existing context resolves an
omitted override from the declaration. Preserve these paths and their typed
Need/error, label-conversion, repository-mapping and publication behavior.
The definition/source observation and the certificate's retained call
definition must distinguish changed private defaults, with A/B/A restoration;
`RepoSpec` stores supplied overrides only and must not be mistaken for default
identity. No DICE
key, lock, new retained representation, ambient source read, command cache or
foreign semantic executor is selected. A retained private descriptor uses
the existing descriptor storage; attempt-local evaluator values and temporary
source roots remain scratch and are released on completion/error.

## Evidence and validation

A Bazel-only fresh-output-base scratch oracle at
`/tmp/slug-repo-private-attr-UZDhCd` already distinguished seven cases:
omitted private string default, concrete string override, direct `None`,
wrong concrete type, unknown concrete name, unknown-`None`, and private label
default read. Add a private label override with a distinct source marker so
the second admitted kind's call-site conversion is discriminated. Add one
small self-contained Stage 1 fixture with source
provenance and deterministic BUILD content to preserve those discriminators.
Use message-shape for diagnostics and exact target-name/value checks; keep
invalid module calls isolated by command mutation or separate minimal fixtures.
Generate and independently replay Bazel 9.2 in fresh roots. This fixture is
Loading/repository-definition evidence, not Slug Query parity. Reuse accepted
repository-effect and module-extension evidence; copy no upstream source,
archive or credential into the fixture. Its new files remain below the
orchestration fixture-growth checkpoint.

Focused Rust tests must replace the old `_private` declaration-rejection
expectation with a malformed-name guard; prove both private kinds' typed
defaults and `repository_ctx.attr` visibility, concrete string and label
overrides, direct
`None`, unknown concrete versus unknown-`None`, wrong type, descriptor
retention across freeze/call/RepoSpec and changed-default A/B/A structural
identity. Preserve public attrs, built-in collisions and unsupported schema
guards. Use pinned-toolchain JSON `--no-run`, exact-selector preflight and
focused owner tests with serial Cargo; compile direct Loading/Bzlmod/Core
dependents, then fresh `cargo build -p slug_cli_v2` before authentic CLI smoke.
Clean stale `slugd` before and after. The authentic replay uses
`PATH=/bin:/usr/bin:/usr/local/bin` with HOME/NETRC unset to avoid the
previously recorded ambient-PATH `which` limit. If it fails before the
`_rule_name` declaration or requires a different owner/effect, correct scope
or REPLAN. Run format, diff, archive and plan-status checks; require
independent DESIGN and FINAL reviews. No broad suite or M7A close selected.

## Scope and stops

Base clean main `50d282a2e`. Allowlist: this manifest, canonical Current
paragraph and Stage 4 owner contract only for accepted invariants;
`app/slug_loading_v2/src/package.rs` for the bounded name/kind gate;
`app/slug_loading_v2/src/module_extension_repository_rule.rs`,
`module_extension_repository_instantiation.rs` and
`repository_rule_context.rs` for focused tests only;
`tests/v2_oracle/fixtures/repository-rule-private-attrs/` for the small
Bazel fixture. The already large `package.rs` remains the cohesive declaration
owner; add no second repository attribute parser. No Starlark runtime,
cross-crate API, DICE key, repository-effect driver, remote transport,
NativeLink/BuildBuddy configuration, lockfile, credential or generated
rules_python source change. If a private default needs new storage/identity,
call-site behavior contradicts the pinned oracle, or the selected source
requires an unmodeled repository effect to pass the definition-time gate,
stop for reviewed correction or REPLAN. Normal Bazel RC discovery may pick
up user auth when explicitly needed; never inspect or print `~/.bazelrc`
or netrc.

## Immediate predecessor

WP798 at `50d282a2e` admitted ordinary public direct-`None` Starlark rule
defaults. Its normalized-PATH authentic smoke passed the former generated
`iso_date=None` stop and reached rules_python's `_rule_name` declaration at
toolchain registration row 53. An initial ambient-PATH `rules_shell` `which`
limit was the previously recorded transient environment boundary, not a new
selected implementation gap. WP798's eight-command Bazel oracle, focused
Rust gates and independent reviews passed; M7A remains partial.

## Acceptance receipt

Independent DESIGN and FINAL reviews returned ACCEPT. The Bazel 9.2 fixture
passed generation and independent replay in separate fresh roots, 8/8 rows
each: private string default, concrete override, direct `None`, unknown-`None`,
private label default/read and concrete label override/read, plus isolated
wrong-type and unknown-concrete negatives. The authenticated rules_python
1.7.0 `_rule_name` and `_rules_python_workspace` source anchors were checked;
the fixture copies no upstream files. The only production change is the
bounded `package.rs` name/kind gate. Existing retained call definitions carry
private defaults; `RepoSpec` continues to carry supplied overrides only.

Pinned-toolchain JSON `--no-run` compiled Loading/Bzlmod/Core in 13.74 s.
Exact-selector preflight found four new/changed tests and two selected
regressions; all six passed. Fresh `cargo build -p slug_cli_v2` passed before
the authentic normalized-PATH smoke. That smoke passed the former `_rule_name`
definition stop and reached a distinct rules_python module-extension
observation error: `repo_utils.bzl:316` requests undeclared environment name
`RULES_PYTHON_REPO_DEBUG`. This does not admit environment behavior, Python
repository effects, toolchain analysis or root-build success. No stale
`slugd` process was present before or after. Format, diff, V1 archive and
plan-status checks passed. The packet took approximately 14 minutes from
draft through final review; no broad suite was selected.
