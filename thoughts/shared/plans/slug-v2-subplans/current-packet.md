# Current Slug V2 Packet

Packet: `WP-6-7A-transition-declaration-setting-identity-r1`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 transition
declaration breadth.

Status: terminally accepted. R1 independent pre-review returned `REVISE` for
five design gaps; the R2 contract and oracle corrected them and focused R2
rereview returned `ACCEPT`. Implementation review found one reversed canonical-
duplicate diagnostic operand, the focused correction matched the pinned oracle,
and correction rereview returned `ACCEPT`. Complete loading passed 559 tests
with one ignored, complete analysis passed 121, the closed command-option proof
passed, and authentic rebuilt rules_rust 0.73.0 replay cleared transition
construction and stopped at generic `rule(cfg = transition)` binding in
`rust/private/rust.bzl:1120-1124`.

Base commit `072c721ad` terminally accepts complete cross-module attribute
descriptor identity. The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked
at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`;
do not edit or stage it.

## Observable result and compatibility classification

Implement the complete Bazel 9.2 regular `transition()` declaration-setting
category instead of permitting the one rules_rust input. A transition may
declare zero or more `inputs` and zero or more `outputs`; every setting is
validated and canonicalized in the `.bzl` module that called `transition()`,
while the spelling supplied by that module remains available to the future
implementation call and return validator.

Exact admitted behavior:

- `implementation`, `inputs`, and `outputs` remain required named-only
  parameters; `implementation` must be callable and each setting collection
  must be a list or tuple of strings;
- both lists admit zero, one, or many entries. A setting is either the exact
  `//command_line_option:<name>` native-option form or an absolute Bazel label;
  relative and malformed labels fail during declaration;
- non-native labels resolve through the defining module's canonical
  repository and apparent-repository mapping. `//pkg:setting` is owned by the
  defining repository, `@//pkg:setting` is owned by the main repository, and
  invisible apparent repositories fail closed;
- only a raw setting with the exact `//command_line_option:` prefix enters
  Bazel's declaration-time native-option name policy. That raw form rejects
  `experimental_*` and `incompatible_*` names except Bazel's two explicit
  `incompatible_enable_{cc,apple}_toolchain_resolution` exceptions. Explicit-
  main and canonical-main spellings of the same canonical label pass through
  ordinary label validation; canonical identity still classifies them as
  native settings for the later execution boundary;
- identical declared strings are rejected as raw duplicates before retained
  construction. Distinct spellings that resolve to the same canonical label
  are rejected as canonical duplicates, independently for inputs and outputs;
- cross-parameter failure order is callable binding, input then output
  sequence/string conversion, raw input then raw output validation, followed
  by canonical output then canonical input duplicate detection;
- each retained list is sorted by canonical Bazel label order and stores one
  typed canonical label plus the original compact spelling. Input/output
  overlap remains valid; and
- live/frozen values, imported descriptors, final rule schemas, equality, and
  same-DICE restoration preserve both complete lists. The existing exact
  input-free/single-output typed Starlark build-setting execution slice
  consumes the stored canonical output directly rather than reparsing its
  declared text.

Slug-native behavior:

- Rust valid-Unicode storage and diagnostics follow the admitted project-wide
  divergence. Bazel labels hold UTF-8 bytes in internal Java strings, so
  `Label.compareTo` orders shared valid-Unicode names by those internal bytes;
  Rust string byte order matches that observable order;
- the internal typed record, package source fingerprint, and DICE equality
  projection are Rust-native. They do not claim Bazel object addresses,
  HotSpot state, configuration checksums, or output-path bytes.

Unsupported/deferred behavior:

- evaluating nonempty inputs, multiple or zero outputs, native-option
  inputs/outputs, the implementation's `attr` struct, patch/split return
  breadth, transition composition, and allowlist policy remain separate
  execution/consumer categories. Analysis must reject such a shape explicitly
  before invoking the implementation; declaration acceptance is not an
  execution-parity claim;
