# Current Slug V2 Packet

Packet: `WP-6-7A-label-file-admissibility-category-parity-r1`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 public dependency-
attribute declaration and configured-prerequisite validation breadth.

Status: terminally accepted; implementation, proof correction, complete
owner/dependent suites, authentic replay, and independent rereview pass.
Initial review required
adding the public reexport owner and structural Host input; the focused
correction returned `ACCEPT`. The corrected candidate measures 214 production
net / 650 gross, 448 proof net / 606 gross, and 1,256 total gross Rust lines. The
earlier 177/613 production and 53/209 proof candidate already exceeded the
original 470-gross production estimate without exceeding its file allowlist or
semantic scope, so the R1 cap-only `REPLAN` corrected the limits without
widening scope; independent focused review returned `ACCEPT`. Terminal
preflight found that two existing owner functions necessarily touched by the
packet already exceed the original absolute 150-line wording; the bounded R2
complexity `REPLAN` below records their cohesion without changing Rust;
independent focused review returned `ACCEPT`. The first terminal implementation
review found no production defect but returned proof-only `REPLAN`: the frozen
identity matrix used separate declarations instead of a same-DICE source
mutation, scalar `allow_single_file` lacked invalid-type rows, and suffix-
specific macro preservation plus aspect/repository rejection were not
discriminated. The bounded correction adds only those proofs inside the frozen
allowlist. Complete deterministic suites pass and independent terminal
rereview returns `ACCEPT`. Base commit
`1e65972a6` terminally accepts compact `attr.flags` ownership and exact
`DIRECT_COMPILE_TIME_INPUT` binding across all five exposing constructors. The
unrelated dirty `app/slug_loading_v2/src/registration_expansion_tests.rs` proof
remains parked at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`;
do not edit or stage it.

## Observable result and compatibility classification

Close Bazel 9.2's complete public label file-admissibility category rather than
patching rules_rust's one `attr.label_list(allow_files=[".rs"])` call.

Exact admitted behavior:

- `attr.label`, `attr.label_list`, `attr.string_keyed_label_dict`,
  `attr.label_keyed_string_dict`, and `attr.label_list_dict` accept named-only
  `allow_files=None|bool|Sequence[str]`; every other constructor rejects it at
  binding;
- scalar `attr.label` alone accepts named-only
  `allow_single_file=None|bool|Sequence[str]`; every other constructor rejects
  it at binding;
- omitted and explicit `None` retain the dependency default of no files and no
  single-artifact projection. Two non-`None` scalar parameters fail before
  publication. A non-`None` `allow_single_file`, including `False` or `[]`,
  independently retains the single-artifact property;
- `True`, `False`, and a sequence retain any-file, no-file, and exact ordered
  suffix-predicate states. Do not sort or deduplicate sequence elements. An
  empty sequence remains distinguishable from Boolean `False` in retained
  schema identity even though both match no direct files;
- ordinary source and generated-file targets are admitted only when the
  retained predicate matches their filename. Rule targets remain eligible;
  provider/rule-class policy is separate. For a suffix predicate, generated
  rule output succeeds when at least one regular output matches or an admitted
  directory artifact is present. A non-no-file single-artifact policy requires
  exactly one files-to-build artifact before suffix testing; and
- dictionary policy follows the label-bearing side: values for
  string-keyed-label dictionaries, keys for label-keyed-string dictionaries,
  and nested values for label-list dictionaries. Existing dependency order and
  `ctx.attr` value shape remain unchanged.

Slug-native admitted behavior:

- suffix matching follows the selected dependency configuration's structurally
  retained Rust Host flavor: case-sensitive for Unix and ASCII case-insensitive
  for Windows. Bazel's Java `regionMatches(true)` behavior
  for unusual non-ASCII Windows suffixes is outside the approved Rust valid-
  Unicode/Host observation class; and
- diagnostics retain Bazel's failure category and decisive attribute/label/
  predicate facts, not HotSpot exception decoration or byte-for-byte wording.

Unsupported/deferred behavior:

- `SKIP_ANALYSIS_TIME_FILETYPE_CHECK`, rule-class filters, validation skipping,
  materializing/dormant dependencies, and other property flags remain fail-
  closed under their existing packets;
- no new File/ctx reflection namespace, query schema reflection, Fileset,
  repository-rule, tag-class, macro, aspect, or subrule surface is inferred.
  Existing accepted conversions must preserve the typed policy; existing
  rejected conversions must continue failing before publication; and
- action families and artifact kinds not already representable by V2 remain
  unsupported rather than approximated. Existing `ActionOutputKind::Directory`
  is sufficient for Bazel's tree-artifact file-type exception.

This is generated binding, retained loading schema, and configured-prerequisite
validation. It is not parser grammar, `set`, a rule implementation, a C++
builtin, or a `cc_common`/`cc_internal` branch. Bazel 9 BCR Starlark remains the
rule-body owner; C++ APIs are ordinary later consumers of the same general
analysis graph.

## Bazel 9.2 authority and pinned evidence

Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a` is sole semantic
authority. Pinned SHA-256 values are:

