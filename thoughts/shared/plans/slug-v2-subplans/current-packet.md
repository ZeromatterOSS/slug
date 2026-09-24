# Current Slug V2 Work Packet

Packet: WP-7-98-m7a-starlark-rule-direct-none-default-r1
Status: ACCEPTED 2026-09-24; M7A remains partial

## Outcome and compatibility

Admit Bazel 9.2's direct `None`-as-omitted rule-attribute behavior for known,
ordinary public declared Starlark rule attributes. The authentic rules_rust 0.73
`rust_host_tools` generated `rust_toolchain` call supplies five such values:
two labels, two nonempty-default string dictionaries, and `iso_date`, a
string defaulting to empty. All five must take their declared defaults, and
the original extension route must advance beyond its current
`iso_date=None` Loading stop. This is an M7A source-closure packet, not a
root-build or configured-toolchain success claim.

Exact Bazel 9.2 semantics: after known-name validation, direct `None` on an
ordinary public Starlark rule attribute does not set an explicit value, undergo type
conversion, or override its declaration default. A mandatory declaration
still reports missing. A concrete value remains explicit and type checked.
Slug-native: retained `AttributeProvenance` and structural package/DICE
identity, not Bazel's internal bitset or checksum bytes. Unsupported/deferred:
`None` inside `select()` branches or concatenations, callable built-ins such
as `visibility`, private `_` and implicit/built-in attributes, native rule APIs, arbitrary
macro/initializer changes, exact diagnostic wording, and analysis-time
requirements beyond this Loading slice. Those paths retain their existing
behavior with no new parity claim; if the selected source requires one,
scope-correct or REPLAN. Do not infer all-rule or configured parity.

## Source, owner and decision

Pinned Bazel 9.2 `StarlarkRuleClassFunctions.java:1886-1893` passes the
default-true unknown-attribute flag from `BuildLanguageOptions.java:634-641`.
`AttributeProvider.java:204-241,251-327` checks a rule
attribute name, ignores a known direct `Starlark.NONE` before type conversion
and explicit-bit setting, then supplies the declared default or reports a
mandatory missing value. `RuleFactory.java:67-72` treats `name=None`
separately; `BuildType.java:720-723` handles selector-branch `None` through a
different path. The authenticated rules_rust 0.73
`rust/private/toolchain.bzl:664-869` declares `allocator_library` and
`global_allocator_library` as labels, `opt_level` and `strip_level` as
string dictionaries, and `iso_date` as a string. Its generated BUILD writes
all five as direct `None` for the selected stable Rust 1.97.1 host tools.
Bazel 9.2's authentic offline build succeeds. Its query oracle returns the
five declared defaults: `@rules_rust//rust/settings:default_allocator_library`,
`@rules_rust//rust/private/cc:global_allocator_library`, `opt_level =
{"dbg": "0", "fastbuild": "0", "opt": "3"}`, `strip_level = {"dbg": "none",
"fastbuild": "none", "opt": "debuginfo"}`, and `iso_date = ""`. This is not a
string-only or type-null normalization.

`FrozenRuleDefinition::invoke` in
`app/slug_loading_v2/src/package.rs:8026-8190` is the Loading owner. Its
unknown-name loop must run on raw kwargs first. Keep existing raw-presence
checks for implicit/built-in attributes and `name`; after those checks, and
only when `!builtin` and the declaration name is public (does not start
with `_`), treat a direct `Some(Starlark None)` for an ordinary declared
attribute as absent
before the default/mandatory/provenance branch. Reuse the existing schema
default and `AttributeProvenance::Default`, with no invented `None` value in
`CoercedAttributeValue`, no untyped bypass, and no source-specific name
switch. Explicit non-None values still take current coercion, allowed-value,
file/provider and configuration checks. Rule-initializer input and callable
visibility stay in their existing, separately scoped paths.

Keep source/owner configuration structural through existing package
observation and DICE equality; direct None and omission may share normalized
semantic value/provenance, but a changed source or declaration must recompute
and A/B/A restore. No DICE key, locking, repository owner, macro storage,
transport, archive materializer or command-side cache changes. This is a
short branch in the existing cohesive `package.rs` owner; no retained heap
increase or new data structure is needed.

## Evidence and validation