- nonroot/external single-output execution remains outside the previously
  admitted root build-setting slice. Analysis rejects it, and every native or
  otherwise unsupported shape, before build-setting DICE lookup or callable
  invocation; the complete canonical declaration identity remains available
  for a later execution packet;
- Bazel's `--incompatible_disable_transitions_on` command mutation is not yet
  an admitted Slug command option. The default-empty semantics list is exact;
  a nonempty command request remains unsupported and fails at the closed
  native-command boundary before Starlark evaluation;
- rule-level `rule(cfg = transition)` attachment, including the authentic
  rules_rust consumer, is the immediate next consumer category after replay;
- `exec_transition`, `analysis_test_transition`, exec groups, and native
  built-in transition families remain separately typed categories; and
- no parser grammar, `set`, rule body, ruleset, provider/aspect execution,
  `cc_common`, `cc_internal`, C++ rule, action, or BCR special case is added.
  Bazel 9 BCR Starlark remains the owner of every rule body.

## Bazel 9.2 authority and evidence

Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the sole semantic
authority. Pinned source SHA-256 values are:

- `ConfigGlobalLibraryApi.java`:
  `0e6fe8335b206fbc4bfbb9075cea091a02544b5f3f35f60d9d233061d445afe4`;
- `ConfigGlobalLibrary.java`:
  `3eccfc67f78ab57b575c5b080ef83623efc8ba57eb9f246e6336d46b3c13a2d1`;
- `StarlarkDefinedConfigTransition.java`:
  `427a16020eb158943b4073981b1f0701b75ebd85d28816f3a3b6415afcb9a22b`;
- `StarlarkTransitionTest.java`:
  `ff8a91d92783bdfa3cbbd222ab4809183cc18517a08d6b06cd3be19e47c49f22`;
- `StarlarkAttrTransitionProviderTest.java`:
  `607f6f12b6fbc343a3a423f7ab99f25eef40dfb05e1b1abaf948245b6baca7d7`;
  and
- `StarlarkRuleTransitionProviderTest.java`:
  `9b7e78408513f0d989d76fb84bed45093333dbb0d066f737b01e49035e4ae3bb`;
- `BuildLanguageOptions.java`:
  `b01e106ef0ff7af458766248bce7799b49c0f54fc14d023a8297aeb7dbfb44e5`;
- `StringEncoding.java`:
  `6d82718c6f33f676e5fbc4388916fe3e3825f95b5d22daf7b77be1b6e754d983`;
  and
- `Label.java`:
  `4ec889fcfd907fd593d4b22905331f77ad15093fe738215a1e63198de35fa774`.

The API declares three required named-only arguments. Binding and sequential
casts validate the callable, inputs, then outputs. `ConfigGlobalLibrary`
performs raw input then output validation, applies native-name policy only to
the exact raw prefix, checks visibility, and rejects exact spelling duplicates.
`StarlarkDefinedConfigTransition` canonicalizes outputs before inputs, applies
the semantics-provided disabled-option names, resolves with the defining
label/package/repository mapping, rejects canonical aliases, and stores
canonical-label-sorted maps whose values are the original spellings.
`RegularTransition` equality includes inputs, outputs, and implementation.
The upstream tests cover output-only and input/output execution, raw and
canonical duplicates, malformed/nonconfigurable/disallowed options, missing
outputs, patch/split returns, rule attachment, attr attachment, and
composition. Only declaration behavior and the already-admitted narrow
execution slice are selected here; the other tests name later categories.