- `StarlarkAttrModuleApi.java`:
  `af70c851882fa049034184dbb6f6580731cfa738d79dfb8abcf61af176257670`;
- `StarlarkAttrModule.java`:
  `388421c44c623c1c6625fd9f2b059d2a7d1e13b8d45e7c96173f24866a917967`;
- `Attribute.java`:
  `fbe208c37ad4ed88030f874fa6cd8bd5cf2f4aac63f9a01a4ff24ca499c9a6a4`;
- `FileType.java`:
  `f61bea03f7174de152c8cd8dadc78950c8a900eef43b7378213da4c9c6f3609e`;
- `FileTypeSet.java`:
  `44cdd573e6e9b4d3ebf1c8b5e63049914313c9f3c3dc4aabe99f50f7859071c0`;
- `StarlarkRuleClassFunctionsTest.java`:
  `e09c93616e096d639ec69b6b0c6a397a8a36bc8a95fa21b986cb5fc7f8f010aa`;
  and
- `StarlarkRuleContextTest.java`:
  `d195e5d49aae52a92bd3abebfc8de7942aacb252b522cea315985d41277f082d`.

The five/one signature split is fixed by `StarlarkAttrModuleApi.java:303-470,
602-728,771-881,889-1005,1014-1127`. Shared conversion is
`StarlarkAttrModule.setAllowedFileTypes:125-140`; conflict/default/single-
artifact behavior is `createAttributeFactory:431-445`. `FileType` preserves the
ordered suffix list and performs host-sensitive filename suffix matching.
`Attribute` retains both `FileTypeSet` and `SINGLE_ARTIFACT` in immutable
attribute equality/hash. `RuleContext.validateDirectPrerequisiteFileTypes:
1942-1995` establishes source, generated-output, any/no-file fast paths,
singleton validation, and the tree-artifact exception.

Pinned discriminators are
`StarlarkRuleClassFunctionsTest.testAttrAllowedFileTypesAnyFile`,
`testAttrAllowedFileTypesWrongType`, `testAttrWithList`,
`testAttrAllowedSingleFileTypesWrongType`, and `testAttrSingleFileWithList`
around lines 713-829. `StarlarkRuleContextTest` lines 1292-1517 and 1908-2008
cover label-keyed and label-list dictionaries with true/list/false predicates;
its single-file cases around 2539-2615 cover generated output cardinality.
These pinned regressions are accepted evidence for the implementation matrix;
add an oracle only for a demonstrated uncovered ambiguity.

## Current architecture and representation decision

Slug currently retains `allow_files: bool` plus
`Option<AllowSingleFile>` through ordinary and fixed schemas. Boolean direct-
file admission already affects configured-node preparation, while lifted
subrule dependencies carry a validation row. The current form cannot represent
extension-list `allow_files`; it also incorrectly treats
`allow_single_file=False` as absence of the independent single-artifact
property and applies suffix checks only to single-file dependencies.

Replace those two fields atomically with one V2-owned `FileAdmissibility` value
beside `AttributeSchema`:

```text
FileAdmissibility
  file types: NoFiles | AnyFile | Suffixes(Arc<[CompactString]>)
  single artifact: bool
```

The concrete Rust layout may use a six-variant enum to avoid padding, provided
it exposes only semantic methods: `admits_direct_file`, `single_artifact`,
`matches_filename(HostPathFlavor, ...)`, and the exact suffix view. `Suffixes` preserves order,
duplicates, and an empty slice. Clone shares the immutable `Arc`; parser lists
and tuples are evaluator scratch and never survive publication. Derive
`Debug`, `Clone`, `PartialEq`, `Eq`, and `Allocative`; add a layout/clone proof.
Do not retain raw Starlark `Value`, add a second extension list, dynamic map,
set, interner, cache, DICE key, hash, or side table.

