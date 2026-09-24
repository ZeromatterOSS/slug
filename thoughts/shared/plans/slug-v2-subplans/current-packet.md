# Current Slug V2 Work Packet

Packet: WP-7-120-m7a-config-exec-positional-group-r1
Status: ACCEPTED; independent DESIGN and FINAL reviews ACCEPT; M7A remains partial

## Outcome and compatibility

Admit one positional `exec_group` argument to Loading's `config.exec`, while
preserving zero-argument and named forms. The rebuilt authentic `slug build
//:probe` in `/tmp/slug-wp797-authentic-2YQmSt` must clear rules_python's
`config.exec(value)` extra-positional-argument stop at
`attr_builders.bzl:307`. A later stop is a separate packet.

Exact Bazel 9.2 slice: `config.exec()`, `(None)` and `(exec_group=None)` select
the default execution group; a positional or named string selects the same
named execution group. Empty string is accepted at declaration and retains
its distinct name, though an undeclared empty group fails later in Bazel
analysis. Slug-native: diagnostics, structural transition/schema identity
and admitted configured group behavior. Unsupported/deferred: any wider
configuration or output-identity parity, new execution-group semantics and
other rules_python behavior. Two positional arguments, duplicate binding,
wrong type and unknown keywords remain errors; BUILD-context access remains
denied.

## Source, owner and decision

Pinned Bazel 9.2 HEAD `8220c6198837d5c13d53fea211cf3282aa12408a`:
`StarlarkConfigApi.java:170-191` declares an optional string-or-None
`exec_group` as named without disabling positional binding;
`StarlarkConfig.java:68-72` selects default or named
`ExecutionTransitionFactory` accordingly. Upstream
`StarlarkExecGroupTest.java:278,336` uses positional strings. Offline,
ignored-RC Bazel 9.2 oracle `/tmp/slug-wp820-config-exec-U2ac43`
accepts zero-arg, positional/named None, positional/named `"test"`, and
positional empty string at declaration. A configured platform discriminator
produces execution-platform A for default forms and B for either `"test"`
form; undeclared empty group fails analysis. Two args, integer and unknown
keyword fail at declaration. Oracle runs disabled downloads and used isolated
output bases. Authenticated rules_python 1.7.0
`python/private/attr_builders.bzl:193-205,280-307` turns `cfg="exec"` into
`value=None`, then calls `config.exec(value)` positionally. The stack does
not identify one unique attribute, only that builder call.

Slug `app/slug_loading_v2/src/package.rs:7675-7685` already parses
`Option<NoneOr<&str>>` and returns `StarlarkExecTransition(Option<CompactString>)`;
only `#[starlark(require = named)]` refuses the positional form. Remove that
binding restriction, preserving the typed conversion and existing default vs
named value. `set_attribute_cfg` at `:6874-6895`, frozen schema and sparse
named-exec transition references already retain the distinction. Existing
source/manifest owner and frozen rule schema own equality, invalidation,
export/replay, Need/error and A/B/A restoration; Analysis consumes the same
retained field as the already-admitted named form. No DICE key, retained
representation, cache, lock or Analysis implementation change. The argument
binding is evaluator scratch; the produced transition still freezes into the
existing schema.

## Evidence and validation

Extend the colocated execution-group Loading test to demonstrate pre-fix
`config.exec(None)` failure, then assert zero/positional/named None equivalence
and positional/named `"test"` equivalence in frozen attributes and rule
schema. Include `config.exec("")` as a distinct retained name at Loading.
Protect too many/duplicate/wrong-type/unknown-keyword and BUILD-context
rejections. Reuse the accepted existing named exec-group configured test as a
protected downstream control; no new Stage 1 fixture. Do not infer broad
configuration parity from the platform oracle.

Resolve pinned-nightly Cargo; compile selected Loading lib tests with JSON
`--no-run`, exact nonignored selector preflight, and serial focused/protected
execution. Direct Analysis/Core/CLI dependent compile and
`cargo build -p slug_cli_v2` precede one normalized-`PATH` authentic smoke.
The roughly 35-second smoke is necessary to prove the selected transitive
rules_python closure advances. Check/clean `slugd` before/after; run Rust
format, diff, plan and archive checks. Independent DESIGN and FINAL reviews
cover the changed public call boundary and retained-value equivalence. No
broad suite or remote transport; never inspect BuildBuddy authentication.

## Scope and stops

Base clean main `a3fa5107c`. Allowlist: this manifest, canonical Current
paragraph, compact Stage 4 owner note after acceptance; production
`app/slug_loading_v2/src/package.rs` only; colocated tests in that file or
focused `app/slug_loading_v2/src/host_package_load_tests.rs`. Rough growth
1 production/40 proof lines is a review trigger, not authority to alter
transition representation. `package.rs` is large, but its existing method
is the only binding owner; extracting it would add no cohesion. REPLAN if
positional and named values differ in Bazel, if the binding change cannot
preserve named calls or reject duplicates, or if the selected builder passes
a value other than None/string. A later stop cannot widen this packet.

## Acceptance receipt

Independent DESIGN and FINAL reviewers ACCEPT the one-argument binding
correction. The selected Loading regression first failed on unmodified
production at positional `config.exec(None)` with the authentic extra-argument
error (exit 101). After removing only the named-only annotation, pinned-nightly
JSON Loading `--lib --no-run` compiled; exact preflight selected one
nonignored test, which passed. It proves default/positional/named None
equivalence, positional/named string equivalence in frozen attribute and
sparse rule-schema identity, distinct empty-string retention, and arity,
duplicate, type, unknown-keyword and BUILD-context rejections. A protected
named-exec configured Analysis integration test compiled via JSON `--no-run`,
passed exact preflight and passed execution. Direct Analysis/Core/CLI
`cargo check` and CLI rebuild succeeded. The normalized-`PATH` authentic
smoke clears `config.exec(value)` and now stops separately at protobuf
`configuration_field("proto", "proto_compiler")` in `protobuf.bzl:81`
(exit 2, about 34 seconds). Pre/post checks found no `slugd`; Rust format,
diff, plan and archive checks pass. No remote transport or BuildBuddy
credentials were used. Packet wall time from the accepted predecessor
through final review was about 11 minutes; selected Loading and Analysis
test execution totaled under 0.1 second, with Cargo preparation and the
authentic smoke dominating validation time.

## Immediate predecessor

WP819 accepted at `a3fa5107c`: empty scalar-label aspects now freeze no
attachment and clear their keyword stop. The authentic build next reaches
this independent `config.exec(value)` binding stop.