A fresh ephemeral Bazel 9.2 oracle at
`/tmp/slug-transition-declaration-oracle.2tq4zD` supplies the missing public
behavior evidence. Its successful cquery constructs an empty transition and a
two-input/two-output transition declared in reverse order. It prints
`TRANSITION_INPUTS=//producer:alpha,@//producer:zeta`, proving canonical sort
with original spellings, and analyzes `//:subject`. The same loaded module
constructs raw-policy-bypassing `@//command_line_option:experimental_alias`
and `@@//command_line_option:incompatible_alias` declarations successfully.
A locally overridden
external module loaded as `@mapped_dep` declares `inputs = ["//:flag"]` and
`outputs = ["//:flag"]`; cquery prints `EXTERNAL_SETTING_KEYS=//:flag` and
analyzes `//:external_subject`, proving definition-repository ownership. A
BMP-private-use/supplementary target pair prints the stable order
`//producer:,//producer:𐀀`; this distinguishes Bazel's internal UTF-8 byte
ordering from Java Unicode UTF-16 order and matches Rust `str` ordering.

Focused query failures record exact declaration diagnostics and precedence:

- `duplicate transition input '//producer:alpha'`;
- `Transition declares duplicate build setting '//producer:alpha' in INPUTS
  (specified as '//producer:alpha' and '@//producer:alpha')`;
- the corresponding raw and canonical `output`/`OUTPUTS` diagnostics;
- malformed `not:a` with the native-option-prefix guidance;
- invisible `@missing//:setting`; and
- rejection of `//command_line_option:experimental_example`.

Four dual-invalid rows prove callable failure before element casting, input
element casting before output casting, raw input duplication before raw output
duplication, and canonical output duplication before canonical input
duplication. `//command_line_option:forbidden_by_semantics` succeeds with the
default-empty semantics list and fails under
`--incompatible_disable_transitions_on=forbidden_by_semantics` with
`Option 'forbidden_by_semantics' is not allowed in transitions INPUTS.` Slug
does not add that command option in this packet; its closed option catalog is
the fail-closed regression.

The final oracle hashes are
`ccc6816d13b07faa837ebe3372df0371bf57bd30059aad6fd619c3a10fcd352c`
(`MODULE.bazel`),
`13bfc5d059356348951410cc2995c7b0b021232a9151afd39df65b90bda96761`
(root `BUILD.bazel`),
`c3c521e41b0c2072ec12b00a2583b43195150f69eb5c16cdfc8015a6cb517eb6`
(`producer/defs.bzl`), and
`b2fdd81325bc66ca05340e3db209de924665749b30254b33b90461521cb7a7e7`
(`dep/defs.bzl`). Bad-case source hashes are
`92839c51c8adfabd808e4daf92329686dc541918bbaeb86299ab1e96c064f618`,
`9858486e6fd68d8bc03020074f3bdc681caad93afc02be6401ce4125c38b65c7`,
`1f095f7ad2ff5abd7e40dd9bdbb44ddadc043a04681e24560bc8dc9dabca1b04`,
`acf25ded5edfab8ce0c94ad06d1daa09b99e47fc8175898987d14e638b3d5e91`,
`683bd940ef81068badd432863d0a2c9f2a27d86977593f9a503be306d0670a77`,
`3d6e6736897917e6bc7fc51efacb015c5481081830375c942b8a44e40e4c418e`,
and
`b41e53cbaaf59e95bb74dc41f7d65808932818670cb79b6f4f13e4d55097d629`,
`944981e6d865d8d68d1f4c94597f602e26b2c2aae42193bf3a3988401b8b82a6`,
`b81445623f2c2d70168b5589a05f211575fde58278a0ddbae329098f0042d24b`,
`fe409f584c4095b82e37bf1dca733ee88a76696e6e89ba42ac3b4609ef37539b`,
`db833d4faf791359c520d23f564a35b06a696aab1510114908806595203ec01f`,
and
`7e8a2de8506684322cc2bc47cadfe7f2978bb9aba4d78baea9104b54c5d59622`.
The fixture remains ephemeral because focused Rust tests can encode every
selected discriminator without adding a permanent workspace.

## Learned Slug facts and architecture decision