Flow the one value through `AttributeDefinition`, generated binder, freeze,
ordinary `AttributeSchema`, fixed/lifted schemas, and
`ConfiguredDependencyValidation`. Remove/replace the obsolete public
`AllowSingleFile` reexport in `slug_loading_v2::lib`; no compatibility alias or
second carrier remains. Ordinary declared dependencies must receive
the same validation object currently used by lifted rows; this closes the
existing source-admission-only gap for scalar/list/dictionary rules without
changing label topology. One shared validator handles direct files and rule
outputs. Use artifact basenames, accept directory outputs before suffix
failure, and keep any/no-file fast paths. Every suffix-bearing dependency
validation obtains `HostPathFlavor` only from the selected dependency
configuration's existing structural
`SlugConfiguration::configured_action_path_flavor()`: carry that optional
phase-scratch copy through source-node preparation and post-compute rule-output
validation, and fail closed when a suffix predicate lacks Host facts. Boolean
any/no-file policies do not consult Host state. Ambient OS reads, `cfg!`,
build-host defaults, and path-string inference are forbidden. No lock crosses
a DICE computation; the policy is immutable retained input already owned by
package/configured-target equality and invalidation, while the selected
structural configuration already owns Host invalidation.

## Buck2 and Zabel guidance

starlark-rust commit `088c75c7e36805df99c3de29062baa95db700b8b` remains the
parser/evaluator/generated-binder substrate. Buck2 parse fixtures demonstrate
the broad Boolean/list call family but provide no Bazel semantic owner to port.
Existing Slug `CompactString`, `Arc<[T]>`, and `Allocative` patterns are the
smallest appropriate retained utilities; the Stage 9 row records intentional
reuse and no new import.

Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance,
not truth. Its `build_rule_declaration.zig` correctly separates non-`None`
`allow_single_file` presence from allowed file types and retains ordered suffix
storage; its captured declaration keeps policy beside the value. Adopt those
ownership lessons. Do not copy its Zig code, arena/pointer lifetime, Boolean-
only `allow_files` limitation, evaluator diagnostics, cache, or compatibility
claim. Bazel 9.2 supplies every behavior above.

## Allowlist, caps, validation, and stops

Production allowlist:

- `app/slug_loading_v2/src/attrs.rs`;
- `app/slug_loading_v2/src/lib.rs` for the one mechanical public reexport
  replacement;
- `app/slug_loading_v2/src/package.rs`;
- `app/slug_loading_v2/src/subrule.rs`;
- `app/slug_analysis_v2/src/subrule.rs`; and
- `app/slug_analysis_v2/src/dice.rs`.

Proof allowlist:

- inline tests in those production files;
- `app/slug_loading_v2/src/host_package_load_tests.rs` for mechanical retained-
  schema assertion migration only;
- `app/slug_loading_v2/tests/build_file_loading.rs`;
- `app/slug_loading_v2/tests/subrule_loading.rs` for mechanical public-carrier
  assertion migration only;
- `app/slug_analysis_v2/tests/starlark_rule.rs`; and
- `app/slug_analysis_v2/tests/subrule.rs`.

Scheduling/status edits may replace this manifest and update canonical Live
Status, Stage 6, and the matching Stage 9 ledger row. Do not touch parser/
starlark-rust, Cargo, rulesets, fixtures outside the named proof files, query,
commands, action construction, `cc_common`, `cc_internal`, or the parked proof.
Corrected production cap is 260 net / 680 gross Rust lines; proof cap is 520
net / 700 gross; total cap is 1,350 gross Rust lines. The correction admits no
new file, behavior, carrier, or owner: the measured 613-gross production delta
is the atomic replacement across existing binder/freeze/validation paths, and
the remaining headroom covers correction rather than adjacent breadth.
`package.rs` and `dice.rs` exceed the physical-size trigger but are existing
sole binder/freeze and configured-dependency/DICE owners; no new module or
ownership boundary is justified for one typed policy. No new or expanded
semantic helper exceeds 150 lines. Two pre-existing functions necessarily
touched by the packet remain above that trigger: `attr_methods` is the one
generated `#[starlark_module]` registration table for the thirteen public
constructors, with independent bounded constructor bodies; and
`ConfiguredNodeAnalysisKey::compute_inner` is the one existing configured-
analysis stage orchestrator whose Need/error order spans attribute, dependency,
toolchain and publication phases. This packet only adds calls to extracted
`unpack_file_admissibility`, `configured_dependency_path_flavor`, and shared
validation helpers plus bounded per-row input selection. Splitting either
existing function here would widen generated method-table or DICE stage-order
ownership without reducing the policy's semantic complexity, so the concrete
cohesion decision is to retain both and require the extracted helpers and every
new test function to remain below 150 lines.