Add a small Stage 1 Bazel 9.2 fixture only for the demonstrated gap: a
Starlark rule with string, string-dictionary and label defaults, and omitted,
direct-None, explicit-empty and concrete-override calls. Use `query attr`
and `labels` to discriminate actual default values from empty, literal
`"None"` and concrete overrides; use `--output=xml` to distinguish
non-explicit omitted/direct-None attributes from explicit empty/concrete
ones. Include separate mandatory-None, unknown-name-None, private-attribute
`_x=None` and wrong-type
negative packages so one invalid BUILD cannot poison positives. Run the
fixture Bazel-only with message-shape/anchored output expectations; this is
Loading evidence, not a claim about Slug query support. Keep outputs/source
provenance host-independent and
self-contained. Reuse the authentic rules_rust direct/extension smoke as
source-derived closure evidence; do not copy upstream files or archives into
the fixture. The existing label-list default may be a local regression
control, but do not claim selector-None or native-attribute parity.

Focused Rust tests in the Loading owner must distinguish omission versus
direct None versus explicit empty/concrete values in both value and
provenance, including a nonempty string/dictionary/label default, mandatory
None, unknown-name None, private `_x=None` exclusion from the admitted
normalization, wrong-type non-None, and source/default A/B/A. The private
negative need only preserve the preexisting disposition; it does not claim
Bazel parity for that unsupported form.
Preserve accepted label-None/default, initializer, visibility and generated
repository Loading tests. Compare all five authentic selected defaults above
against Bazel evidence and require the original extension smoke to pass the
former `iso_date=None` Loading stop; a later separately typed stop is recorded
as next work, while a different earlier stop requires scope correction or
REPLAN.

Run pinned-toolchain JSON `--no-run`, exact-selector preflight and focused
tests with serial Cargo, direct Loading/Bzlmod/Core compile dependents, and
fresh `cargo build -p slug_cli_v2` before any CLI smoke. Clean stale `slugd`
before/after. Run format, diff, V1 archive and plan-status checks. Obtain
independent DESIGN review before Rust edits and FINAL review of the actual
diff. No broad suite or M7A close is selected.

## Scope and stops

Base clean main `7efc2508f`. Allowlist: this manifest, canonical Current
paragraph and Stage 4 owner contract only for accepted changes;
`app/slug_loading_v2/src/package.rs`,
`app/slug_loading_v2/src/host_package_load_tests.rs` with focused tests;
`tests/v2_oracle/fixtures/starlark-rule-direct-none-default/` for the small
oracle fixture. No DICE key, Starlark runtime, Bzlmod, Core, repository
materializer, NativeLink/BuildBuddy configuration, credential, generated
rules_rust file, or unrelated attribute family change. The large Loading
file remains the existing cohesive rule-invocation owner; add one bounded
normalization branch, not a second attribute interpreter. If exact behavior
requires a different semantic owner, a new retained representation, selector
semantics, a changed builtin policy, or unmodeled generated-rule effects,
stop for reviewed scope correction or REPLAN. Normal Bazel RC discovery may
pick up user auth when needed; never inspect or print `~/.bazelrc` or netrc.

## Immediate predecessor

WP797 at `7efc2508f` admitted declared Host OS/architecture factors on the
selected extension owner. Its authentic rules_rust smoke passes the factor
gate and six accepted archive effects, then stops in generated BUILD at
`rust_toolchain(iso_date=None)`. Exact JVM factor bytes and lockfile replay
remain deferred. Two older diagnostic fixtures lacking Host inputs remain a
separate known validation gap.

## Acceptance receipt

Independent DESIGN and FINAL reviews returned ACCEPT. Bazel 9.2's
self-contained fixture passed generation and independent replay in separate
fresh roots, 8/8 commands each. Value queries distinguished omitted/direct
`None`, explicit empty, literal `"None"` and concrete overrides; XML without
defaults established the non-explicit versus explicit attribute distinction.
Four isolated negative packages preserved mandatory, unknown-name, private
and concrete wrong-type boundaries. The source-derived authentic Bazel
rules_rust target also returned the five declared defaults recorded above.

Pinned-toolchain JSON `--no-run` compiled Loading, Bzlmod and Core. Exact
selector preflight found all three new tests; 3/3 new tests and three selected
existing Loading regressions passed. A fresh `cargo build -p slug_cli_v2`
passed before the authentic CLI smoke. The first ambient-`PATH` replay passed
the former generated `iso_date=None` Loading stop but hit the previously
recorded transient `rules_shell` repository `which` limit. A second replay
with the already validated `PATH=/bin:/usr/bin:/usr/local/bin` cleared that
limit and reached toolchain registration row 53: rules_python's
`python_repository.bzl` declares unsupported repository-rule attribute
`_rule_name`. This is the next source-selected Loading boundary, not a
root-build or M7A completion claim. No stale `slugd` process was present
before or after. Format,
diff, V1 archive and plan-status checks passed; no broad suite was selected.