Slug currently has two scalar transition owners. Loading-time
`TransitionDefinitionGen<Value/FrozenValue>` retains an implementation and one
raw `CompactString` output. Final `attrs::TransitionDefinition` retains the
frozen implementation and the same raw output; equality compares only that
string. `transition()` rejects every shape except `inputs = []` and one direct
main-repository output. Analysis reparses the string as a root canonical label,
loads one of the already-admitted integer, Boolean, string, string-list, or
string-set setting declarations, calls the implementation with two `None`
arguments, and accepts exactly one matching dictionary entry.

Introduce one V2-owned `TransitionSetting` record with canonical
`CanonicalLabel` identity and original `CompactString` spelling. One small
transition module owns absolute setting resolution, raw-only native-option
policy and the two-phase cross-parameter validation order; it reuses
`CanonicalLabel::bazel_natural_cmp` for the already-accepted internal-byte
sort. Both live/frozen transition
values and final loading definitions retain `Arc<[TransitionSetting]>` inputs
and outputs. Attribute descriptor projection and rule-schema lowering clone
only those arcs. Analysis uses the canonical label and declared spelling from
the sole output only after checking `inputs.is_empty()`, `outputs.len() == 1`,
the output is nonnative, and its canonical repository is root. Every rejected
shape fails before build-setting lookup or callable invocation.

Do not retain raw and canonical maps, an evaluator dict, source text, a second
execution schema, or a reconstructed repository mapping. Do not reparse a
setting after declaration. Do not make declaration support silently activate
unimplemented execution shapes.

## Lifetime, memory, incremental ownership, and peer guidance

The transition call is the natural producer. Canonical settings and caller
spellings become immutable module values, then immutable rule-schema values.
The existing frozen heap owns the callable; referenced-module heaps and the
package's `load_fingerprint` own its lifetime and source-derived semantic
invalidation. Input/output slices participate structurally in schema/package
equality. No DICE key, compute, lock, request overlay, filesystem observation,
cache, async task, or shutdown path changes.

Each setting owns one already-compact label plus one `CompactString`; each list
is a shared immutable `Arc` slice with `Allocative`. Lists are expected to be
small, so construction may use evaluation-scratch `Vec`/`SmallSet` before one
final allocation. No global interner, retained hash map, side table, duplicate
string vector, or deep clone is justified.

Clean Buck2 commit `088c75c7e36805df99c3de29062baa95db700b8b` and
`app/buck2_transition/src/transition/starlark.rs` SHA-256
`ad2e47beeba7fbd54ba77d6a518da78b99b63a36bae7db86e9ca620559e19b76`
are concept/runtime guidance only. Buck2 retains one frozen transition object,
an `Arc` identity, compact maps, `Allocative`, and the callable, but its
platform/refs/attrs semantics are not Bazel's setting transition API and no
code is copied.