Focused proofs must cover:

1. all five constructors with omitted/None/True/False/list/tuple/empty/
   duplicate/reordered suffix inputs, plus wrong outer and element types;
2. scalar-only single-file binding, both-parameter conflict, either explicit
   None with the other parameter, positional rejection, and keyword rejection
   on every unexposed constructor;
3. the sole new public typed carrier compiles without an `AllowSingleFile`
   alias; final published schema identity distinguishes Boolean false, empty
   suffixes, order, duplicates, and the independent single-artifact bit, with
   A/B/A restoration and shared immutable clone/layout proof;
4. direct source/generated-file suffix success and failure, default/false
   rejection, and true admission under structurally selected Unix case-
   sensitive and Windows ASCII-insensitive flavors, plus suffix-bearing
   missing-Host fail-closed behavior without perturbing Boolean policies;
5. generated rule outputs: matching/mismatching suffixes, one-of-many match,
   directory admission, no/one/multiple single-artifact behavior, and no-file/
   any-file fast paths;
6. one ordinary dependency per dictionary orientation proving the label-
   bearing side is validated without changing `ctx.attr` shape; and
7. existing aspect, macro, subrule, repository-rule, and tag-class controls do
   not silently lose or newly admit policy.

Then run serially: focused owner proofs; complete `slug_loading_v2` library and
integration tests affected by the packet; complete `slug_analysis_v2`; rebuild
`slug_cli_v2`; clean stale `slugd`; authentically replay
`cquery //app/slug_cli_v2:slug`; clean `slugd` again; `cargo fmt --all --
--check`; Cargo metadata; `scripts/v2_archive_status.sh`; `git diff --check`;
cap accounting; and parked-file SHA verification.

Candidate evidence after the proof-only terminal correction:

- the focused five-constructor retained-identity matrix, ordinary source/
  generated/dictionary/platform matrix, configured-subrule diagnostic matrix,
  and directory-exception unit proof pass. Scalar invalid-type, ordered-suffix
  macro preservation, suffix-specific aspect/repository rejection, and one
  same-DICE ordinary-policy A/B/A that restores both configured results and
  errors also pass;
- complete `slug_loading_v2` passes with 552 tests plus one intentional ignored
  realized-BCR fixture when run with one test thread. A prior default-thread
  run exposed only the existing temporary module-name environment race in
  `glob_invalidation`; that test passed immediately in isolation;
- complete `slug_analysis_v2` passes 117 tests;
- `cargo build -p slug_cli_v2`, `cargo fmt --all -- --check`, and Cargo metadata
  pass. No stale `slugd` exists before or after replay;
- rebuilt one-shot `target/debug/slug cquery //app/slug_cli_v2:slug` clears the
  predecessor `attr.label_list(allow_files=[".rs"])` frontier and next fails in
  authentic rules_rust `rust/private/rust.bzl:928` at the general
  `configuration_field(fragment = "coverage", name = "output_generator")`
  category, still before `cc_common` or `cc_internal`; and
- the archive checker reports only its three known non-V2 thoughts paths,
  `git diff --check` passes, and the parked proof remains at SHA-256
  `36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`.

`REPLAN` before changing label topology, provider/rule filtering, selector or
transition semantics, `ctx.file`/query reflection, an action identity, adding a
new artifact kind, accepting another property flag or attribute parameter,
widening a fixed-schema consumer, adding retained raw spellings or a second
collection, adding a DICE key/cache/interner/lock, touching a ruleset/C++/
starlark-rust owner, or exceeding a cap. Independent design review is required
before Rust and independent terminal review before commit.

## Immediate predecessor

Commit `1e65972a6` terminally accepts
`WP-6-7A-attribute-flags-direct-compile-input-category-parity-r1`. The authentic
one-shot replay passes `flags=["DIRECT_COMPILE_TIME_INPUT"]` and stops in the
same rules_rust declaration at `allow_files=[".rs"]`, before a rule body,
configured analysis, `cc_common`, or `cc_internal`.
