# Current Slug V2 Work Packet

Packet: WP-7-118-m7a-label-keyed-dict-empty-aspects-r1
Status: ACCEPTED; independent DESIGN and FINAL reviews ACCEPT; M7A remains partial

## Outcome and compatibility

Admit explicit `attr.label_keyed_string_dict(aspects=[])` in Loading, as the
same zero-attached-aspect declaration as omitted `aspects`. The authentic
rebuilt `slug build //:probe` in `/tmp/slug-wp797-authentic-2YQmSt` must
clear rules_python's `label_keyed_string_dict` unexpected-keyword error at
`attr_builders.bzl:758/89`; a later stop is a separate packet.

Exact Bazel 9.2 slice: an explicit empty list attaches no aspect to dictionary
key dependencies, like omission. Slug-native: diagnostics and existing
structural Loading/schema identity. Unsupported/deferred: singleton/multiple
nonempty dictionary-key aspects and their configured propagation, empty tuple,
broader attr-family aspects, and exact Bazel configuration/output identity.
Nonempty input must fail closed, never be accepted then dropped. Direct
`aspects=None` and wrong-type inputs remain errors.

## Source, owner and decision

Pinned Bazel 9.2 HEAD `8220c6198837d5c13d53fea211cf3282aa12408a`:
`StarlarkAttrModuleApi.java:987-1000` declares `aspects` as a sequence
defaulting to `[]`; `StarlarkAttrModule.java:969-1019` forwards it through
the shared factory, whose `:498-510` loop attaches one aspect per element.
Fresh ignored-RC/offline query/build oracle
`/tmp/slug-wp818-label-keyed-aspects-1MOAxI` accepts omission and `[]` with
the same no-aspect configured result; singleton and two distinct aspects
produce distinguishable nonempty results, while direct `None` and a string
element fail. This packet does not import the nonempty behavior. Authenticated
rules_python 1.7.0 `python/private/attributes.bzl:377-406` omits aspects for
`config_settings`; `python/private/attr_builders.bzl:734-770` supplies `[]`
and `:86-89` forwards it. Its builder test at
`tests/builders/attr_builders_tests.bzl:216-225` asserts the empty default.

Slug `app/slug_loading_v2/src/package.rs:7233-7272` omits the named
`aspects` parameter entirely, so the evaluator rejects the keyword before
the method body. Add only this parameter, and explicitly validate omitted or
list-typed empty input as a no-op; reject nonempty and malformed input. Do
not call the label-list aspect parser and drop its output: it accepts a
singleton that has real Bazel semantics. The existing
`AttributeDefinitionGen.attached_aspect: Option<V>` and frozen rule schema
remain `None`, with existing dictionary kind/default/provider/coercion
unchanged. Loaded Bzl source/manifest and frozen rule-schema owners already
carry equality, invalidation, export/replay, Need/error and A/B/A restoration.
No DICE key, retained representation, cache, lock, Analysis or Query change;
the new argument is evaluator scratch only and no semantic aspect fact is
discarded after acceptance.

## Evidence and validation

First add a focused Loading regression that calls
`attr.label_keyed_string_dict(aspects=[])`, asserts the frozen schema has no
attached aspect and demonstrates the pre-fix unexpected-keyword failure.
Include omitted, direct `None`, nonempty singleton and wrong-type controls;
preserve existing dictionary default/provider/label-resolution tests. Reuse
the offline oracle without Stage 1 fixture growth. No configured aspect
behavior is tested or claimed for accepted Slug input.

Resolve the pinned-nightly toolchain; compile the selected Loading lib test
binary with JSON `--no-run`, exact nonignored selector preflight, then run
focused and protected tests serially. Direct Core/CLI compile and
`cargo build -p slug_cli_v2` precede one normalized-`PATH` authentic smoke.
The roughly 40-second smoke is required to show the selected transitive
Loading stop clears; smaller focused tests cannot establish that closure.
Check/clean `slugd` before/after. Run Rust format, diff, plan and archive
checks. Independent DESIGN and FINAL reviews cover the new named API
boundary. No broad suite or remote transport; never inspect BuildBuddy
authentication.

## Scope and stops

Base clean main `a89845c02`. Allowlist: this manifest, canonical Current
paragraph, a compact Stage 4 owner note after acceptance; production
`app/slug_loading_v2/src/package.rs` only; focused tests in
`app/slug_loading_v2/src/host_package_load_tests.rs`. Rough growth 12
production/45 proof lines is a review trigger, not permission to broaden
dictionary aspects. `package.rs` is large, but the existing method is the
sole attr-signature and semantic owner; a new abstraction would add no
cohesion for this no-op case. REPLAN if the selected builder forwards a
nonempty list, if empty input is not omission-equivalent in Bazel, or if
Slug's frozen schema cannot preserve the empty fact. A later error cannot
widen this packet.

## Acceptance receipt

Independent DESIGN and FINAL reviews ACCEPT the empty-only API boundary.
The new selected regression first failed on clean production with the exact
`aspects` unexpected-keyword diagnostic (exit 101). After the named argument
and explicit empty-list guard, omission and `[]` both freeze no attached
aspect; direct `None`, scalar, wrong-member list and exported singleton
remain rejected. Pinned-nightly JSON Loading `--lib --no-run` compiled; exact
preflight selected four nonignored tests, and all four passed serially. They
cover the new slice, existing dependency schema, generic aspect rejection and
aggregate dictionary default-label mapping/collisions. Core/CLI `cargo check`
and rebuilt CLI passed. The normalized-`PATH` authentic smoke cleared the
dictionary-key keyword error and stopped separately at `attr.label(aspects=[])`
in rules_python's `attr_builders.bzl:615/89` (exit 2, about 31 seconds).
Pre/post checks found no `slugd`; Rust formatting, diff, plan and archive
checks pass. No remote transport or BuildBuddy credentials were used. Packet
wall time through final review was about 7 minutes; four final tests ran in
under 0.1 second combined, with Cargo preparation and the authentic smoke
dominating elapsed time.

## Immediate predecessor

WP817 accepted at `a89845c02`: empty `attr.label_list(aspects=[])` now
freezes no attached aspect, clearing its cardinality stop. The authentic
build next reaches this independent dictionary-key keyword stop.