Clean Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` and
`build_rule_declaration.zig` SHA-256
`f2221daad6d0ad61177d860e58faf3ade1bb249cce9789d7150f22bc18804fcd`
are peer architecture guidance only. Its `TransitionDefinition` keeps typed
input/output slices with declaration identity and shares one captured object
across rule/attribute consumers. Slug adopts the declaration-owner/single-
object principle, not Zabel's allocator, ordinals, or semantic conclusions.

## Implementation boundary, caps, and proofs

Production allowlist:

- `app/slug_loading_v2/src/lib.rs`;
- new `app/slug_loading_v2/src/transition.rs`;
- `app/slug_loading_v2/src/attrs.rs`;
- `app/slug_loading_v2/src/package.rs`; and
- `app/slug_analysis_v2/src/dice.rs` only for the narrow shape check and
  canonical-output consumption.

Proof allowlist:

- `app/slug_loading_v2/tests/build_file_loading.rs`;
- `app/slug_loading_v2/src/host_package_load_tests.rs`; and
- `app/slug_analysis_v2/tests/starlark_rule.rs` only for fail-closed shape and
  preserved narrow execution evidence; and
- `app/slug_configuration_v2/src/native/tests.rs` only for the closed
  `incompatible_disable_transitions_on` command-option regression.

Plan/status allowlist is this manifest, the canonical plan, the Stage 6 owner
plan if the review identifies a reusable decision, and the Stage 9 ledger.
Proposed caps are 320 net / 440 gross production Rust lines, 420 net / 600
gross proof Rust lines, and 1,040 total gross. No function over 120 lines is
expected.

`package.rs` is 9,057 lines and mixes many loading declarations, so the
complexity trigger applies. Setting parsing, policy, ordering, and retained
record ownership go in the new cohesive transition module; `package.rs` keeps
only Starlark value/globals wiring. `attrs.rs` remains the final schema owner.
The 36,563-line host test module is already the recursive import/mapping and
same-DICE harness; add only the mapping/import lifecycle discriminator there.

Focused proofs must cover:

1. empty and multi-entry input/output declarations, reverse-to-canonical
   sorting, original spelling retention, native option identity, raw native
   versus explicit/canonical-main aliases, input/output overlap, and
   callable/list/tuple/string/named-only binding;
2. raw duplicates and canonical aliases independently for inputs and outputs,
   malformed/relative labels, invisible repositories, experimental and
   incompatible option policy including the two exceptions, package recursion
   rejection through the shared label grammar, four dual-invalid rows proving
   cross-parameter precedence, and the default-empty/unsupported-command
   `incompatible_disable_transitions_on` boundary;
3. main, external defining-repository, explicit-main, canonical, and mapped
   apparent label identity across direct import and re-export;
4. live/frozen descriptor projection and final schema equality with different
   input/output identities, reorder-equivalent canonical slices, source A/B/A,
   and original callable pointer preservation, plus the BMP/supplementary
   internal-byte ordering discriminator;
5. an explicit analysis failure before DICE child lookup or invocation for any
   nonempty-input, nonsingle-output, native-output, or external-output shape,
   plus unchanged input-free/single-root-output execution for every already-
   admitted typed Starlark build-setting kind using canonical identity; and
6. authentic rebuilt rules_rust replay clearing transition construction and
   stopping at the next generic rule-level `cfg` or earlier independently
   demonstrated frontier, never at parser, `set`, `cc_common`, or C++ special
   handling.

Validation is serial: focused loading tests, complete `slug_loading_v2`,
focused and complete `slug_analysis_v2`, `cargo fmt --check`, metadata,
`git diff --check`, archive status, pinned source/hash checks, clean Buck2 and
Zabel checks, parked-file hash, `cargo build -p slug_cli_v2`, stale-`slugd`
cleanup, and authentic replay. One independent terminal review is required
because this changes retained cross-crate identity.

`REPLAN` before implementation if exact declaration identity requires a new
DICE owner, repository mapping reconstruction, evaluator-borrowed retained
value, global registry/interner, or general label-policy widening. `REPLAN`
during implementation if the existing admitted narrow execution cannot fail
closed without implementing input/native/split evaluation, if ordering cannot
be represented without a second retained collection, if the production cap is
exceeded, or if replay reaches a contradiction with Bazel 9.2 source/oracle
evidence.

Residual risk is deliberately explicit: this packet prevents representation
churn for later transition execution but does not implement that execution.
The next replay frontier must select the complete rule-level transition
attachment/consumer category rather than a rules_rust-only `cfg` parameter.

## Immediate predecessor

Commit `072c721ad` terminally accepted
`WP-6-7A-cross-module-attribute-descriptor-identity-r1`: one complete transient
live/frozen descriptor projection serves all six declaration consumers and
preserves provider, aspect, and transition pointers. Complete loading passed
556 tests with one ignored, analysis passed 120, independent terminal review
returned `ACCEPT`, and authentic rules_rust replay advanced to the transition
declaration now selected by this packet.
