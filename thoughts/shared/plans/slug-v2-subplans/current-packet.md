# Current Slug V2 Work Packet

Packet: WP-7-164-m7a-gcc-link-param-files-r1
Status: accepted; independent DESIGN/FINAL ACCEPT; M7A remains partial

## Outcome and compatibility

Advance unchanged authentic Linux Rust-toolchain cquery past the confirmed
GCC_QUOTED refusal at cc_internal.get_link_args/finalize_link_action.bzl:352.
Baseline5cb3faef4 (WP863). Reuse immutable LinkArgs, feature/variable expansion,
retained Args recipes and virtual parameter-file publication. Append Gcc to
RetainedParamFileFormat and admit GCC_QUOTED only at LinkArgs construction;
public Args.set_param_file_format and legacy ParamFileFormat unchanged. Windows
stays explicitly refused; no Windows implementation or execution work.

Pinned Bazel9.2 8220c619 GccParamFileEscaper.java:37-60 prefixes backslash to
exactly single quote/double quote/backslash/ASCII space/tab/CR/LF/FF/VT, empty
argument becomes two single quotes; all other characters remain unchanged.
GccParamFileEscaperTest.java:32-51 covers each character and unchanged shell-
looking ${filename%.c}.o. ParameterFile.java:83-99 writes one LF per argument,
empty sequence no bytes; CppLinkActionTest.java:1048-1071 pins feature-selected
GCC format. Exact ASCII escape algorithm/order/duplicate/trailing-LF and format
selection; inherited Rust valid-Unicode/UTF-8 representation is Slug-native,
arbitrary Bazel legacy byte/Latin-1 strings remain deferred (StringUnsafe:85,
StringEncoding:25-86). No shell escaping/Unicode-whitespace approximation.

## Existing semantic owners and bounded scope

RetainedArgsRecipe.write_format and publication_eq_with own format identity,
even if safe input happens to render identical bytes. Existing immutable calls
and configured/source DICE keys own restoration; docs/developers/dice.md applies.
render() keeps ordinary argv unchanged. render_param_lines owns ArgsWrite and
virtual file encoding; spawn_command_line:130-135 already owns exact virtual
bytes/path/spill order and REAPI projection. Append enum variant preserving
prior ordinals; add bounded gcc escape sibling beside existing shell_escape.
No new retained representation/container/clone/cache/key/lock/identity/sink or
serializer side store. Encoding uses existing phase scratch String/Vec pipeline,
with existing release/publication boundaries. Utility reuse skill + matching
Stage9 generic Args row apply; no donor import or performance claim.

Inherit corrected WP863 contract at5cb3faef4 current-packet.md: original traced/
freezable variables, genuine named inputs, None-lazy/string-eager construction,
first literal placeholder and shared exactly-one-%s validator, filtering before
expansion, no-placeholder Shell default, module-freeze/mutation refusal. Action
registration snapshots and later sink error ordering remain Slug-native; upstream
execution-time expansion, nested structural classification/hash and multiple-
placeholder flags remain deferred. GCC admission must not change those boundaries.

Allowlist: BuildAPI src/actions/spec.rs narrow enum/renderer/helper and
 tests/actions.rs focused existing format/equality/forced-file cases plus pinned
escape regression; Loading src/cc_variables/link_args.rs and its tests.rs;
Analysis tests/cc_feature_expansion/link_args.rs existing fixture only;
REAPI tests/reapi.rs existing forced-parameter helper/test if needed for exact
CAS/Merkle bytes; manifest/canonical and compact existing Stage6/Stage9 notes.
No core/Analysis production, Cargo dependency, new action family or copied public
fixture/materialization. Large spec.rs/actions.rs receive bounded same-owner
format hooks; no unrelated extraction. Estimate production below40 lines, proof
net below150 via parameterization. Independent DESIGN before edits, FINAL after.

## Validation and decision boundaries

Pinned encoder vectors cover all nine escapes, empty argument vs empty sequence,
embedded LF, duplicates/order, literal $%{};, NUL/non-ASCII whitespace/UTF-8 text
in inherited representation; raw argv unchanged. Existing recipe/write and
forced-spill tests gain Gcc, ensure ordinal/type distinction and inequality with
Multiline for safe equal bytes. Loading existing mode matrix admits Gcc while
Windows/invalid formats stay refused; eager expansion/policy/filter/no-placeholder
and mutation/freeze/GC invariants protected. Analysis reused root/subrule fixture
runs GCC mode with forced bytes through both-route same-DICE source/config/exec
restoration. REAPI existing virtual parameter-file topology/digest regression
must inspect exact escaped bytes/CAS digest, not only count/spawn argv.

Root serial pinned-nightly Cargo JSON BuildAPI actions/Loadinglib/Analysis
starlark_rule/REAPI named test prep60s each, batch exact nonignored preflight and
focused runtime belowfewseconds; protective subrule reuse only if production
inputs unchanged, Core direct-dependent check and CLIrebuild before authentic.
Unchanged originalthreehash fixture supervised180s cap justified by77s sourceprep;
actual process checks/cleanup; scoped rustfmt/diff/archive/status. Operation caps
are not user ceilings. REPLAN if existing recipe format cannot own GCC identity/
bytes, new semantic/action/cache owner is required or native string encoding
violates the declared inherited class. Routine selector/compiler fixes remain.

## Immediate predecessor

WP8635cb3faef4 accepts immutable LinkArgs None/UNQUOTED/SHELL slice, corrected
DESIGN/FINAL,37checks0.592s andCore/CLI pass. Authentic77.565s changes missing
method to precise GCC_QUOTED refusal atsamecall; hashes/cleanup pass. Evidence
target/wp863. Helper/M7A/M8/full link closure remain partial; Windows deferred.

## Acceptance receipt

WP864 appends Gcc format and source-exact ASCII encoding through existing
recipe publication/serialization. Windows remains refused. Independent DESIGN
and FINAL accept the bounded contract; six production/test files plus four
compact plan records, production net23 lines/proof net114. Forty-two exact
nonignored checks pass0.681s: BuildAPI3, Loading26, Analysis5, REAPI2, subrule6.
Final prep BuildAPI1.40s/Loading14.82s/Analysis23.85s/REAPI22.09s/subrule1.52s,
Corecheck11.94s/CLIbuild28.67s pass. Initial test-only equality operator compile
failure corrected to existing SpawnSpec publication comparison and explicit equal
recipe bytes; initial REAPIprep60s cap preserved, remaining artifacts completed
within second cap. No owner/assertion relaxation. Evidence target/wp864.

Authentic unchanged three-file cquery77.418s exit2 clears GCC and next refuses
callable resource_set at rules_cc finalize_link_action.bzl:381. Hashes unchanged,
actual process cleanup empty; rustfmt/diff/archive/status pass. Window09:55→10:17
UTC (including summary recovery), no runtime/performance speedup claim. Resource
callbacks, full helper/link closure, bootstrap and M8 remain unproved.
