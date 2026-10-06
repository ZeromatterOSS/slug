# Stage 6: Analysis, Toolchains, and Actions

## Goal

Analyze configured targets with Bazel-compatible providers, depsets,
configuration transitions, toolchain resolution, and action declarations.

## Scope

### Current state

Native filegroup analysis reuses configured condition/child observations and
retained native srcs/data, never Loading's flattened branch candidates. The admitted
default-output-group/no-coverage slice keeps files and executable srcs-only and
forwards stable files and distinct compile-order default/data runfiles, honors
the native srcs-data-runfiles flag, and
publishes singleton FilesToRun without RunfilesSupport or own actions. Empty
InstrumentedFilesInfo uses the existing builtin occurrence owner and an empty-only
native facade; hidden/validation groups use ordinary rule completion. Source and
derived artifacts preserve their original owners. Bounded visibility honors public
and ordinary same-package edges (or check_visibility=false); requested aliases own
visibility, actual targets own testonly checks. Restricted/macro-nonpublic visibility,
testonly prerequisites, package-group aliases and nonneutral unmodeled native
attributes fail closed. Named data declaration admission reuses the existing
Loading attribute owner and query metadata. This is a
full-toolchain prerequisite, not installed-tool or full cc_toolchain acceptance.

Loading retains named data selectors/concatenations, omission/default provenance
and the union of srcs/data condition keys; literal/branch duplicates fail there,
while selected concatenation duplicates fail in Analysis. Selected data
prerequisites share target configuration, condition/child joins,
cycle/Need/error handling and per-attribute indices. Default runfiles merge data
contributions before srcs default runfiles; data runfiles begin with srcs files,
then data contributions and conditional srcs data runfiles. Each rule data
contribution includes stable-wrapped files only when the retained
incompatible_always_include_files_in_data Boolean is True, then its data runfiles.
Actual source/generated File dependencies, including aliases, always contribute
files. The appended typed command option defaults True and carries through exec.
Both partitions propagate package closure and validations regardless of runfiles
flags. Bazel 9.2 Filegroup:61-105, Runfiles:669-773 and AliasConfiguredTarget:166-168
pin this split; no second retained representation or action owner is added.
Native filegroup data-only same-package source eligibility reuses retained data
label values and the existing tracked source resolver. Selector condition keys are
excluded by the value walker; only selected joins observe physical sources.
Symbolic-macro-only references and names of existing macro instances are excluded
from new data candidates while legacy srcs eligibility remains unchanged. Nested
unexported data Files keep Null identity and unconditional File runfiles inclusion.
External generated-label reconstruction remains separately deferred.

The native declare_file binder accepts positional/named filename and a named-only
genuine File or None sibling. Under admitted Linux paths, sibling supplies its
output-directory-relative parent only; existing path normalization and componentwise
repository/package confinement precede the shared declaration sink. Every new File
keeps the current context's configured owner and bin root, including source,
foreign-owner/configuration derived, directory and status File siblings. Existing
interning, type conflicts, active-token guards and write/symlink registration remain
authoritative; declaration itself creates no action. Directory sibling declarations,
Windows sibling behavior and alternate repository layout remain deferred.

Repeated ordinary artifact-symlink registrations within one CtxActions reuse the
first action/index only for full typed input/output and execution-flag equality
with the same default execution envelope. Progress is presentation and the first
message remains retained. Generic registry/batch conflicts and cross-target closure
eligibility stay authoritative; absolute-path/general symlink sharing is deferred.
Eligibility is the exact bounded slice; first-action vector canonicalization is
Slug-native, with Bazel raw duplicate-row/progress inspection remaining deferred.

WP846 admits one Bazel 9.2 package-default legacy license case in configured
native filegroup analysis: a Default-provenance singleton `licenses=["notice"]`
is retained by Loading/query and ignored only by the filegroup's provider/
action construction under default license options. Explicit target licenses,
other inherited values, `output_licenses`, other nonneutral attrs and opt-in
license checking remain closed. Root/external forwarding and same-DICE
notice-to-restricted-to-notice tests pass; fresh Bazel/Slug configured queries
match. Authentic Rust-toolchain cquery clears the license guard and next
stops at `_xcode_config` Apple configuration-field resolution. The error
does not name the owning target; no C++ closure is claimed.

WP847 projects the admitted Apple `xcode_config_label` configured field
from the owning structural `xcode_version_config` Label option, yielding
the pinned Bazel 9.2 cpp `host_xcodes` default in both target and exec
configurations. It does not use the distinct objc Loading fallback or add
an option override, Xcode provider, repository effect or DICE owner.
Configuration tests pass 93/93; the rebuilt authentic cquery next
panics in Bzlmod selected-extension routing. The panic's requester is
unlabelled, so configured C++/Xcode closure remains unproved.

M2 configured analysis is accepted with Slug-native configuration/path identity.
WP-7-35 adds `ctx.actions.declare_directory(filename, sibling=None)` through the
active analysis token and existing artifact owner. A phase-local declaration map
includes predeclared outputs, permits same-kind repeats and rejects type conflicts.
`File.is_directory` and typed `DefaultInfo.files` retain Directory kind, owner and
path; executable/runfiles and generated-input restrictions remain unchanged.
Sibling artifacts and analysis-time tree contents/Args expansion remain unsupported.
Pinned Bazel 9.2 StarlarkActionFactory.declareDirectory and DefaultInfo own the
admitted declaration/provider semantics; configured output path identity stays
Slug-native. REAPI's verified output-tree manifest is operation-owned (Stage 7).

WP-7-36 admits these whole-tree artifacts in typed Spawn ordinary input lists and
top-level depsets. The retained ArtifactInputs keep exact owner/path/kind and
depset representation; tree Args expansion and directory executable/tool admission
remain unsupported. ConfiguredTargetKey.artifact_owner exposes the existing
Analysis producer projection, including full configuration and toolchain execution-
platform preference, for Core prerequisite lookup. No new artifact identity.

WP-7-39 retains OutputGroupInfo as checked artifact depsets in the configured
provider collection. The kwargs-only loading/analysis constructor normalizes File
lists/tuples immediately; dependency, forwarded and nested values share its builtin
identity and the existing graph-aware lowering/materialization memo. Groups retain
source labels, derived owners/configurations/kinds, depset order/topology and aliases
across groups/providers. Only an all-empty record canonicalizes its group depsets
to the default empty. Publication uses the shared provider equality state.
Field iteration follows pinned Bazel UTF-16 ordering; public dir() inherits native
Rust ordering. User providers named OutputGroupInfo remain distinct. Configured
_validation_transitive overrides reject pending the native allowlist/propagation
owner. WP-7-40 completes returned groups through stable builders after runfiles
support: binary runfiles trees or nonbinary default-runfiles files/symlink targets
join the hidden group, and eligible ordinary dependencies contribute validations.
Implicit/tool/skip-validation/filtered edges do not contribute. Target-configured
configuration_field late-bound attributes may contribute: their leading underscore
does not classify them as ordinary implicit edges. Constructor-only
and nested values retain their original graphs; DefaultInfo files stay independent
from explicit OutputGroupInfo.default until command selection.

Source and generated-file configured nodes now own singleton file providers;
generated files additionally forward only their generator's nonempty validations.
Dependency views consume these providers directly. Ordinary ctx.attr label
dependencies expose Targets, including sources; callers obtain Files from DefaultInfo
instead of a source-only projection. Default-bearing immutable Targets also expose
`files` as their existing frozen DefaultInfo.files depset, preserving explicit files
independently of executable/runfiles/output-group contents. Reflection is conditional
on that provider; missing-default/package-group legacy fields and the unmodeled
`incompatible_disable_target_default_provider_fields` option remain deferred/refused.
One actual-target node identity
represents configured and null targets, allowing aliases to preserve source identity
without a configured-only fallback or panic. Source aliases retain source filename
admissibility checks. Internal merges, private validation overrides, analysis-test
execution, requested-output scheduling and CLI activation remain deferred.

Registration-error identity is implemented and accepted at `a06f3ddfc`; source
observation presentation is accepted at `ac6140f41`. Preserve the shared typed
Loading-owned error DAG, structural equality and bounded borrowed Display/Debug.
Those changes are not pending implementation.

Combined selected-toolchain request/output-conflict R2 is preserved and
unaccepted. [The fixture/gate ledger](./configured-cli-fixture.md) owns the
remaining validation state. The detailed contracts below own semantics; the
current manifest owns scheduling. The named/automatic runtime is preserved
unaccepted through `498ea2f49`. Its source-invariant reconciliation packet is
independently reviewed and Phase B ready; no runtime correction is accepted
yet. Authentic F3 and all three real CLI
gates now clear that invocation and select rules_cc `_def_parser`, an omitted
`attr.label` computed default whose retained schema already carries
`cfg = "exec"`. Stage 4 owns its narrow loading-time callback prerequisite;
existing configured dependency resolution consumes the resulting label. R2,
groups, and that prerequisite remain one atomic acceptance stack.

WP-7-20 accepts the finite rules_rust 0.73.0 regular-crate-root Args callback.
Loading authenticates the canonical loaded source identity, SHA-256 and native
call span; analysis lowers one regular `(File, str)` pair into an immutable
artifact/root-path recipe with structural equality. WP-7-21 also admits regular-File
dirname mapping at the four generated-manifest/output/stdlib/sysroot sites.
It retains ArtifactInputs, including shared depset shape and alias identity,
and maps before formatting/uniquification. One AnalysisArtifact.dirname
projection serves both File and retained rendering and spells the execution
root as `.`. Constructor validation rejects directories and FilesToRun.
WP-7-22 admits nonempty CrateInfo/AliasableDepInfo extern (ordinary/metadata)
and dependency-dir callbacks at the two pinned provider sites. Their dedicated
retained owner validates selected fields and preserves the full provider depset
and mapper identity; source authentication precedes the two-function branch
discriminator. The production-loader proof publishes a configured Spawn and rejects/restores changed source in one DICE
service. File.is_source and default-layout Label.workspace_root are exact for
the admitted artifact/repository shapes; generated paths and error timing keep
their existing Slug-native classification. Generic callbacks, directories and
sibling layout remain unsupported; the native-link slice below is separate. No callable
or evaluator heap crosses publication, and no new semantic cache/key is added.

WP-7-28 admits regular source files from materialized canonical repositories as
Null SourceFile nodes. Analysis resolves their existing canonical load route and
Bzlmod repository path keys, including observed producers; it never reconstructs
a host root from an execution path or reads source bytes. Source execution paths
use `external/<canonical repo>/...`, and source short paths use `../<repo>/...`;
main paths stay repository-relative. AnalysisArtifact owns both projections for
File/Args/Spawn, and native-library ambiguity lookup uses short_path. Exact path
shape uses existing Slug-native canonical names. Built-in catalog source-file
admission and sibling layout remain deferred; WP766 supplies the separately
classified coherent derived-root/short-path relationship below.

WP-7-30 exposes the same observed source-path resolver to Core with its existing
route/path observation frontier. Configured source analysis still projects only
path semantics and does not read or depend on source content. The function checks
neither declaration visibility nor action closure admission; its execution caller
must supply retained artifacts from the validated closure.

WP-7-23 adds `SpawnSpec::expand_forced_param_files`: one immutable action-local
aggregate owns replacement argv and virtual file bytes derived from the retained
recipe and first declared output. Forced shell/multiline/flag-per-line formats
share ArgsWrite rendering; flag-only positionals follow their replacement arg.
Empty files, spill numbering and substitution follow pinned Bazel 9.2; paths
retain Slug-native spelling. Expansion validates primary paths and rejects
virtual-file collisions with every output. Conditional spilling is unsupported
pending command-line-limit ownership. No retained identity or DICE key changes.
Stage 7 owns atomic input-tree composition; execution admission remains open.

WP-7-24 carries each loaded module's observed source SHA through the existing
shared filename/provenance table into `AnalysisEvaluationContext`. Rule equality
includes the table; the pinned Args classifier compares manifest, evaluated
caller and pinned digests. This prepares authentication of imported native-link
helpers without retaining evaluator objects or reading source files in analysis.
The static/PIC Cc provider slice below supplies constructor and lowering support;
nonempty native Cc internals and toolchain discovery remain open.

WP-7-25 admits the pinned native-library directory and default direct/indirect
link flag callbacks plus ambiguous-library directory mapping. Loading checks the
caller and the single canonical utils.bzl observed SHA; only the two immutable
default callback bindings are accepted. `RetainedRustNativeLinkArgs` retains full
four-field AnalysisValue tuples via the shared lowerer and publication equality
state, including otherwise unused fields and depset aliases across Spawn inputs.
It validates regular-library shapes, maps PIC/static/interface/dynamic preference,
whole-archive flags, ambiguity/name/version handling and user flags before Args
formatting/uniquification. The real Linux builder proof uses explicit linker
input/toolchain structs and the Rust linker fallback, with direct/indirect and
include-flags branches, parameter bytes, imported-source A/B/A and unused-callable
rejection. This is argument mapping, not native Cc providers/toolchains or execution;
Windows/Darwin and C++ runtime-library callbacks remain unsupported.

WP753 adds the pinned Cargo `_runfiles_map` closure as a typed recipe. The unique loaded
Cargo source row, observed/evaluated SHA, exact callsite and immediate helper authenticate
capture extraction. Args traces the evaluator-local fake executable and copies workspace
name; Analysis lowers those captures and the shared File depset into owned structural state.
Exact artifact equality filters only emitted arguments, preserving the action input depset.
Rendering uses file.path and the existing short_path projection before ordinary vector
options/parameter encoding. Noncallback Directory vectors render literal paths only with
expand_directories=False; their default/true expansion remains rejected. The Bzlmod-only
ctx.workspace_name getter exposes `_main` under
the active-context check. Derived paths remain explicitly Slug-native; arbitrary callbacks,
Directory expansion and complete Cargo toolchain/rule execution remain deferred. The focused
proof invokes the unchanged private helper with real typed values and documented inert
imports; it is not a full Cargo module-loading proof.

WP-7-26 supplies shallow immutable list/dict copies to `cc_internal.freeze`.
Read-only variants reuse the retained evaluator's collection algorithms, keep
nested aliases, reject mutation and direct hashing, and survive module freeze
and GC without changing mutable collection layout. Structural provider hashing
rejects mutable nested inputs. The unchanged rules_cc public static/PIC library,
linker-input/linking-context and CcInfo constructors now feed real provider rows
to Rustc. WP762 extends HeaderInfo to an immutable shared artifact-bearing DAG:
an Arc token owns native equality/hash; iterative publication comparison checks
all module/header/dependency payloads and the global alias partition, memoizing
visited pairs. Loading projects eight immutable attributes; materialization
reuses the owned model without producer-heap retention. Constructor normalization
uses observed Host for .inc matching, independent of target/exec OS; empty module
initialization needs no Host. Ordinary dependencies retain graph edges; exported
dependencies merge ordered public/private/textual lists. No global counter or
evaluator heap crosses DICE. Native scanning/transitive memoization, complete
toolchain rule production and full Cc compilation remain separate gates.

WP763 publishes the complete unchanged CcToolchainInfo constructor through the
existing AnalysisValue owner. Loading authenticates the pinned provider definition
against canonical identity, observed/evaluated source SHA and callsite; only its
initialized invocation registers four finite callable recipes. The module GC root
owns registrations/captures alongside empty depsets. Runtime's generic lazy
occurrence capability preserves symmetric original/native equality and existing
function-name hashing without retaining a heap or executable closure. Publication
compares all captures and aliases; the externally comparable module-global recipe
also requires the original token. Retained Cpp owns the complete configuration,
uses explicitly Slug-native occurrence identity, and checks private access against
the current evaluator's provenance. Unchanged configure_features and Rust linker
queries consume the actual complete provider. Generic callable serialization,
full toolchain/FDO production and compile/link actions remain deferred.

WP877 implements the captured Rust-toolchain callback as one finite retained
model, with coupled publication/invocation activation. The pinned toolchain source
(`c4b613ce…c4d468e`) publishes nested make_ccinfo, capturing complete cc_toolchain,
feature_configuration, the link-stdlib Boolean and the genuine rust_std Target.
The consumer supplies label/actions/allocator/std; producer ctx/actions never
enter that recipe. Loading authenticates caller/source/callsite and imported
allocator-helper (`ae4acb50…1e536e`) provenance, reads the directly calling frame
through the existing native_call_local_value capability, and traces registrations
until owned lowering. BuildAPI remains Starlark-free and owns source/capture/alias
semantics with occurrence equality and complete publication comparison; Loading
runtime and Analysis rematerialization own invocation against the consumer's
current token, configured root and native sink.

Loading prepares the original helper through its existing Legacy/Observed source
owners and authenticates complete transitive authority before either Analysis
materializer runs. Frozen code and defining heaps are phase-owned bindings;
BuildAPI's RustToolchainCallable retains only immutable authority, occurrence and
four owned captures. Genuine current/live consumer actions are required before
original helper validation. Private calls retain actual helper source frames.
The Core coupled gate proves both routes, producer disposal, fresh consumers,
static/dynamic Solib construction and complete configuration/source/import
restoration. Complementary original-helper checks cover allocator forms, ordered
library/panic layers, both std modes, validation order, self-contained action
construction and merge/None branches. Runtime Solib/link execution, resource_set,
conditional interface/LTO/stamp prerequisites and generic closure/code retention
remain deferred. Exact helper behavior and Slug-native identity remain distinct;
the manifest records genuine authentic replay evidence before activation.

WP764 adds exact public copts/conlyopts/cxxopts and private dotd/save-temps/header
policy getters over that same complete configuration. Eight command admissions
reuse typed converters; both exec paths map host compile options, append -g0
according to observed Host OS, propagate dotd and reset save/header policy.
Missing Host observation fails closed. No new retained option owner or cache is
introduced. File.extension follows the pinned full exec-path scan; per_file_copts
requires genuine positional Cpp/File/Label and admits only empty retained state.
Unchanged get_copts and dotd_files_enabled consume the published complete toolchain
with ordered language/rule flags, suppression branches and same-DICE restoration.
Nonempty per-file regex state, actions2ctx/root/naming adapters and discovery-aware
CppCompile production remain deferred; a generic non-discovering Spawn is not
evidence for the default dotd-on compile path.

WP765 recovers the original analysis ctx through a GC-traced registration in the
existing evaluator roots. Root/subrule factories retain their existing call-stack
identity; lookup checks live membership and registration/evaluator association,
while the returned outer ctx remains locked during subrules. A scoped guard
clears registration and invalidates root lifetime on success/error. No moving
Value enters DICE or an untraced host owner. Toolchain artifact naming queries
the retained feature model using observed non-Windows Host/POSIX semantics.
Native CcToolchainInfo wrapping requires its real keyed provider, preserved schema
classification and native model. ProviderOccurrence now carries that classification
through publication equality and materialization without copying a schema list;
existing language equality/hash and mixed-schema parity scope are unchanged.
WP766 supplies coherent mandatory derived roots across Files, declarations, Args,
REAPI and physical publication. File.root/owner and live ctx.bin_dir/genfiles_dir
share immutable typed owners; unchanged rules_cc root-relative/repository helpers
and Rust root/exec slicing consume full exec paths. Structural identity and output
tokens remain Slug-native; exact named path relationships follow pinned Bazel9.2.
Ordinary external outputs and shared-bin Solib coordinates remain distinct. Split
genfiles, direct external generated-label requests and discovery-aware CppCompile
remain deferred.

Native Solib construction accepts source and generated regular library Files and
all preserve-name/consumer-prefix flag pairs. The existing two-field spec owns the
complete input artifact and current consumer's bin-root output. Source naming uses
its source path; generated naming uses its root-relative path. Preserved names
keep the basename below an escaped parent directory, optionally prefixed by the
current sink's canonical consumer label; only that directory component is shortened.
False mode retains the existing escaped soname. Pinned Bazel9.2 branch structure,
filetype checks and UTF16-length/UTF8-SHA256 shortening apply; Slug label, root and
configuration/path spelling remain Slug-native. Existing action token, declaration,
typed registry equality and conflict checks remain authoritative. This prerequisite
supports the Rust stdlib's actual True/True library-builder call; callable dispatch,
Solib execution and REAPI projection remain deferred.

WP767 supplies compute_output_name_prefix_dir and declare_compile_output_file.
The phase-local configuration facade shares the complete SlugConfiguration;
coverage still projects from it, and configuration provider publication remains
unsupported. Prefix selection follows the required String purpose suffix, not a
fabricated configuration mnemonic. Declarations normalize supplied-label _objs
coordinates and require containment in the active ctx package; supplied label
locates the File while active ctx owns it. Full configuration equality/liveness
precede mutation of the existing declaration registry. Main/external and target/
exec configured publication and A/B/A are covered, with naming protected separately.
This is not full compile() or compiler-action execution. Alternate roots, sibling/
Windows layout and normalized backslash/NUL paths remain unsupported.

WP768 composes the unchanged compilation-context initializer with genuine main/
external target/exec owners, ordered include branches, ordinary/implementation
dependencies and full CcInfo/HeaderInfo publication/restoration. The restricted
configuration query exposes the existing fixed nonsibling layout, not a new
configuration option; command capture still rejects the sibling-layout flag.
is_tree_artifact reads genuine File kind, including true for Directory artifacts.
The unchanged public compile wrapper succeeds for header-only inputs. Context
composition does not admit tree templates, virtual-header rewriting, module-map
actions or whole compilation-output provider publication. Selected-
registry test inputs exercise existing main-origin loading; production loading
and all retained representations are unchanged.

WP769 adds construction-only typed CppCompile recipes for regular C/C++ sources
with object and default dotd outputs. Eager toolchain/input snapshots remain
separate from evaluator-live Variables and compilation context. Loading traces
pending recipes and their original role depsets through module roots; Analysis
lowers them with returned providers, preserves completed depset memo entries,
then atomically merges actions in registration order before runfiles completion.
One publication alias bijection covers the full recipe and distinct mandatory,
cache-only, declared-header, builtin and additional-prunable input roles. Existing
configured-node DICE ownership supplies invalidation and cutoff; no discovery
cache or new key is introduced. Structural identity and paths remain Slug-native.
Generic CppCompile Spawn/ActionKey projection, header discovery and execution remain
unsupported in BuildAPI/Core/REAPI. The [WP918 typed query](wp918-cpp-compile-aquery-2026-10-06.json)
borrows regular pre-discovery C/C++ metadata under default Unix/no-paramfile guards.
Mandatory-spawn, cache-key and declared-header identities supply text inputs; source
and each input keep their own configured roots. Fixed query environment and borrowed
tool/options differ from effective execution environment. Inert constructor fixtures
do not establish allocator toolchain/argv or compiler execution parity.

WP770 adds pure typed argv and effective-environment projections over these
immutable recipes. The retained Variables adapter mirrors admitted native lookup
and lazy conversion using per-call scratch: argv uses a fresh empty child cache,
while environment uses original cache seeds. Tool-first ordered feature expansion
does not imply shell processing or implicit compiler flags. Explicit client values
resolve only declared inherited environment names; PWD is synthesized unless
requires-darwin is present, then feature entries override. NOOP path mapping,
no paramfile and always-pass filtering are the exact admitted source slice;
paths/identity and diagnostics remain Slug-native. No new DICE owner or execution
admission is introduced. Published C/C++ PIC/main/external/target/exec command
regressions use inert toolchains, not production allocator argv evidence.

WP771 adds pure regular-file input-role projection and supplied-dotd reconciliation.
Mandatory inputs include cache-only dependencies; spawn inputs instead include
the complete declared/prunable header superset. Parsed dependency paths resolve
against allowed derived identities first, then an explicit authoritative source
resolver. Final inclusion checks use action-root strict_system_includes policy,
declared membership and source-defined ignored directories. Returned discovered/
post-discovery sets never mutate the configured recipe. Exact POSIX/NOOP lexical
and resolution algorithms operate on Slug-native valid UTF-8 and structural paths;
invalid bytes, tree inputs and unmodeled platform policies fail closed. The pure
resolver seam does not supply Core observation or execution authority.

WP772 adds pure pre-execution input preparation: original-cache compiler options
supply system include paths, while raw user flags and context supply ordinary/
quote paths. Checks preserve native combined/split flag classification and POSIX
normalization. Pre-execution exemptions always include builtin, context system
and external directories, independent of the stricter post-dotd policy. Success
returns the existing input-role sets and caller-owned dangling-option warnings;
it is not an execution certificate. Neither warnings nor results mutate DICE
recipes, and failure publishes no partial result.

WP773 supplies the unchanged rules_cc toolchain-provider helper with genuine
configuration, File depsets and its authentic no-profile FDO subrule. The full
published CcToolchainInfo feeds separate public C/C++/PIC compile consumers in
main/external fastbuild, opt and default exec configurations. Configuration owns
nullable plain-text minimum_os_version and Core stamp options; both exec paths
reset them. Loading exposes public minimum_os_version/is_tool_configuration
methods and current-caller-restricted stamp_binaries, without parallel state.
Exact named helper/configuration semantics retain Slug-native structural identity.
The fixture authors only the helper's attribute envelope; full cc_toolchain
attribute extraction, allowlist/TemplateVariableInfo envelope, runtime Solib,
module maps, profile actions, %package include paths and execution stay deferred.

WP774 projects genuine ctx.file/ctx.files from the already-prepared dependency
DefaultInfo.files. An existing stable depset root owns mixed-order traversal and
deduplication; runfiles and executable-only Files do not leak into this collection.
Schema eligibility distinguishes absent optional labels, single-artifact inputs
and nondependency attributes. Immutable captured structs/lists are ordinary
publishable data, while retrieving them through an expired ctx remains forbidden.
No new DICE owner, graph value kind, filesystem lookup or ordering engine exists.
The authentic helper/compile proof now consumes both facades. Full cc_toolchain
still requires genuine template providers and implicit dependency
construction, notably both build-info status transforms even when stamping is off.

WP775 supplies native package-group PackageSpecificationInfo through the existing
configured-node include graph, including canonical external groups. Loading and
Query share the BuildAPI membership value; group-local negatives remain separate
through stable transitive composition. Native method-only equality is constant,
but provider publication compares the full membership graph for invalidation.
The authentic toolchain helper retains a genuine allowlist. This does not admit
external declaring visibility, broader package-spec syntax, status transforms or
full toolchain construction.

WP776 adds native TemplateVariableInfo as a shared immutable ordered string map.
The Loading constructor snapshots its input and each variables read returns a
fresh mutable dictionary. Native equality/hash uses occurrence identity, while
typed direct/nested publication compares the full ordered payload and a shared
alias bijection. No evaluator heap or context is retained. The unchanged rules_cc
make-variable helpers feed genuine construction and Target lookup alongside the
existing CcToolchainInfo proof; ctx.var merging/expansion and full cc_toolchain
remain deferred.

WP777 gives configured contexts genuine workspace-global info_file/version_file
descriptors from one pure DICE key over NormalizedAbsolutePath. BuildAPI owns
distinct status artifact/root provenance, stable versus constant metadata, and
full workspace/role equality. Native File fields and DefaultInfo/provider
publication preserve that identity across target/exec and repositories, freeze,
GC and context teardown. Loading also supplies the general native is_symlink
predicate. No status bytes or actions are synthesized; source staging, requested
prerequisite planning, runfiles encoding, Cpp discovery and REAPI refuse status
execution. At the WP777 boundary, real tracked status production, volatile
execution policy and build-info transforms remained prerequisites to the authentic
full toolchain envelope.

WP778 accepts construction: the installed verbatim C++
build-info rule registers volatile and stable TranslateBuildInfo actions followed
by the redacted Symlink. Its genuine bare OutputGroupInfo uses the existing
provider lowerer; arbitrary structs are not promoted to providers. The authentic
toolchain-helper/public-compile proof now consumes this real dependency rather
than an empty envelope. The two capture-free callbacks are represented by finite
recipes authenticated against canonical source, complete source digest and native
callsite; no callback executes or evaluator heap survives in the action.
BuildAPI's shared immutable spec retains full workspace/status, template,
configuration/root/output and recipe identity. Orthogonal Ordinary/Constant output
metadata participates in structural Eq/Hash/Ord and declaration conflicts; only a
typed matching transform may own a constant output. Ordered input deduplication
does not erase the distinct status/template roles. These are exact admitted
construction semantics with Slug-native structural identity, not an ActionKey
claim. Core and raw projection/materialization entrypoints fail closed before
execution effects. [WP922](wp922-build-info-recipe-render-2026-10-06.json) adds pure
rendering on these checked recipes over borrowed supplied status/template strings.
It preserves ordered literal replacement, callback defaults and the exact missing
timestamp diagnostic; the returned String owns its bytes after input release.
Three borrowed pairs and local output buffers add no retained cache or interner.
The native Unicode rendering domain remains Slug-native.
[WP923](wp923-supplied-workspace-status-values-2026-10-06.json) adds a pure supplied-byte
status parser on WorkspaceStatusArtifacts: LF and first-space delimiters, last-value
duplicates, exact Latin1 mapping, and independent compact map ownership. It streams
borrowed slices into existing owned compact values without a whole-file decoded
String or extra pair Vec. [WP924](wp924-build-info-supplied-byte-render-2026-10-06.json)
completes pure supplied status/template bytes to independently owned header bytes.
Both entrypoints share one borrowed callback selector and literal engine; byte input
Latin1 closure permits exact checked byte encoding. Local map/template owners drop
before encoding, while rendered String/final Vec overlap remains explicit.
[WP925](wp925-build-info-catalog-template-preparation-2026-10-06.json) adds a separate
observed catalog-content resolver and private native BuildInfo template preparation
key. The actual validated installed action selects the tracked catalog route/value;
original result/byte Arcs, full action identity and complete union frontier/certificate
are retained. Native acceptance checks current observations, and held metadata keeps
its evaluation graph alive. [WP926](wp926-native-workspace-status-inputs-2026-10-06.json)
adds fresh native Unix request input capture and a workspace-global DICE status-content
producer. Actual USER reported text, kernel full/short hostname, uid provenance,
SOURCE_DATE_EPOCH presence and one clock sample participate in tracked raw equality;
fixed English UTC and default policies are explicit native domains. Repeated consumers
share the original capture/descriptor/status Arcs and independent sorted UTF8 buffers.
Certified checked templates produce complete independent headers through the existing
Latin1 consumer. Original-Arc/transaction/source validation precedes native acceptance;
selected, ordinary and abort paths revoke capture while held historical bytes survive.
Actual cancellation/drain and final owner release pass. Bounded text/direct buffers and
local date scratch add no global interner/pool or whole map; held metadata still retains
its graph. [WP927](wp927-native-workspace-status-publication-2026-10-06.json) adds a
private certified native publication root over that original combined producer.
Attempt-local fixed two-role stages borrow the original buffers; existing descriptor
helpers are shared with configured output staging. Linux GNU readable single-link
regular destinations publish equal stable without writing and volatile unconditionally.
Same-parent replacement and 0444 mode are native policies, not Bazel inode/link parity.
Both roles preflight under current-source finalization after revision commit; later IO
may be partial and cannot return an accepted terminal. Original capture, full frontier,
namespace admission and current source validation remain required. Stages retire on
normal retry/abort/last-Drop and immediately after success even through held clones;
initial identity-read/keep/unlink failures may leave unclaimed scratch. Native build
preparation establishes the marker namespace before certification; real source edits
exercise retry, without fabricating a cold-directory retry. No payload copy/global
interner or semantic filesystem cache is added. Thirty-nine distinct Core controls
(33 preserved protected plus6 current), default CLI and three FINAL reviews pass.
[WP928](wp928-native-workspace-status-file-inputs-2026-10-06.json) connects that
publisher to an original producer/request-bound readonly correspondence owner.
The genuine selected native root reads actual bounded status-file bytes into the
unchanged checked callback with original catalog bytes; its independent header Vec
is distinct from precomputed metadata. Immutable compute-issued metadata/root Arcs
supply the full frontier and issuance binding; a shared permit spends on success
and every error before any further effect. Historical FDs cannot grant a new request.
Four readonly receipt FDs retire on last Drop;8236 stack bytes replace no semantic
owner or string interner. Original precomputed header/graph remain intentionally
retained. All44 current Core controls (39 protected+5 new), default CLI and three
FINAL reviews pass. Capture-phase IO and finite renderer errors are source-only
propagation proof; actual source retry/abort, partial rename, physical mutations/read
refusal and four-FD/Weak release pass. Header-file publication remains deferred.
Nondefault status policies/epoch, volatile cache/invalidation, arbitrary or Java callbacks,
BuildInfo/action-chain execution, installed module tools and full cc_toolchain stay deferred.
Source-target analysis now classifies builtin catalog Files through the tracked
repository route and existing immutable source key, propagating catalog/type/hash
errors. This is distinct from filesystem acquisition: no host path or metadata is
invented. Ordinary filesystem source acquisition still refuses the builtin catalog;
the separate metadata path shares immutable catalog content without a real_path.
Root-origin loading of this zero-import C++ module uses the same tracked canonical
builtin mapping owner as installed-target loading; unrelated zero-import modules
are not promoted. The existing single-import promotion remains unchanged.

WP760 retains C++ Variables through the existing AnalysisValue publication owner.
Loading owns traced evaluator-live bindings and dotted lookup caches; freeze and
publication preserve immutable cache seeds, parent aliases, raw binding types,
File owners and depset topology. Frozen queries use per-call scratch, never mutate
published DICE results. Map/Single native equality and raw hashability are distinct
from structural publication equality, which also compares unexpanded cache payload.
BuildAPI owns pure feature expansion behind a narrow lazy variable-access adapter;
no new semantic key, interner or external runtime is introduced. The unchanged
compile-variable builder and configured dependency query prove the admitted exact
NOOP-mapper slice; structural artifact/configuration identity remains Slug-native.
WP761 adds real Cpp link-variable projections over the same SlugConfiguration,
including fission membership, mode-dependent strip and ordered linkopts/omitfp.
Both exec paths share the source-derived reset/host mapping. Getters retain no
parallel option state. Unchanged link-variable and Rust linker-helper proofs use
real ctx/native configuration with explicit authored toolchain envelopes.
Full toolchain/FDO construction, compile action production and execution remain separate
gates. WP760's receipt records unsupported value types
and the deferred frozen diagnostic-cache history explicitly.

### Analysis surface

- configured target keys and analysis DICE keys;
- user-defined providers and native providers;
- depset semantics and validation;
- `ctx`, `ctx.attr`, `ctx.files`, `ctx.actions`, outputs, runfiles, and
  default providers;
- configuration fragments, build settings, and transitions;
- platform and toolchain resolution;
- aspects and aspect propagation after base analysis is stable.

## V1 Extraction Candidates

- depset/provider tests from
  `slug-v1-archive:app/slug_build_api_tests/src/interpreter/rule_defs/depset.rs`
  and
  `slug-v1-archive:app/slug_build_api_tests/src/interpreter/rule_defs/provider/collection.rs`;
- `rule(implementation=...)` tests from
  `slug-v1-archive:app/slug_interpreter_for_build_tests/src/tests.rs`, with
  implementation orientation from
  `slug-v1-archive:app/slug_interpreter_for_build/src/rule.rs`;
- selected `cc_common` and provider surfaces from
  `slug-v1-archive:app/slug_build_api_tests/src/interpreter/rule_defs/cc_common.rs`,
  `slug-v1-archive:app/slug_build_api/src/interpreter/rule_defs/provider.rs`,
  and
  `slug-v1-archive:app/slug_build_api/src/interpreter/rule_defs/provider/collection.rs`;
- action declaration plumbing from
  `slug-v1-archive:app/slug_build_api/src/actions/registry.rs` and
  `slug-v1-archive:app/slug_build_api/src/interpreter/rule_defs/context.rs`,
  only after Stage 3 path semantics are clean;
- shared-DAG design and traversal from
  `slug-v1-archive:app/slug_build_api/src/interpreter/rule_defs/nested_set.rs`,
  `slug-v1-archive:app/slug_build_api/src/interpreter/rule_defs/transitive_set/traversal.rs`,
  and
  `slug-v1-archive:thoughts/shared/plans/slug-bazel-subplans/54-depset-transitive-set-shared-core.md`.

These paths are absent from the active clean root. Inspect them with
`git show slug-v1-archive:<path>` or an external archive worktree; do not search
for or import them from the active root. Use the matching
[Stage 9 extraction-ledger](./09-v1-extraction-ledger.md) row to choose the
import mode, oracle, and validation.

## Bazel Oracle Anchors

- `ConfiguredTargetFunction.java` for configured-target evaluation.
- `StarlarkRuleConfiguredTargetUtil.java` and
  `StarlarkRuleClassFunctions.java` for rule implementation and rule
  definition behavior.
- `RuleContext.java`, `ConfiguredTargetFactory.java`, and provider classes for
  `ctx` and provider semantics.
- `ToolchainResolutionFunction.java`, `SingleToolchainResolutionFunction.java`,
  `PlatformFunction.java`, and `PlatformKeys.java` for platform/toolchain
  selection.
- `AspectFunction.java` and `ToplevelStarlarkAspectFunction.java` for aspect
  propagation after base analysis is stable.

Migrate focused themes from Bazel 9.2.0 tests including
`RuleConfiguredTargetTest`, `StarlarkRuleContextTest`,
`StarlarkRuleClassFunctionsTest`,
`StarlarkRuleImplementationFunctionsTest`, `DepsetTest`, and the owning
platform/toolchain tests. Every fixture records its exact class/method in the
Stage 1 provenance manifest.

## Accepted Analysis Graph Invariants

These invariants define the accepted M2 graph and constrain every new
M7A analysis family. Earlier scaffold implementation instructions are historical.

1. `ConfiguredTargetKey` is a real DICE key over a Stage 3 label,
   configuration, transition inputs, repository mapping, toolchain/platform
   policy, and the loaded target revision.
2. Its computation obtains the loaded package from the shared Stage 2 graph,
   resolves configured attributes, and recursively computes configured
   dependencies. Use DICE parallelism such as `try_compute_join` for
   independent edges while preserving deterministic Bazel ordering.
3. The rule implementation runs through the real analysis registry/context.
   `ctx.attr`, files, executables, configuration, toolchains, and dependency
   provider collections are prepared inputs; Starlark-visible getters perform
   no filesystem or graph discovery.
4. The returned provider collection is authoritative. Do not synthesize
   `DefaultInfo` from declared outputs or infer providers from action side
   effects. Validate Bazel's required-provider and duplicate-provider errors.
5. Action declarations are registered during that rule evaluation and retained
   in `AnalysisResult`. Stage 6 computes deterministic action ownership and
   conflicts but performs no execution.
6. `cquery` reads these configured-target results and `aquery` reads these exact
   action objects. Separate command-only mock graphs are forbidden.
7. Same-daemon dependency, `.bzl`, configuration, toolchain, and repository
   mapping edits invalidate through named DICE dependencies, including create
   and delete transitions.

Preserve these invariants and focused tests when extending the existing graph.

### Buck2 and V1 reuse anchors

Before implementing this gate, inspect Buck2 commit
`088c75c7e36805df99c3de29062baa95db700b8b` at:

- `../buck2/app/buck2_analysis/src/analysis/calculation.rs` for the analysis
  key and recursive dependency-compute pattern;
- `../buck2/app/buck2_analysis/src/analysis/env.rs` and
  `../buck2/app/buck2_build_api/src/analysis/registry.rs` for analysis context
  and action/provider registry ownership; and
- `../buck2/app/buck2_interpreter_for_build/` for attribute coercion,
  interning, and prepared Starlark values.

Inspect the equivalent V1 archive paths as behavior/test sources where Stage 9
records them. Port the DICE/registry/compact-data patterns behind V2 Bazel
labels, configurations, providers, and output paths; reject cells, Buck labels,
Buck configurations, and Buck output semantics. Hot graph structures require
the repo utility audit: prefer retained `SmallMap`/`SmallSet`, Fx hashing,
`Hashed`, `ArcStr`/`ThinArcStr`, `Dupe`, and `Allocative` where their measured
shape fits instead of default owned `String`, `Vec`, or std hash collections.

## Implementation Slices

These are subsystem contracts and capability inventory, not a next-packet queue.
Dated activation/review passages describe historical evidence only; use Current
state and the canonical manifest for scheduling.

### 6.1 Configured Target Key and Configuration

- Define Bazel-shaped configured-target keys using Stage 3 labels plus
  configuration hash.
- Implement target, exec, and host-like transition policy only where Bazel 9
  still exposes it.
- Build setting values, command-line flags, and transition outputs are DICE
  inputs, not global process state.
- Initial modules: `app/slug_analysis_v2/src/{key.rs,result.rs,dice.rs,configured_target.rs}`
  and `app/slug_build_api_v2/src/providers/`.
- Use archived V1 analysis code such as
  `slug-v1-archive:app/slug_analysis/src/analysis/calculation.rs` and
  `slug-v1-archive:app/slug_analysis/src/analysis/toolchain_resolution.rs` only
  as pattern sources; do not port Buck labels or V1 configuration identity.
- Implement this as the DICE computation defined by the current-priority gate;
  a key-shaped serializable struct without `Key::compute` integration is only
  substrate.

### 6.2 Providers and Depsets

- Implement user providers, native providers, `DefaultInfo`, `OutputGroupInfo`,
  `RunEnvironmentInfo`, `FilesToRunProvider`, `PlatformInfo`, and the provider
  collection API needed by the first rulesets.
- Implement Bazel `depset` order, validation, flattening, equality constraints,
  and transitive nesting without implicit `transitive_set` coercion.
- Store the transitive structure as an immutable shared nested DAG: composition
  must not recursively copy children, and flattening is an explicit consuming
  operation. Selectively extract the archived shared traversal/depth lessons
  named above while keeping the Bazel depset facade V2-owned.
- Initial modules: `app/slug_build_api_v2/src/{ctx.rs,attrs.rs,providers.rs,runfiles.rs,depset.rs}`.
- Extraction candidates are the archived V1 depset/provider tests and
  implementation paths named above, but public types must be Bazel-shaped and
  Stage-3 label based.

#### 6.2A Zabel-informed retained depset core gate

Accepted architecture: `WP-6-7A-dense-retained-depset-action-import-r1` at
base `683538254`. It freezes one immutable dense store with ordered tagged
successor rows, local and external handles, structural alias-aware publication
equality, and a typed File/action-input view over the same owner. Authentic
rules_cc `all_files` forwarding selects this as the immediate prerequisite to
generic action builtins. Independent reserved-representation rereview returned
`ACCEPT`; the bounded
`WP-6-7A-dense-retained-depset-action-import-implementation-r1` packet is
implemented, proved and terminally `ACCEPT`.

Before the first broad ruleset consumer depends on nonempty transitive depsets,
implement one bounded Zabel-informed retained core behind the exact Bazel
`depset` facade. Bazel 9.2 remains the semantic oracle. Use Zabel commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` as the primary design reference,
but independently implement the Rust types and algorithms and copy no Zabel
production code. Zabel test scenarios may seed independently written V2
fixtures only when their provenance is recorded and their expected results are
regenerated with the pinned Bazel 9.2 oracle.

Start from a dense retained store informed by Zabel's packed row/bit layout,
compact node and edge indexes, distinct node-identity and leaf-equality
deduplication, Bazel-specific construction normalization and traversal,
generic/File specialization, producer-qualified external references, late leaf
materialization, direct action-input import, and invocation-local flatten
caches. Treat the current immutable per-node `Arc` DAG as migration scaffolding,
not the presumptive long-lived representation.

Construction must retain the user-declared Bazel order and perform Bazel's
construction-time validation at the exact public boundary. The internal
traversal algorithm may be selected and executed only when a consumer needs a
projection: delayed computation is not itself observable. Every consumer must
nevertheless receive the exact result for the retained declared order, and the
Bazel facade must not expose Buck2's ability to choose a different order at
consumption time. Keep node/topology identity, leaf equality deduplication,
semantic DICE equality, flattened values, action-input topology, and action or
REAPI fingerprints as distinct domains.

The gate must include:

- all four Bazel orders, mixed/default compatibility, empty-set behavior,
  singleton reuse, multi-child depth, validation precedence, and the configured
  depth limit;
- diamonds, repeated child aliases, distinct nodes with equal leaves, duplicate
  direct values, and cross-owner forwarding; in particular pin Bazel's
  topological `[a, b, c, b] -> [a, c, b]` alias result;
- a non-recursive or otherwise stack-safe traversal at the supported maximum
  depth;
- cold and repeated `to_list()`, direct action/Args consumption without
  flattening, provider freeze/materialization, and release of retained owners;
- retained bytes, allocations, construction cost, cold/warm consumption cost,
  and realistic rules_cc/rules_rust fan-in and diamond graphs; and
- `Allocative` coverage plus proof that caches are request/evaluator scoped or
  explicitly bounded and never become unmodeled DICE semantic state.

Record the resulting representation and measurements. After the exact
Zabel-informed baseline works, optional focused experiments may adopt isolated
Buck2 ideas only when they improve a measured Slug workload without widening
the surface or weakening Bazel semantics. Public Buck2 `transitive_set`
definitions, projections, reductions, BFS/DFS, and implicit `depset` coercion
remain unsupported/deferred; an internal late projection is admissible only
when its Bazel-visible values, errors, timing boundary, action inputs, and
identity projections remain exact.

### 6.3 Rule Implementation Context

- Implement `ctx.attr`, `ctx.file`, `ctx.files`, `ctx.executable`,
  `ctx.label`, `ctx.outputs`, `ctx.actions`, `ctx.fragments`, `ctx.toolchains`,
  `ctx.exec_groups`, `ctx.var`, `ctx.expand_location`, and
  `ctx.resolve_command` in priority order driven by fixtures.
- Starlark-visible methods that need prepared values must receive them from
  analysis inputs rather than doing filesystem or graph discovery.

### 6.4 Action Declaration IR

- Define an action IR that is independent of executor choice.
- Actions include mnemonic, argv, env, execution requirements, input digests,
  tools, paramfiles, output declarations, progress message, and exec
  properties.
- Stage 7 consumes this IR to build REAPI commands.
- Initial modules:
  `app/slug_build_api_v2/src/actions/{registry.rs,spec.rs,ctx_actions.rs,reapi_projection.rs}`.
- Implement `declare_file`, `declare_directory`, `declare_symlink`, `write`,
  `write_json`, `expand_template`, `run`, `run_shell`, `args`, output conflict
  checks, exec groups, toolchain action contexts, and action exec properties.
- Stage 6 emits deterministic action descriptions only; it does not execute
  actions or decide CAS/AC policy.

### 6.4A Immutable action-owner context

Schedule this owner after the first M1 request-revision/source-certificate
vertical and the just-in-time action/toolchain oracle subset, but before any
M7A packet admits broader action registration. Acceptance of the immutable
owner context is therefore an M7A entry gate. It is not blocked on the complete
Wave A fixture catalog, M8 bootstrap, M7B command breadth, or M9 exact identity
bytes. The current M7 repository source-consumer audit and its fixed cutover
remain unchanged.

Before Stage 6 admits another general action kind, named exec groups, applied
aspect actions, or multi-platform selection, retain one immutable owner
context at action registration. The target shape is:

```text
ActionOwnerContext {
    configured_owner,
    semantic_configuration_identity,
    admitted_checksum_or_display_projection,
    exec_group,
    execution_platform,
    exec_properties,
    selected_toolchain_context,
    aspect_provenance,
}
```

The exact Rust representation may be smaller or split into authenticated
projections, but every field that can affect action behavior must participate
structurally in equality and invalidation. The default exec group is an
explicit identity, not the absence of a context. An action retains the group-
selected platform, combined platform/target/group execution properties, and
selected toolchain context at creation time.

Do not later reconstruct an action's platform or properties from only its
label, the owner's current topology, a process-global current platform, or a
new toolchain-resolution run. `aquery`, Stage 7 execution, Slug-native action
provenance, progress observations, and future explain output must consume the
same retained owner context and action row. Bazel checksum and
configured-output bytes remain separate M9 domains.
Bazel ActionKey is a derived exact projection admitted per action family under
6.4B; neither projection permits semantic inputs to be omitted.

The design packet must audit the current `ConfiguredActionExecGroup::Default`
and `ConfiguredActionView` topology-derived platform path, then prove:

- default and named groups select and retain distinct contexts;
- two actions of one owner may use different groups/platforms/properties;
- `cfg = "exec"` dependencies use the group-selected exec configuration;
- aspect-created actions retain their applicable aspect/owner context;
- configuration, platform, property, registration, mapping, and toolchain
  edits invalidate through named DICE dependencies;
- aquery and REAPI projections are derived from the identical retained row;
- conflicting-output and other diagnostics preserve Bazel's error precedence;
  and
- retained data is immutable, compact, `Allocative`, and released with the
  owning analysis value.

Compatibility classification:

- selected platform/toolchain, exec-group behavior, property merging, and
  action-visible provenance are **exact** for admitted Bazel 9.2 slices;
- private Rust rows, compact identities, and added explain fields are
  **Slug-native**; and
- exact configuration checksum and configured output token remain
  **deferred to M9**; Bazel ActionKey projections are independently classified
  per family under 6.4B, with unadmitted exact projections deferred to M9.

Use the Stage 1 provider, action-conflict, aquery-topology, and toolchain
fixture backlog before implementation. This section freezes a future owner
contract; it does not modify the accepted bounded FileWrite action surface or
widen the active M7 packet.

### 6.4B Exact Bazel ActionKey projection

An exact Bazel ActionKey is a derived compatibility projection of the retained
configured-action row plus immutable owner context. It is not the action's
structural semantic identity, a new action graph, a configured-output token, or
the REAPI Action digest. Exact projection admission is independent of functional
execution admission. M7A requires complete semantic inputs, structural
invalidation, validated requested-root output ownership, discriminating aquery
comparison, and canonical REAPI projection for each demanded family. Exact
ActionKey bytes are not an M7A or M8 prerequisite. Configuration checksum,
output identity and unadmitted exact ActionKey projections belong to M9.

Every family records separate semantic, aquery-field and REAPI classifications
in bootstrap readiness. Preserve accepted exact projections byte-for-byte.
An unadmitted ActionKey field is explicitly unsupported/deferred or has a named
Slug-native token contract; it must never be presented as an exact Bazel key.
Missing semantic execution inputs still block action admission.

Mirror Zabel's byte-feeding approach, but reverify every field and order against
Bazel 9.2 source and fresh discriminating oracle evidence:

- provide one small V2-owned `BazelFingerprint` leaf over SHA-256 with the
  exact protobuf `CodedOutputStream` no-tag bool/int64/string length framing,
  Java UUID word order, and Bazel internal-string-to-Java-UTF-8 conversion;
- implement each admitted action family's exact `computeKey` body, including
  its GUID, field order, collection order, conditional fields, and byte
  representation, then append the common `ActionKeyComputer` tail;
- the common tail must reproduce platform presence and `PlatformInfo`
  fingerprinting in Bazel order, including parent/constraint structures,
  maps/lists, settings, toolchain types, message booleans, action execution
  properties, and the current zero uniquifier;
- never hash debug output, formatter text, Slug canonical identity bytes,
  `StrongHash`, a weak/precomputed map hash, or the REAPI Action encoding as a
  substitute for Bazel's ordered fingerprint stream; and
- fail closed when any source-derived input is not modeled. No zero,
  hard-coded, truncated, or opaque fallback key is admitted.

FileWrite is the first projection. Its packet must distinguish the regular and
compressed GUIDs, executable bit, and exact logical content; reproduce Bazel's
Java compression threshold and deterministic `GZIPOutputStream` bytes; and
retain the accepted discriminator that content and platform affect ActionKey
while declared output path, owner configuration token, and output-root spelling
do not. Each selected exact projection receives this source audit and its own
discriminating proof before the exact inspection surface is admitted.

Reviewed Zabel donor commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` supplies bounded leaf and test
ideas from:

- `src/core/bazel_fingerprint.zig`;
- `src/core/bazel_internal_string_java_utf8.zig`;
- `src/analysis/action_key_fingerprint.zig`;
- `src/analysis/file_write_content.zig`;
- `src/analysis/file_write_action_key.zig`; and
- `src/analysis/complete_action_key.zig`.

These are donor inputs, not the oracle. The packet must record exact Bazel 9.2
source anchors, accepted output vectors, mutation discriminators for every
conditional field, and a cross-check that Zabel's result still matches the
pinned Bazel revision.

Keep the domains visibly separate:

```text
ConfiguredAction + immutable owner context
  -> Slug structural identity       (equality and DICE invalidation)
  -> Bazel ActionKey projection     (aquery and parity)
  -> REAPI Action projection
       -> REAPI ActionDigest        (remote Action Cache)
```

There is no direct ActionKey-to-ActionDigest mapping. Exact Bazel ActionKey
reproduction alone is insufficient for Bazel local-cache or remote-cache
interoperability. Bazel's internal incremental ActionCache is a separate,
deferred surface whose reader/writer would also require
cache-record versioning and namespace, discovered inputs, environment and
execution metadata, output validation, and stale-entry behavior before the
exact key may be used for lookup or publication. The reusable library targets
remote caches and Bazel `--disk_cache`, both using REAPI Action digests;
[Stage 11](./11-bazel-compatible-cache-library.md) owns that contract.

Compute the projection on demand in request/phase scratch unless measured reuse
and invalidation justify a DICE projection. Do not retain a second semantic
identity or duplicate the action row. Reuse compact immutable owner fields and
deterministic compact maps already retained by Stage 6, preserve
`Allocative` coverage for any new long-lived wrapper, and benchmark only after
semantic vectors pass. Buck2 fast hashers and `strong_hash` remain useful in
their existing domains but do not implement Bazel's SHA-256 fingerprint
protocol.

Canonical scheduling selects exact projection work under M9. Functional
bootstrap admission uses the semantic and REAPI gates above.

#### Per-family ActionKey feasibility checkpoint

Before freezing an exact ActionKey projection implementation, fill one compact
table from pinned Bazel source and the actual admitted configuration:

| Fingerprint input (including conditional/tail fields) | Ordinary semantic producer | Bazel path-mapping behavior and effective mode | Exact bytes available? / evidence | Missing prerequisite |
|---|---|---|---|---|
| One row per distinct input source | Owner/key/value | Mapped, unmapped or path-independent, with source anchor | Established or unresolved; discriminator handle | None if proved; otherwise precise dependency |

At Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a`,
`src/main/java/com/google/devtools/build/lib/analysis/actions/SpawnAction.java`
`computeKey` fingerprints command lines and environment. In
`actions/AbstractCommandLine.java`, `addToFingerprint` obtains arguments through
`PathMapper.forActionKey(effectiveOutputPathsMode)`. Thus generated-path inputs
can require exact configuration/output bytes currently deferred to M9; mapping
may remove a dependency only when the actual action, encoding and effective
mode prove it. Audit the family implementation, not just this base-class example.

Reuse or add a discriminator across two configurations that change generated
path spelling, holding unrelated semantics fixed where possible. Verify each
relevant mapped/unmapped input and the resulting raw key bytes against Bazel.
Path-independent families such as the accepted FileWrite discriminator need no
invented output-identity dependency. No whole-M9 prerequisite is assumed.

If fingerprint bytes lack an ordinary producer, defer the exact projection and
record its precise M9 prerequisite. Functional family admission remains governed
by complete semantic inputs and the separate aquery/REAPI gates. Stage 10's
graph-local path correspondence is comparison-only and cannot supply runtime
fingerprint bytes. No oracle-generated production mapping or guessed token may
serve as an exact key. Record projection availability per family in readiness.

### 6.4C Generic Args/spawn/artifact-symlink category

Selected architecture packet:
`WP-6-7A-generic-args-spawn-symlink-category-architecture-r2` at base
`7b0db03e1`, now independently `ACCEPT`. R1 review returned `REPLAN` for an exact generated-File path
claim, missing effective default action-environment ownership, insertion-
ordered map identity and raw executable/absolute-symlink strings. The corrected
candidate freezes one evaluator-local Args recipe and one action-time
finalization seam into typed retained command-line segments, dense depset-
backed input/tool sources, a common run/run_shell SpawnSpec, canonical
environment/execution-requirement maps and distinct artifact, unresolved-path
and normalized private absolute-path symlink variants.

Before action construction, implement one bounded configured-action-
environment prerequisite from the sole structural native option vector plus a
retained Rust Host observation. It owns exact `action_env`/`host_action_env`
set/inherit/unset and Exec rewrite semantics, strict/default shell policy,
runfiles/shell-path inputs, key-sorted fixed bindings and inherited names, and
per-action env composition. Client inherited values remain command/execution
state, never configured identity; until their resolver lands, such actions fail
closed before execution. Map insertion order is not semantic identity.

Authentic rules_cc 0.2.17 FDO is the first discriminator only. Its chained
Args.add, File path fields, File executable, list/depset inputs, nested depset
tool, artifact symlink and cc_internal absolute-symlink route must use generic
Starlark/action owners with no C++ or parser branch. Plan the complete
non-callback category now, then land bounded environment, scalar/FDO,
vector/paramfile, complete spawn-envelope and unresolved-symlink successors.
String executables reproduce Host-flavored Bazel `PathFragment` normalization;
private absolute symlink targets additionally require absolute classification.
Generated File path/dirname/basename and rendered argv bytes remain Slug-native
until exact Bazel output paths land in M9, while typed relationships and path
operations are exact. Callback mappers, directory expansion, resource callbacks
and new execution/REAPI behavior stay explicitly deferred and fail closed.
Focused R2 rereview and the post-preflight command-surface narrowing rereview
both returned `ACCEPT`; implement only the configured-action-environment
prerequisite before selecting the scalar FDO action successor. That prerequisite
is accepted in `71d34affa`.

The scalar R1 candidate passes its serial Rust validation but terminal review
returns `REPLAN`. The authenticated FDO body exposes a pre-existing generic
provider defect: target-shaped source/generated files rematerialize
`DefaultInfo.files` as strings rather than Files. Configured evaluation also
lacks the recursive source manifest required by the already-designed private
caller check, and the action A/B/A proof did not count parent DICE computes.
Independent architecture review returned `ACCEPT`; activate only
`WP-6-7A-fdo-basic-args-run-symlink-implementation-r2`. It retains typed `AnalysisArtifact` leaves in the one
dense `DefaultInfo.files` depset for every file-target/rule producer, hands the
immutable source manifest into generic configured evaluation, and proves real
publication cutoff. Runfiles, OutputGroup, executable and FilesToRun typed
migration remain the next standard-provider category; add no FDO, rules_cc,
`cc_common`, parser or C++ branch.

The R2 implementation terminally passes. One checked dense
`DefaultInfo.files` owner now retains typed source/generated/declared artifacts;
configured evaluation receives the immutable recursive source manifest; the
A1/A2/B/A3/A4 proof counts real parent-key recomputation; and scalar Args,
Spawn, artifact symlink, and authenticated absolute symlink publication share
the generic typed action sink. The direct-source rules_cc FDO body reaches
configured analysis with no FDO/C++/parser special case. Generated path bytes
and structural identities remain Slug-native, while new-action aquery,
execution, ActionKey and REAPI projection remain deferred and fail closed.
Independent terminal review returned `ACCEPT`; author only the bounded
non-callback vector Args/param-file successor next. That successor is now
authorized after focused architecture correction rereview returned `ACCEPT`.
It fills only the already-reserved vector recipe, param-file policy, and Args-
backed write variants; callbacks, directory expansion, execution and the
remaining Spawn envelope stay deferred.

The R1 implementation terminal review accepts that retained/action
architecture but returns `REPLAN` on two bounded integration details. Bazel's
typed binding rejects an invalid supplied sequence/depset source before
entering unsupported callback handling, and the existing no-op loading action
sink needs a one-line signature adapter that R1 did not allowlist. Activate
only `WP-6-7A-noncallback-vector-args-paramfiles-implementation-r2`: move the
shared source conversion before callback rejection after the two-position name
check, formally admit the frozen adapter blob, and change no retained owner,
compatibility class, parser, C++ branch, callback semantics, or cap.
Focused correction and complete terminal rereview returned `ACCEPT`. The R2
implementation passes the focused ordering/consumer proofs, all four full
owner/direct-dependent suites, the public core check, and hygiene gates at
891/747/1,638 added Rust lines. The remaining Spawn envelope is the next
bounded action category; typed standard-provider breadth remains separate.
Commit `a01a23fe7` freezes that accepted implementation. The first complete-
envelope design returned `REPLAN`: Bazel's public `resource_set` parameter is
callable-or-`None`, not the callback-result dictionary, and an Artifact used as
an executable or direct-list tool may be associated with FilesToRun through an
`executable=True` configured attribute. R2 then returned `REPLAN` because
Bazel checks executable association for each File in a top-level tools depset
but deliberately skips that lookup for a depset nested inside a tools
sequence. Activate only
`WP-6-7A-complete-noncallback-spawn-envelope-implementation-r3`: unify
`run_shell` with the typed Spawn owner, admit only omitted/`None` resource
callbacks, and derive producer-owned executable-Artifact provenance so only
Files admitted by Bazel's exact container-specific association branch are
retained bare. Top-level depset leaves are visited as validation scratch
without flattening retained topology; nested depsets are not inferred.
Associated Files and direct
FilesToRun values fail closed for the later typed standard-provider/runfiles
category. Named/automatic exec groups, shadowed actions, resource callbacks,
execution, and C++ action families remain separate fail-closed categories.
Independent focused R3 correction review returned `ACCEPT`; implement only
that frozen envelope before selecting typed standard-provider/runfiles breadth.

Commit `bfe6f2690` terminally accepts that envelope. One pre-method binding
pass now preserves Bazel's public parameter order and outer-shape diagnostic
precedence before the common typed sink; the terminal correction rereview is
`ACCEPT`. Both action methods publish the same compact immutable Spawn schema,
with shell invocation, scoped executable provenance, environment, requirements,
unused-input discovery and every admitted argument/input/tool/output field in
publication equality. FilesToRun/runfiles, callbacks, named exec groups and
execution projection still fail closed. Select the typed FilesToRun/runfiles
standard-provider category next from actual BCR consumers, without a
`cc_common`, rules_cc, parser or C++ branch.

The [WP915 support query view](wp915-runfiles-aquery-2026-10-06.json) borrows the
existing configured action/spec/context and immutable support graph. It validates
all physical roles against borrowed owner configuration bytes and output roots,
ordinary expected kinds and default Unix policies before query. Retained ownership,
publication equality and executable support production remain unchanged. Callback
loans stay synchronous; no new DICE key/cache, string pool or lifetime boundary.

The complete typed DefaultInfo/runfiles/FilesToRun category architecture is
independently accepted in
`WP-6-7A-typed-files-to-run-runfiles-category-architecture-r1`. It freezes one
effective provider model, typed runfiles occurrences, producer-owned support
artifacts and scoped Spawn expansion, split into four bounded successors. The
first successor may migrate only the provider core and must retain an explicit
incomplete-support guard until runfiles-tree production is admitted.

The first successor,
`WP-6-7A-typed-files-to-run-provider-core-implementation-r1`, is terminally
accepted. It replaces executable path strings with typed Artifacts, retains a
stable files-to-run depset, materializes one dedicated FilesToRun Starlark
value, carries complete providers in root/subrule scope-local provenance, and
fails incomplete associated actions before publication. A bounded terminal
correction also reserves the exact typed RetainedRunfiles/RunfilesSupport
schema from the architecture, without constructing support or opening
runfiles behavior. Select successor 2 for typed runfiles values, merge
operations, and all five DefaultInfo parameters; support actions and Spawn
expansion remain successors 3 and 4.

Bazel 9.2 remains the sole semantic authority. Zabel
`0795445f3ab60f4e49070bdd0b94425c5610f73a` supplies peer phase-ownership,
typed-segment and input-topology lessons only; copy no Zig behavior or layout.
Keep evaluator objects out of retained values and keep semantic action/artifact
identity distinct from rendered paths, Bazel ActionKey and REAPI digests.

### 6.5 Toolchains and Platforms

- Implement constraint values/settings, platform target analysis, registered
  toolchains, toolchain type resolution, execution platform filtering, exec
  groups, and per-action exec properties.
- Registration order must come from Stage 5 bzlmod outputs and command-line
  flags in Bazel order.
- Initial modules:
  `app/slug_analysis_v2/src/toolchains/{registered.rs,resolution.rs,context.rs,exec_groups.rs,platform_constraints.rs}`.
- Replace V1 process-global toolchain state with DICE keys for registered
  toolchains, platform aliases, host fallback, optional and mandatory
  toolchains, target settings, default constraints, and per-exec-group contexts.

### 6.6 Aspects After Base Analysis

- Defer aspects until custom rules, providers, depsets, and toolchains are
  stable.
- First aspect fixture should cover attr propagation and provider requirements;
  advanced incrementality can be a later Stage 8/9 extraction.
- Initial modules: `app/slug_analysis_v2/src/{aspect_key.rs,aspect_analysis.rs}`.
- Model aspect dependency edges explicitly; do not smuggle aspect state through
  the configured-target cache.

## Exact Test Criteria

- `custom-rule-analysis-basic` fixture returns providers matching Bazel for:
  user provider, `DefaultInfo.files`, runfiles, and an output group.
- `ctx-attrs-files-executable` fixture compares `ctx.attr`, `ctx.file`,
  `ctx.files`, `ctx.executable`, `ctx.label`, and `ctx.toolchains`.
- `default-info-runfiles-executable` and `provider-output-group-basic` compare
  normalized provider keys, files, runfiles, and output groups.
- `depset-orders-and-rejections` fixture covers all Bazel orders, incompatible order
  failures, nested depsets, duplicate handling, and flattening order.
- `depset-orders-and-rejections` fixture compares `to_list()` order and
  rejection diagnostics.
- A focused structural test proves that combining nested depsets preserves
  shared child-node identity and does not flatten or recursively clone the DAG.
- `actions-api-basic` fixture declares write, run, run_shell, symlink, and
  expand_template actions and compares action IR to Bazel's normalized
  `ActionGraphContainer`.
- `action-declare-file-package-boundary`, `action-run-shell-basic`,
  `action-run-tool-exec-cfg`, and `action-conflicting-output` compare output
  paths, mnemonic, argv, env, tools, inputs, outputs, and diagnostics.
- `toolchain-resolution-first-platform`, `toolchain-resolution-host-platform`,
  `toolchain-resolution-platform-alias`, and `toolchain-mandatory-missing`
  compare selected execution platform, resolved toolchain labels, events, and
  missing-toolchain diagnostics.
- `exec-groups-action-platform` proves per-action exec-group platform selection.
- `transition-basic` fixture executes a user transition and proves outgoing
  configuration affects a dependency.
- `aspect-provider-propagation` runs only after base analysis is stable and
  compares aspect-produced providers and actions.
- Registered toolchain and platform edits invalidate through named DICE keys.
- `rg -n "std::fs|process-global|CellResolver|buck-out" <v2-analysis-crates>`
  returns no semantic production shortcuts.

## Acceptance Criteria

- Custom Starlark rule fixtures produce the same providers/actions as Bazel.
- Depset construction is cheap shared-DAG composition; flattening/copying is
  confined to explicit Bazel-visible operations.
- The retained depset core starts from an independently implemented,
  Zabel-informed dense/packed design and records Bazel 9.2 oracle evidence plus
  retained-memory and consumption measurements. Buck2 ideas are optional later
  experiments, and no Zabel production code is copied.
- Toolchain and platform fixtures match Bazel for focused public examples.
- Action declarations produce REAPI-ready command/input/output structures.
- No analysis shortcut depends on Buck cells or direct filesystem scans outside
  DICE-tracked inputs.
- A multi-target fixture proves recursive dependency analysis, provider flow,
  shared subdependency reuse, and deterministic action ownership in one
  same-daemon DICE graph.
- `AnalysisResult` contains the providers actually returned by each rule and
  the actions actually registered during its implementation; no output-derived
  provider synthesis or command-specific mock graph remains.
- Stage 8 `cquery` and `aquery` consume this graph without re-evaluating rules
  in separate command-owned state.

## Validation

```bash
cargo test -p slug_analysis_v2
cargo test -p slug_build_api_v2 depset
cargo test -p slug_analysis_v2 toolchain
slug-v2-oracle run --fixture custom-rule-analysis-basic --compare providers,actions,outputs,diagnostics
slug-v2-oracle run --fixture depset-orders-and-rejections --compare stdout,stderr
slug-v2-oracle run --fixture actions-api-basic
slug-v2-oracle run --fixture toolchain-resolution-first-platform --compare providers,events
slug-v2-oracle run --fixture exec-groups-action-platform --compare actions,providers
slug-v2-oracle run --fixture transition-basic
slug-v2-oracle run --fixture aspect-provider-propagation --compare providers,actions
```


## Selected-toolchain request contract (pending combined R2 acceptance)

#### Remaining group decisions resolved by pinned source

At Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a`:
`RuleClass.AutoExecGroupsMode.isEnabled:169-193` makes ordinary dynamic mode
use the private attribute when present (false and true both override the flag),
otherwise the native flag. Auto on empties default requirements and creates
one group per declared toolchain type; auto off retains the default requirements.
`skyframe/toolchains/ToolchainContextUtil.java:54-92,120-245` and
`DeclaredExecGroup.process` own this normalization and target group constraints.

`RuleContext.java:1091-1141` uses direct automatic-group lookup followed by
resolved requested-label alias lookup for `ctx.toolchains`; action
`toolchain=` does not get that alias-search fallback. `StarlarkRuleContext.java:
860-909` builds a composite automatic view or default view and thin
`ctx.exec_groups`. Explicit action group names still pass public-name validation;
the internal automatic label string is not a user-nameable `exec_group=`.
`StarlarkActionFactory.java:129-138,405-472,718-895` distinguishes omitted
toolchain from explicit None. Under auto policy, no explicit group and multiple
contexts, an unassociated executable/tool requires explicit toolchain or None;
recognized dependency runfiles and string executables do not trigger that check.
Nested tool depsets trigger it before element-type conversion. This is validation,
not automatic inference of a toolchain label from an artifact.
Slug's `subrule_invocation.rs::bind_toolchain` currently erases None/omission;
the future complete group packet must fix that at the binder, not in the sink.

`AutoExecGroupsTest:303-431,488-705,750-824,1279-1305,2500-2558` supplies the
policy/action/alias discriminators. Ordinary rule inheritance via `rule(parent=)`
is not currently admitted by Slug; preserve its rejection, not an invented copy
keyword. `rule(test=True)` injects a test group in Bazel
(`StarlarkRuleClassFunctions.java:1064-1080`) and has separate test-action
ownership; that category and configured aspect/subrule toolchain contexts remain
deferred. These decisions activate nothing in the selected prerequisite.

#### Contradiction and authoritative correction

Bazel's direct toolchain implementation edge does **not** apply the ordinary
Exec configuration transition. `analysis/producers/DependencyProducer.java:
161-176` passes the parent's configuration and separately the selected group's
execution-platform label; `PrerequisitesProducer.java:350-355` attaches that
label to the configured-target key. `skyframe/ConfiguredTargetKey.java:
127-139,188-220` includes it in equality/hash. The pinned
`skyframe/toolchains/README.md:31-45` explicitly distinguishes this mechanism
from a configuration transition. Toolchain-resolution tagged trimming
(`RuleTransitionApplier.java:187-218`) is not an implementation-edge Exec
transition; manual feature-flag trimming remains unadmitted in Slug.

In contrast, live `dice.rs::prepare_selected_toolchain_context:3525-3695`
constructs implementations in the selected platform's Exec configuration.
`tests/starlark_rule.rs::selected_toolchain_accepts_declared_actions_and_default_outputs`
asserts Exec and removal of target-scoped settings at 2625-2643. Those
assertions encode the defect, not compatibility evidence. The new packet must
replace them: preserve the parent's configuration/target-scoped settings on the
implementation edge; only the implementation's actual Exec attributes perform
Exec projection. Semantic option loss cannot be classified as a Slug-native
checksum/path-byte divergence.

The child default context receives the separate preference through
`DependencyResolver.java:477-510` and `ToolchainContextUtil.java:120-143`.
It is a **preference**, not unconditional forcing:
`ToolchainResolutionFunction.java:324-359` selects it first if suitable, otherwise
continues ordinary eligible-candidate selection. `PlatformKeys.java:255-305`
looks it up among the known target/execution platforms; absent preferences do
not trigger new discovery. Zero requirements still honor a suitable preference.
Allowed-toolchain-type platform policy is already outside Slug's admitted
platform schema; do not silently admit it while implementing this preference.

Propagation is edge-specific. Incoming rule-transition delegation preserves
the preference and the existing apply-transition marker
(`TargetAndConfigurationProducer.java:304-316`). Ordinary Target/Exec/Starlark
attributes, including alias.actual, do not copy it
(`DependencyProducer.java:268-334`; `rules/Alias.java:61-96`).
A direct alias wrapper may carry the preference, while its actual does not.
Each nested direct toolchain edge gets its own selecting context's platform.
Future named/automatic child contexts use fresh unforced resolution keys
(`UnloadedToolchainContextsProducer.java:100-124`), not the child default's hint.
Direct source/nonconfigurable implementations still fail the existing
ToolchainInfo/provider boundary; this packet does not invent a successful
nonconfigurable toolchain category.

#### Frozen prerequisite representation and producer contract

Add `toolchain_execution_platform: Option<Arc<CanonicalLabel>>` for the preference
to `ConfiguredTargetKey`, not `ConfigurationKey` or the native option vector.
Default construction is None; one explicit toolchain-key constructor/builder
sets it from the selected platform's canonical actual label. Allocate/share one
label Arc per selected context across implementation requests. Derived Eq/Hash/
Ord/Allocative must include label contents, not pointer identity. Preserve it
on same-node incoming-transition delegation, including non-idempotent final
keys. Keep user-facing label/configuration display unchanged; display is not
semantic identity.

The existing `ConfiguredNodeAnalysisKey` and observation wrapper inherit this
field through their node; no new DICE key family or global owner. Extend
`ConfiguredToolchainResolutionKey` and its observation wrapper with the same
optional structural preference, supplied from the analyzed owner's key.
Resolution retains existing tracked registration/platform/toolchain producers
and selection order, changing only suitable-preference priority, including
empty requirements. Known target-platform preference may need an Exec-shaped
platform view for Slug action ownership; obtain it through the existing tracked
platform producer without changing the candidate registration sequence.
Add `known_preferred_execution_platform: Option<ConfiguredTargetKey>` to
`ToolchainTopology`, produced only from the resolver's tracked known-platform
lookup and passed through analysis finalization. Its constructor validates Exec
shape and accepts a selected platform only from candidates or this explicit
known preference. Keep the field in structural equality/Allocative; do not
fabricate registration. A target platform outside the candidate sequence has no
resolved toolchain matches: empty/all-optional requirements can select it, but
do not run declaration matching against it as an extra registered candidate.
Preserve pinned `PlatformKeys.find` alias lookup: resolved target and host labels,
ordinary registered requested labels; the selected platform's actual label is
the preference on a direct implementation edge. An actual label reachable only
through an ordinary registration alias is not arbitrary discovery permission.

Selected implementation requests use the parent owner configuration unchanged,
plus the preference. Requested selection rows keep that full key; actual rows
keep the exact analyzed/delegated result key. Update the context invariants that
currently equate implementation configuration with execution-platform Exec
configuration: requested implementation configuration equals owner configuration;
actual identity is the producer's validated transition/alias result, not a
reconstructed key or an assumed unchanged configuration. Toolchain type
requests remain in owner configuration and action platforms remain Exec-shaped.

Replace lossy label/configuration reconstruction in `compute_actual_child` and
`observed_configured_result` with the existing full-node compute path.
Ordinary attribute construction deliberately starts with no preference; this is
not a blanket propagation rule. Preserve full keys through cycle tracking,
selected-toolchain edges, query/closure membership and runfiles package collection.

Extend the Arc-backed `AnalysisConfiguredTargetKeyData` with the same optional
shared label. Add one analysis-owned projection from full configured key and use
it at all four production conversion sites: dice artifact lowering, loaded-rule
owner creation and both materializer target routes. Derived artifacts, providers,
FilesToRun and runfiles then retain this owner distinction through existing
structural equality. Never append it to the configuration byte payload.

FileWrite semantic identity adds one optional tagged canonical-label field to
its configured-owner encoding; None preserves existing bytes, Some differs even
for identical label/configuration/content/platform. This is a Slug-native owner
projection, not Bazel checksum, output-path spelling, ActionKey or REAPI digest.
No new execution/action family or output-path algorithm is admitted.

#### Proof, lifetime, complexity and stop contract

Pinned tests: `ToolchainResolutionFunctionTest.resolve_forceExecutionPlatform`,
`_alias`, `_host`, `_host_alias`, `_noRequiredToolchains`;
`ToolchainsForTargetsTest.keepParentToolchainContext` is a synthetic-key source
discriminator, not a prior successful end-to-end Slug replay. Add source-derived
Rust regressions for the live consumer: two registered platforms A then B, parent
toolchain restricted to B, implementation with no toolchains and a FileWrite;
its options stay Target while its action platform is B. Include ordinary Target
and Exec child controls, nested toolchain selection, suitable preference over
greater optional coverage, unsuitable/absent preference fallback and no match,
platform aliases, incoming transitions, alias.actual clearing and missing source/
wrong-provider terminals. The existing named-group guard remains covered.

Required same-DICE cases: source and registration/platform/property A/B/A;
target-setting/native option preservation versus actual Exec projection;
None/A/B/A request interleaving; concurrent distinct preferences; cancellation/
Need/error and recovery without partial parent/provider/action publication.
Retained provider/derived-file identity and FileWrite owner bytes must distinguish
None/A/B even when all other fields match; None preserves old controls. Use full
retained keys, not helper reconstruction that omits the field.

Memory is DICE-retained semantic state: one nullable Arc per configured key/
retained target payload/resolution key plus the topology's optional known key,
sharing label/configuration storage rather than new maps.
No command cache, new interner, global registry or unowned evaluator borrow.
Constructor/key scratch and joined child buffers release with their phase; views
expire with existing tokens; DICE reachable versions/results own retained release
and service shutdown. Cancellation may leave valid DICE child cache entries but
publishes no incomplete parent; existing scoped joins/cycle guard and final source
certificate remain authoritative. Never hold a shared lock over a DICE await.
Reuse current Arc/Allocative utilities (Stage 9 Stages 3/6 utility row); V1/Buck2
are concept/utility guidance only, no donor code or new dependency is imported.
DICE worker equality/dependency/cancellation tests cited in the prior section
remain applicable, not Bazel semantics.

The original source-derived scope estimate was 900 production / 1,800 proof /
2,700 aggregate additions. Combined R2 has its own preserved receipt; do not
apply the superseded standalone estimate as a hard rejection gate.
The seven production files stay cohesive: key/carrier definitions, four thin
projections, existing resolution/delegation orchestration, result invariants and
the existing owner encoder. `dice.rs` stays the established single producer;
no new policy dispatcher or helper graph. Its selection helper must remain local
and bounded rather than copying the 460-line resolver. Large carrier/evaluator
files receive only fields/projections/invariants; tests reuse existing scaffolds.

The implementation must first add the nonfirst-platform/parent-option regression
and observe its expected failure, then fix the complete request category.
No archive/real-workspace replay is needed to discriminate this prerequisite.
No source overrides, fresh graphs, disabled provenance or longer-timeout workaround.
A new configuration domain, alternate forced-key side store, arbitrary preference
discovery, missing retained consumer, a contradicted contract requires REPLAN. Size estimates and focused
corrections follow the orchestration recovery policy. Group constraints/policy/views/action routing remain the immediate
post-correction design, not silently forgotten or implemented piecemeal.


## Configured-action closure integrity contract (pending combined R2 acceptance)

Independent architecture review ACCEPTS the combined selected-request/conflict
successor after one correction: retain the normalized missing-toolchain message.
This contract supersedes the preceding reserved conflict decisions, not the
accepted selected-request edge/resolution contract. No candidate Rust is restored
by this docs milestone. The implementation must deliver both changes together.

#### Source semantics and compatibility boundary

Authority is Bazel git object `8220c6198837d5c13d53fea211cf3282aa12408a`, not
the working-tree Java files. `actions/Actions.java:85-115` compares mnemonic,
Action implementation eligibility and action key, then ordered mandatory inputs
and outputs ownerlessly for two shareable actions. There is no explicit Java
class comparison. Its both-unshareable branch instead permits equal ownerful
input/output aliases; one-shareable/one-unshareable cannot share. The earlier
read-only audit's simplified "both must be shareable" is not the full predicate.
`Artifact.java:690-697` defines ownerless identity as root plus exec path;
`MapBasedActionGraph.java:49-66` registers by that identity, not configured owner.

For admitted string `ctx.actions.write`, `StarlarkActionFactory.java:360-379`
always calls the same FileWrite factory. `FileWriteAction.java:190-202,249-355`
selects compression deterministically from string length and fixed policy;
regular/compressed keys use distinct GUIDs plus executable bit and content or
compressed content. Equal text under this one factory therefore has equal
representation/key inputs even above 256 bytes; comparing structural text does
not require recreating gzip or claiming exact ActionKey bytes. Native custom
compression policies, lazy text and input-bearing writes are not admitted here.
Args writes are ParameterFileWriteAction, not this family despite their mnemonic.

`ActionKeyComputer.java:37-57` adds execution-platform presence/full PlatformInfo,
exec properties and a process-constant uniquifier. `AbstractFileWriteAction.java:
124-127` returns EMPTY action exec properties: target/group property overrides
must not cause false FileWrite conflicts. Raw platform properties still matter.
`PlatformInfo.java:164-179`, `ConstraintCollection.java:275-284` and constraint
value/setting `addTo` establish actual platform label, ordered setting/value
labels, raw platform properties and normalized missing-toolchain error message
for Slug's admitted no-parent/default-policy platforms. `Platform.java:98-100`
supplies the native attribute; `PlatformInfo.java:363-369` maps empty string to
None, preserving nonempty text. Slug already accepts explicit native overrides
(package.rs:2289-2294,8971-8991); retain them, do not add a new rejection guard.
Configured-key configuration, preference, toolchain provenance and
merged owner properties are not substitutes for these inputs. Existing platform
guards keep parents, remote properties, flags and toolchain policies unsupported.

Exact named behavior: scalar FileWrite same-output sharing/conflict, generic
strict-segment output-prefix rejection and runfiles-tree/MANIFEST exemption.
Slug-native: structural configuration/root identity, deterministic first-error
diagnostics, structural equality rather than Bazel fingerprint/hash collisions,
and existing output/display bytes. Exact checksum/ActionKey bytes remain deferred.
All registered outputs in the requested dependency closure enter the inventory,
including unused outputs and File/Directory/Symlink/RunfilesTree kinds. Prefixes
are checked within the same structural output root; equal paths are handled by
exact collision checking. Only a RunfilesTree outer output exempts its immediate
MANIFEST path, never an arbitrary Directory or other nested child (Actions:291-368).

The accepted executable/aquery family remains scalar FileWrite. Typed Spawn,
ArgsWrite, artifact/absolute symlinks and four runfiles-support families retain
their configured-analysis admission and existing execution/formatter guards.
Noncolliding outputs are not newly rejected. Their cross-owner exact-output
equivalence is explicitly unsupported in this packet: a collision requiring
one of those family keys returns an unsupported-equivalence terminal, NOT a
claim that Bazel rejects it and NOT ActionSpec/REAPI-digest equality. Legacy
Run/RunShell/WriteJson/ExpandTemplate/Symlink rows receive the same treatment.
This leaves broader-family sharing for its family admission work; do not
silently omit their outputs, invent a blanket owner inequality rule or broaden
their executor. Configured aspects remain unsupported at their existing guard.

#### Natural owner, publication and consumers

Conflict freedom is a ROOT-SET-dependent fact, not a ConfiguredNodeAnalysisKey
invariant. A and B may each build successfully while A+B conflicts; an unchanged
child reused from DICE must still participate in a new root-set validation.
Keep `compute_build_action_closure` as the single traversal/producer, after all
roots and transitive configured children complete and before its Complete(Ok).
Extract a private `runtime/configured_action_closure.rs` for a pure constructor
of `ValidatedActionClosure`, with private fields, owners Arc slice and compact
shared-duplicate action coordinates. Empty/loading-only closures use its empty
constructor. No unchecked nonempty conversion or success boolean beside raw data.
BuildCommandEvaluation retains this immutable value; every observed singleton,
legacy and observed multi-root constructor uses the same producer. No new DICE
key family: existing command-root keys already retain ordered roots/workspace/
configuration policy and depend on all full configured node keys.

Retain raw platform facts separately from merged execution properties in
ConfiguredActionOwnerContext, supplied by its existing constructor BEFORE merge;
do not recover them by closure lookup or reverse a target-property override.
Add one optional shared PlatformSemanticFact (Arc-backed properties) and accessor.
Extend PlatformSemanticFact itself with `missing_toolchain_error: Option<Arc<str>>`,
read/normalize it at platform_semantic_fact from the loaded native attribute, and
carry it unchanged through ConfiguredPlatform and property merges. Include both
changes in structural equality/Allocative. Unresolved contexts retain None.
FileWrite equivalence uses optional actual platform label, ordered constraint
labels and this full raw fact, mnemonic, exact content and executable bit. Scalar
shape validation proves one File output and empty inputs/tools/other execution
fields; no-platform pairs can be compared during analysis without admitting
them to the selected-platform executor. Missing/inconsistent raw selected facts
fail closed. FileWriteSemanticIdentity must also encode raw facts when they differ
from its already encoded merged properties (new optional tag). A second optional
tag encodes the normalized message when unequal to the pinned native default;
its framing distinguishes None from nonempty text. The default constant is only
an encoding elision rule, never substituted for the loaded source fact. This
preserves old default-platform bytes while distinguishing empty/custom messages.
No broader no-toolchain diagnostic-format claim is made. Neither this identity
nor full owner equality decides sharing.

Output identity is `(owner structural SlugConfiguration, bin root, relative
output path)` inside one workspace; preferences never change the output root.
Use borrowed scratch output rows sorted by structural configuration bytes,
path SEGMENTS and existing owner/action/output encounter ordinal. Exact collision
pass precedes prefix pass; report the first incompatible pair in that order.
For an equivalent FileWrite group choose its first closure-order action as the
execution representative. Retain all owners/actions for analysis, action counts,
literal/deps aquery and diagnostics, plus only nonrepresentative coordinates in
an immutable Arc slice. This is explicit sharing, not lost owner/action identity.
CLI/server use a new execution-view accessor that filters those coordinates;
existing semantic-view accessors remain owner-complete. Run consumes validated
evaluation but keeps its current one-action/executable restrictions. The low-level
test-only/document-hidden view constructor is not a graph validation certificate.
No materializer/REAPI executor repair or command-side collision scan is added.

Cquery deliberately remains independent: pinned `CqueryCommand.java:191` sets
checkForActionConflicts=false. Query/loading do not own an action closure either.
Aquery uses the validated build closure even when its formatter selects literal
owner rows. CLI/server/build/run cannot receive a successful conflicting closure,
so failure precedes execution RPC, action-output materialization and formatting.
Exact errors and unsupported-equivalence errors become structured retained
BuildCommandError variants, with path and both full owners, exit 2 and existing
source-certificate publication. Need/analysis failure precedence is unchanged:
validate only a completed closure; cancellation/outer errors publish no value.

#### Lifetime, proof and implementation boundary

Scratch is O(outputs + path depth) borrowed rows/ancestor stack and phase-local
duplicate coordinates; sorting is O(outputs log outputs), no pairwise all-action
scan. Retained growth is one shared raw-platform fact per action context, one
optional shared message per platform fact (cloned without text copies), and
O(shared duplicate actions) coordinates per command-root result. No retained
output map, global registry, cache, interner, new dependency or mutable service
state. Reuse Arc/Allocative and Stage 9 Stages 3/6 utilities; Buck2/V1 are concept
guidance only, no code imported. Unchanged derived equality permits DICE cutoff;
raw fact edits invalidate even when owner overrides mask the merged properties.
Results release with reachable DICE versions/command tokens, scratch on return,
cancellation or error; existing scoped joins/shutdown own async release. No lock
crosses an await. Existing observation/final-certificate checks remain mandatory;
unavailable historical Host state is rejected, never synthesized.

Adapt OutputArtifactConflictTest invalidation/new-target/overlap, unused action,
repeated/null build and directory-nesting tests to existing inline temporary
workspaces; preserve source attribution, with no copied oracle tree. Add same-
path equal-content share versus changed-content/executable/platform/raw-property
conflicts, equal raw/unequal merged-property sharing and unequal raw/equal merged
conflicts, default/empty/custom missing-toolchain message A/B/A and unchanged-value
cutoff, long/unicode content, distinct configuration/path controls, deterministic
multi-conflict order, runfiles exemption and non-FileWrite unsupported-equivalence
classification. Prove A and B alone succeed, combined fails, warm repeats fail,
source A/B/A restores and concurrent disjoint requests do not poison one another.
Preserve cquery success and all owners in aquery; execution emits one representative
per shared group. The saved selected-toolchain red regression must fail at the
new producer before RPC, not be weakened to a later accessor assertion.

Retained Buck2 worker tests `when_equal_return_same_instance`,
`test_detecting_changed_dependencies`, `mismatch_epoch_results_in_cancelled_result`
remain guidance; reuse Slug observed Need/error/cancellation and source certificates.
CLI one-shot/stable-daemon build/run/aquery conflict tests use local fixture inputs,
zero accepted mock-transport calls and no output changes. Run positive sharing
through the common core execution-view/REAPI plan boundary without external replay.
Full owner and direct-dependent gates remain required after focused discriminators.
No performance improvement is claimed; large-closure benchmark work is not reopened.

Exact original files and test results are preserved in candidate
`27e9e9c0c`; the fixture/gate ledger records outstanding acceptance. Existing giant analysis/core dice files receive only cohesive producer
integration and field plumbing; the new validator and separate core proof module
prevent another policy subsystem accumulating in either file. No fallback exists.
Missing source facts, a newly admitted action-family key, broad execution change,
output suffix or a key side store requires REPLAN. Routine corrections
and growth-estimate review follow the orchestration recovery policy. Named/automatic group design resumes immediately after combined acceptance.
Design validation: pinned-source/structure checks and git diff --check pass;
archive check retains exactly three known thoughts-path failures. Saved candidate
hash/applicability remain verified. No Rust, build, test, replay or materialization
ran for this design; implementation and previously unrun gates remain unaccepted.


## Execution-group successor (after combined R2)

Before freezing implementation scope, reconcile existing production-closure
artifacts under [bootstrap readiness](./bootstrap-readiness.md#family-admission-and-completion-procedure).
Record which producer/rule/action under `//app/slug_cli_v2:slug` demands named
and/or automatic groups, with pins and evidence. The historical `cc_library`
example below proves that rule's requirements, not its presence in bootstrap.
If both modes are demanded, retain the shared complete owner and coupled
semantics below. If demand is narrower, explicitly classify the remainder as
deferred and preserve its guards; absence requires graph evidence. Unknown
demand remains unresolved. Do not bypass a guard or truncate required semantics.
Use a design checkpoint in the implementation packet if this reconciliation
leaves a bounded contract; split design only for an unresolved ownership choice.
The historical design-only selection below is evidence, not current scheduling.

### Rule execution-group runtime prerequisite: named-only REPLAN (2026-09-10)

The selected 0.2.17 cc_library has named `cpp_link` **and** an explicit true
automatic-group policy. Stage 4 authenticates the entire declaration and
corrects the named-transition attribution. The named-only audit terminates in
`REPLAN`: before runtime activation, design the shared rule execution-group
category including automatic policy, requirements and action selection.
Successor `WP-4-6-7A-rule-execution-group-runtime-design-r1` is docs/source only.
It must freeze a bounded implementation contract, not another carrier-only
loading bypass. No guard moves, runtime code, test fixture or representation
change is admitted by this audit.

Independent terminal architecture review returns `ACCEPT` for this docs-only
REPLAN and successor selection; it does not approve runtime implementation.

#### Pinned semantic basis and discriminators

Authority is Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`.
Use that git object in `/home/wgray/bazel`; its working HEAD is different.
Paths below are relative to Bazel's `com/google/devtools/build/lib` Java main
or test trees. These are source-established requirements, not new passing
Slug tests or a whole-category Bazel parity claim.

| Concern | Source and discriminating test |
|---|---|
| Named requirements/constraints and internal default-copy | `packages/DeclaredExecGroup.java:43-149`; `packages/RuleClassBuilderTest.testDuplicateExecGroupsThatInheritFromRuleIsOk`. `COPY_FROM_DEFAULT` is internal, not a public `copy_from_rule` keyword. |
| Independent resolution, including no-toolchain groups | `analysis/producers/UnloadedToolchainContextsProducer.java:100-124`; `skyframe/toolchains/ToolchainResolutionFunctionTest.resolve_noToolchainType`, `.resolve_execConstraints`, `.resolve_noMatchingPlatform`. Each group has its own requirements/exec constraints under the owner's target configuration. |
| Named dependency transition | `analysis/DependencyResolutionHelpers.java:231-255`; `analysis/StarlarkExecGroupTest.testExecGroupTransition`, `.testInvalidExecGroupTransition`. Resolve the named group's execution platform; unknown name errors, never default fallback. |
| Provider view | `analysis/starlark/StarlarkExecGroupCollection.java:43-159`; `starlarkbuildapi/platform/ExecGroupCollectionApi.java`. String-indexed thin views expose resolved `.toolchains`, exclude default and report unknown names. Collection absence differs from an empty resolved toolchain map. |
| Properties | `analysis/ExecGroupCollection.java:70-180,195-245`; `StarlarkExecGroupTest.testSetExecGroupExecProperty` and unknown/inherited/override tests at 605-712. Validate target group prefixes; merge using the selected platform for each group. |
| Action routing/failure | `analysis/starlark/StarlarkActionFactory.determineExecGroup:855-895`; `StarlarkExecGroupTest.testExecGroupActionHasExecGroupPlatform`, invalid-action tests at 395-438. Validate existence/public name, then bind that group's owner/platform; automatic mode also checks explicit toolchain/group agreement. |
| Automatic policy/group derivation | `DeclaredExecGroup.process:131-148`; `analysis/AutoExecGroupsTest` flag/attribute matrix at 303-431, group membership at 756-770, action inference/mismatch at 505-704/1281-1304. Per-toolchain generated names are distinct from public identifiers. These are required successor research, not admitted behavior. |

Property precedence must account for `computeProperties` filling missing
target-group keys from target defaults before its final merge. Effective
precedence is platform-default < platform-group < target-default < target-group;
a target-default value therefore beats a same-key platform-group value. Add
that discriminator, not just the simpler target-group override case.
Toolchain selection invariants may reuse `SingleToolchainResolutionFunctionTest`
multiple-platform/requirement equality tests; they do not prove the public
group API. Aspect group propagation and `AspectAutoExecGroupsTest` remain
deferred configured-aspect breadth. Built-in test-runner groups and inheritance
must be explicitly classified by the successor before any generic rule claim;
neither can be inferred from this non-test cc_library declaration.

#### Live owners and required replacement boundary

In `slug_loading_v2/src/package.rs`, `DeclaredExecGroup` currently retains
only toolchains; nonempty group constraints reject. Frozen declarations retain
the sparse group/name-index carriers, then `FrozenRuleDefinition::invoke:7636`
rejects them before initializer/computed-default checks and publication.
`StarlarkRuleImplementation:855` and its structural equality have no group
carrier. `attrs.rs::AttributeDependencyConfiguration` has Target/Exec/Starlark,
not a named execution-group edge. Builtin target constraints/property schemas
already exist; declaration shape alone is not a configured semantic owner.

In `slug_analysis_v2/src/dice.rs`, `ConfiguredToolchainResolutionKey:216`
contains workspace/configuration/requirements but **no group exec constraints**.
`compute_configured_toolchain_resolution` selects the first platform immediately
for empty requirements; it cannot implement a constraint-only named group by
calling the unchanged key. `root_declared_dependency_keys:1444` accepts one
exec configuration and marks every Exec edge Default. Root preparation at
5713-5995 resolves one context; `prepare_selected_toolchain_context:3525`
and the no-toolchain branch manufacture Default action contexts.

`starlark_rule.rs::AnalysisContextGen` exposes one `toolchains` view and no
`exec_groups`; `validate_default_spawn_context:1378` rejects explicit groups
and nondefault toolchain selection. `result.rs::ConfiguredActionOwnerContext`
already retains structural group/platform/toolchain identity and validates
matches; `with_action_contexts` can match multiple contexts, but this is not
evidence that the live producer supplies them. Its current property helper
is not proof of Bazel's group-prefix/default-fill semantics. The detached
`toolchains/exec_groups.rs` prototype has a mutable second context map, owned
String/Vec/BTreeMap storage and no live DICE provenance: avoid activation.

#### Cross-stage ownership and proof obligations

The natural declaration producer is the loaded rule/package, with compact
detached requirements, constraints and named transition identity participating
in `StarlarkRuleImplementation`/schema equality. The natural normalization
producer is configured-target preparation: combine declaration, resolved target
attributes and the immutable native command configuration, never CLI-side
reconstruction. The native option registry already contains
`incompatible_auto_exec_groups`; the successor must establish its runtime
projection and attribute precedence rather than introduce ambient flag reads.

Resolution must consume all normalized requirements and group constraints
through tracked keys, preserving ordered registrations, alias normalization,
target settings and distinct target/selected Exec configurations. The successor
decides whether to extend the existing structural resolution key or extract
its cohesive owner; it must justify equality sharing when two groups have
identical resolution inputs without collapsing their group/action identities.
One immutable configured-target-owned group collection must supply named
dependency transitions, provider views, property selection and action routing.
No second semantic registry, command cache or evaluator-owned retained map.

Default/named/automatic identity, normalized inputs, selected platform,
configured toolchain providers and merged action properties must remain
structural at their natural owner and affect downstream equality/invalidation.
Configuration identity, output-path/display projections, Bazel ActionKey and
REAPI/content digests remain separate domains. No exact checksum/path claim.

New declarations/contexts are DICE-retained semantic memory, not command
scratch. Prefer current CompactString/Arc-slice/SmallMap/Allocative utilities
(Stage 9's Stages 3/6 utility and Stage 6 platform-property rows); Buck2 is
utility/concept guidance only. Views/normalization/join buffers are evaluator
or phase scratch; no retained borrow of scratch or unowned heap. Publish only
a complete immutable collection after all required group resolutions/children
and final source-certificate validation. DICE may retain independently valid
child results on parent failure, but no partial parent/provider/action escapes.
Cancellation joins scoped work; no manual lock crosses a DICE await. DICE
version reachability owns retained release; views expire with the analysis
token, scratch with its phase, and service shutdown drops runtime-owned state.

Required proof is same-DICE source/configuration/platform A/B/A (including
constraints, automatic policy, toolchain payload and properties); absent/edit/
delete/recreate and unavailable-history rejection; overlapping request policies
without cross-contamination; equality cutoff for unchanged normalized inputs;
Need/error/cancellation and default-only nonregression. Reuse retained Buck2
`dice/dice/src/impls/worker/tests.rs` tests `when_equal_return_same_instance`,
`test_detecting_changed_dependencies`, `mismatch_epoch_results_in_cancelled_result`
as ownership guidance, not compatibility evidence. Existing Slug
`tests/starlark_rule.rs` default-exec all-label-shapes, selected-platform terminal,
zero-toolchain, recursive invalidation and cancellation tests are controls;
`tests/toolchain.rs` prototype collection tests do not prove the live path.

The successor must freeze executable proof commands and upstream adaptations,
Rust/test ownership scope, production/proof growth estimates, all direct
consumers and a concrete split/cohesion decision for `package.rs` (11,851 lines)
and `dice.rs` (6,055 lines), before implementation review. This audit proposes
no final representation/API or cap by guesswork. No fallback is introduced;
guard removal is contingent on complete reviewed runtime ownership. Later
computed defaults, C++/Java providers/actions, aspects and execution remain
separate unsupported owners. The path-epoch performance issue is unchanged;
do not restart checkout-wide replay, exceed the user's test limits, disable
provenance or use a fresh graph to hide it.

## Accepted F3/R2 prerequisite order (2026-09-14)

The bounded package-attempt diagnostic exposed the authentic F3 terminal at
rules_java `toolchains/BUILD:138` -> rules_cc `cc_library.bzl:19`:
`target invocation for named execution-group semantics is unsupported`. The
same proof selected and executed one test in 9.98 seconds with valid observer
and cleanup evidence. This confirms both that the rules_cc owner is in the
portable configured closure and that no further source payload can complete F3
before group activation. The authenticated declaration already proves named
`cpp_link` plus `_use_auto_exec_groups=True`; both modes remain one shared
runtime obligation.

The former order made complete F3 a prerequisite for combined R2, while the
group runtime requires R2's selected-toolchain request correction. Break that
cycle by accepting the combined selected-request/output-conflict candidate from
its own complete non-F3 evidence, then implementing the reviewed shared group
runtime, then replaying F3 once. F3 is evidence for production-closure demand
and later configured-source completion; it is not an input to selected-request
identity or root-set output-conflict correctness.

Keep the preserved candidate atomic. The selected-request correction produces
distinct A/B owners that can collide at one configured output, so the root-set
validator is its required consumer-side completion rather than an adjacent
feature. Reconcile the app-only diff from `97dffd5d4..27e9e9c0c` onto current
`main`; exclude `review-evidence/`. That diff applies cleanly at this decision
checkpoint. Five files have changes on both lines (`analysis_v2/src/dice.rs`,
CLI build/tests, and Core dice/mod), with no textual merge conflict; semantic
review and fresh proof remain mandatory.

Combined acceptance requires every focused identity/resolution/conflict gate,
current source/configuration/platform/property A/B/A and cancellation/cutoff,
the one-shot and stable-daemon build/run/aquery pre-execution rejection proofs,
positive shared execution-view/REAPI proof, and complete affected owner/direct
dependent suites. The two recorded Core failures reproduced on clean
`97dffd5d4` are baseline facts only for their exact selectors and environment.
All other prior full-Core failures are unattributed: compare them with current
`main`, correct candidate regressions, and never waive an unexamined result.
Partition test execution under the standing limits rather than raising them.

After combined acceptance, resume the complete shared named/automatic design
recorded below. Remove the loading guard only when the configured-target-owned
group collection supplies resolution, dependency/provider/property/action
consumers atomically. Computed defaults and C++/Java semantics remain separate
later boundaries. Do not run F3 again before that guard is lawfully replaced.

Independent prerequisite-order design review returns `ACCEPT` for this sequence
and the atomic reconciliation packet. It does not accept candidate
`27e9e9c0c`. The review additionally requires the unrun real CLI consumer gates,
complete current-main/candidate Core attribution, a 12-second internal command
deadline, and a positive producer-validated closure-to-REAPI sharing proof.

## R2/group joint-acceptance correction (2026-09-14)

The reconciled trial invalidates the preceding R2-before-groups sequence. R2's
focused Analysis and Build API suites pass, all 329 active Core tests are
accounted with every nonpass compared exactly to current main, direct REAPI and
server consumers pass, and compile dependents pass. The required real CLI
one-shot, stable-daemon and positive closure-to-REAPI tests nevertheless enter
builtin `bazel_tools` transitive registration before reaching their R2 command
assertions. That path requires the retained named/automatic execution-group
declarations. Matching 12-second timeouts on current main are resource gates,
not semantic acceptance evidence.

Independent correction review returns `REVISE`. Complete group runtime cannot
land first on current main because it consumes R2's selected-platform request
identity and successful analysis must pass through R2's root-set closure
validator. Preserve reconciled R2 commit `f3c90ea46` unaccepted, freeze the
complete group contract against it, implement on top of it, and accept R2 plus
groups atomically after joint gates. Only then replay F3. The loading guard stays
until group resolution, transitions, providers, properties and action routing
are complete; no partial R2 or group behavior reaches main.

The 13 identical current-main/candidate Core assertions are attributable
baseline outcomes. Seven identical exact timeouts remain open and must receive
bounded discriminating proof. The attempted minimal local-module overrides
were reverted: they only bypassed authentic module sources and advanced to the
first missing autoload, so they provide no acceptance evidence.

## R2/group source-invariant reconciliation (2026-09-15)

`WP-4-6-7A-r2-execution-group-source-invariant-reconcile-r2` is independently
reviewed and Phase B ready. The pinned Bazel 9.2 audit preserves the configured
group owner and resolution architecture while correcting its boundary
projections: root automatic runtime names use `//pkg:type`, canonical
constraint keys use `@@//pkg:type`, and external canonical labels retain their
repository. Constraint keys parse in the target package and mapping; property
rows use exact runtime names; action toolchain strings also parse in the target
context while `ctx.toolchains` retains definition context.

Root actions lower executable/tools fields before group selection. A top-level
tools depset is flattened for executable-dependency association, while a depset
nested in the tools sequence is opaque. Provider equality controls omitted
toolchain ambiguity only for root actions. Subrules allow omitted or `None`
`exec_group`, reject non-`None` groups, reject every supplied `toolchain`, and
reject a matching executable File in favor of its `FilesToRunProvider`. An
omitted subrule toolchain becomes explicit `None` and selects Default.

Acceptance uses ordinary correctness proofs. Compile the corrected final Core
candidate first, then preflight and run all 330 active selectors separately;
earlier binaries do not qualify. Focused analysis, selected-request,
configured-conflict, direct REAPI/server and compile gates complete the
replacement boundary. Do not repeat the finished diagnostic chain, run an
authentic configured conflict selector or run F3 before atomic R2/group
acceptance.

## Core source/test-boundary reconciliation (2026-09-15)

The final corrected Core binary
`6001c0a41074422961a36c08f6e4618df37db8182504ccc059aa46b4632b83d6`
completed an atomic 330-selector approved-host receipt: 313 pass, eight
non-timeout assertion results and nine exact 12-second timeouts. The complete
receipt hash is
`0df68baf746ca3916614ee2f103a7a64ab2116d9cd60bf37248ce40927699d8d`.
All configured-action-conflict selectors pass, including the cross-group
addition, but one timeout is sufficient to reject acceptance. Preserve
implementation commit `1b09dbfa1` unaccepted; no merge or push occurred.

The next packet is proof-only. It freezes production and replaces the nine
over-broad public build/cquery tests with 34 smaller exact selectors mapped to
every assertion and semantic return path. Real public root and DICE lifecycle
claims keep their original owners and public command calls; test-only helpers
may factor fixture setup or returned-evaluation assertions but cannot synthesize
an evaluation or eliminate a command. Do not rerun the nine original names,
weaken assertions, lengthen deadlines, resume diagnostics, or run F3. After
independent design review, the replacement selectors must each pass within the
same 12/15-second limits before remaining R2/group acceptance gates resume.

The first materialized partition is preserved unaccepted at `678ff2e7c`.
Binary `66ce3e7472d9c9982139e6ad6c5c38cf4a46ee8c8abcaae9f24e25d899de9b16`
and receipt `3d2c7f0706c0760da702117ed47f9d9cb5a4f8e79be5e43be2630e28b5c59656`
record 25 exact passes, nine exact timeouts and zero assertion failures. Freeze
the 25 passing bodies and shared fixtures. Independent replan review accepts a
53-selector second partition that removes only the nine timed bodies, uses a
fresh fixture and at most three real public command evaluations per selector,
and preserves every expression, result class, assertion and edit order. Do not
rerun any first-partition name.

The accepted second-partition proof binary
`af7fa9c80829856b544d03cfb668df61d83592208044f8342f70ee3fd643396b`
compiled in 32.60 seconds. Exact preflight found all 53 selectors once and
nonignored. Approved-host receipt
`35c469ecd891903220ebde8bb7a13d16edf691155d6f9acfdb22c7e4ae6d3bef`
records 53 passes, zero failures and zero timeouts under the standing 12/15
second limits. Independent final review ACCEPT confirmed the frozen 25 bodies
and fixtures are unchanged, all public paths and assertion semantics remain,
and gross additions are 1,219 of 1,250. Preserve this evidence and resume the
deferred unchanged-main attribution and remaining predecessor gates.

Direct-consumer recovery is complete at feature checkpoint `055b6fe18`: both
required REAPI selectors pass in receipt
`1fca8402a39c0007f65ed57dc3c6a4de624e00d6ae992877cd9cc443ee7f691b`,
the required server selector passes in receipt
`1a61d473b634226b683cc42ca945db1237c8cf2fae9051671c162ba182845076`,
and query/server/CLI compile linkage passes. Final predecessor review still
returns REPLAN because the eight candidate Core assertion failures lack an
unchanged-main classification. Six resumed 60-second main preparations ran no
tests and produced no binary, repeatedly stopping during `slug_bzlmod_v2`.

The docs-first successor freezes every passing artifact and allows exactly one
same-command main Core preparation at clean `4824a0861` under a 180-second
compile-only ceiling. It changes no test deadline. Success requires complete
Cargo JSON, one Core executable and its hash; failure cannot retry. Only the
eight frozen failure selectors may then preflight and run once under the
unchanged 12-second TERM/three-second kill limits. Every main row must reproduce
the candidate exit/timing and material assertion family. Any pass, timeout,
missing selector, different failure or incomplete receipt replans. Merge and
push remain prohibited pending successful attribution and final atomic review.

The accepted r1 recovery's single 180-second preparation exited 124 while
compiling `slug_core_v2` after dependencies completed. Receipt
`232d6ce07b1cfaefaf8ddfde359d3c728751e26281c780534fd3f930ec5684c7`
contains 366 compiler artifacts, 51 build-script rows, 17 messages, zero errors,
zero executables, no build-finished row and zero tests; cleanup and main state
are clean. Final r2 resource recovery authorizes one identical continuation
with the warmed target under a 360-second compile-only ceiling, without any
clean, feature/job/profile/incremental change or intervening package build.
Success requires complete JSON, one successful build-finished row, exactly one
hashed Core executable and clean cleanup. Another stop ends this evidence path
as an external resource blocker. Only after success may the frozen eight main
comparators run once under 12-second TERM/three-second kill limits; all other
tests, source changes, merge and push remain prohibited.

The final r2 continuation succeeded in 81 seconds. Compile receipt
`97ac6aa2b4bacb80c30107631b62cd0c8610eeebcd4a6b0e1c94a86c56378978`
contains zero errors, one successful build-finished row and one main Core
binary, SHA-256
`31b941d3ec87a59a6cae2d7c399bd2ee6fb0182cd5284c54c35d957580fb149e`.
Exact preflight admitted only the frozen eight. Main receipt
`7f87d741e0ad8edaa08b9d98f13b4488f478c1a8afa1704264e5651a73dca68c`
records eight exit-101 results with no timeout or forced kill. Comparison
receipt `9819e9487d30d089ffa6b59900a26c1ea5f10990c698b6047242162e86ca2eb6`
matches every main row to the candidate by selector, timing class, terminal
assertion/panic family and material expected/actual diagnostic. These eight
outcomes are baseline-attributed; preserve all evidence for final atomic review.

Independent final atomic review ACCEPTS checkpoint `71d9ce4bf`, fast-forwarded
from main `4824a0861`. The selected-request/configured-action-closure and
named/automatic execution-group owners, source-invariant corrections and
reviewed loading/query/consumer prerequisites are accepted as one atomic
boundary. Focused owner suites, partitioned Core proofs, direct REAPI/server
consumers, query/server/CLI linkage and all unchanged-main comparisons are
complete. The eight exit-101 comparisons remain baseline debt. Authentic F3 is
the next separately selected post-activation packet; no broader action-family,
C++/Java or exact identity boundary is implied.

## Configurable native alias prerequisite (2026-09-14)

After the computed-default correction, authentic F3 reaches rules_java's
configured `alias.actual = select(...)`. The selected correction reuses
`prepare_configured_attribute_conditions` and `resolve_configured_attribute` in
the existing native alias arm. The alias's own target or exec configuration
selects one label, and the child keeps that same configuration. Condition nodes
remain configured dependencies; only the selected label becomes the
`AliasActual` edge. Missing, ambiguous or wrong-shaped selections fail before a
configured alias result is published.

This adds no transition, key, cache, resolver or command repair. Loading's
retained expression is the semantic input, existing condition DICE keys own
Need/error/invalidation behavior, and the configured alias result owns the
selected edge. Target/exec A/B/A, condition dependencies and stale-publication
negatives are mandatory before the combined R2/group stack can be accepted.

## Configurable alias checkpoint and later loading stop (2026-09-14)

Configured alias selection now reuses the existing condition preparation and
attribute resolver, publishes one selected `AliasActual` edge in the alias's
exact Target or Exec configuration, and carries condition packages only in the
runfiles closure. Exact A/B/A, Need, invalid-selection and query proofs pass;
full analysis is 168/168. Authentic F3 proceeds beyond this configured owner and
stops during later rules_java loading at rule-initializer invocation. Stage 6
adds no initializer owner; the R2/group/alias stack remains atomic and
unaccepted while Stage 4 freezes the successor.

## Configured condition alias for toolchain settings (2026-09-24)

WP836 admits the exact selected-condition truth and toolchain eligibility
needed by generated `rules_java` 9.1.0 `target_settings` aliases. The existing
configured condition owner and declaration parent both walk native alias
`actual` through current selector preparation, DICE-load each visited package,
and require a selected `config_setting`; the requested label remains the
selector/toolchain edge while the selected declaration supplies truth and
specialization. The parent directly depends on alias/leaf packages so an
equal-truth selected-leaf switch still invalidates. A transient visited set
rejects cycles in plain alias chains; the existing configured cycle detector
covers selector-condition recursion. No new key, retained representation, generic provider
projection or configuration identity is admitted. Focused target/exec,
toolchain, wrong-kind, cycle and A/B/A tests pass; authentic bootstrap proceeds
to generated `rules_cc`'s unsupported `repository_ctx.symlink`. M7A remains
partial.

## Configured constraint-setting defaults

WP814 admits the Bazel 9.2 NODEP default for configured constraint settings
with absent refinement. Analysis validates same-package target existence from
the observed loaded package, not by configuring the default target; a value
may point back to its setting without a cycle. The configured setting result
retains a pointer-sized Arc default fact in structural equality. Requested
constraint values share that fact; platform constraints remain explicitly
declared only. Condition and toolchain matchers choose an explicit platform
value first, otherwise compare the requested value with its setting's default.
`use_target_platform_constraints` requests only the target platform's explicit
values, then applies that same effective-value comparison on execution.
Same-DICE default edits restore both condition and toolchain decisions.
Nonempty refinement fails closed; Starlark providers, full
`ConstraintCollection`, alias-valued defaults and implicit source-file target
materialization remain outside this configured slice. The pure `ConstraintSet`
helper is not the DICE path and gains no parity claim.

## Bare Starlark rule-implementation None

WP815 normalizes a bare/implicit `None` return from an ordinary
non-executable rule to zero explicitly returned providers. The existing
no-explicit-DefaultInfo branch synthesizes the effective provider, including
predeclared files, without losing registered actions. `[None]` and other
invalid list elements still reach the provider decoder and fail; bare/listed
Info and empty-list returns are unchanged. This is evaluator scratch only:
no DICE key, retained provider identity, cache or lock changes. Tuple returns
and executable-rule default-output semantics remain deferred.

## Attached-aspect selected Analysis boundary

WP826 retains a nonempty attached-aspect edge in Loading without claiming
configured aspect semantics. A selected Starlark rule with that retained row
returns a typed unsupported Analysis result immediately after the loaded-rule
lookup, before transition, selector, child dependency, toolchain or rule
implementation work. The ordinary base edge remains available to unconfigured
Query only; Analysis cannot silently consume plain dependency providers as a
substitute for the aspect overlay. The zero-edge WP825 path remains eligible.
The selected fail-closed, sibling and zero-edge controls pass; configured
aspect application, generated providers and actions remain deferred.

## Default Apple rule-fragment ingress (WP849, 2026-09-25)

Configured root rules declaring `apple` now receive an evaluator-local
facade over the existing structural `SlugConfiguration` option vector.
Only the pinned `apple_support` 1.24.2 empty-Xcode-config path's null
Xcode/SDK/minimum-OS flags and two option-backed booleans are exposed;
non-null dotted/text state fails closed before rule invocation. The
existing declaration and active-call gate owns access, while subrule/aspect
Apple fragments remain unsupported. Target/exec and both Analysis routes,
Loading lifetime negatives and whole-configuration A/B/A tests pass.
Authentic cquery clears `ctx.fragments.apple` and next stops at
`apple_common.dotted_version` in `xcode_config.bzl:400`. Provider
construction, Xcode selection and generated repository semantics are
not admitted.

## Default XcodeVersionInfo provider (WP851, 2026-09-24)

The pinned `apple_support` empty-Xcode path now authenticates its exact
14-keyword `apple_common.XcodeVersionConfig` tuple and publishes a typed,
immutable builtin `XcodeVersionInfo` provider. A configured dependent can
index that key and call all five source-backed methods; SDK and minimum-OS
results are typed DottedVersions, with Catalyst's distinct macOS SDK and iOS
minimum. `apple_common.platform_type` exposes six pinned string members.
Invalid tuples and a generic occurrence spoofing the builtin identity fail
closed; Objc remains declaration-only. Source A/B/A on one DICE passes.
The rebuilt authentic cquery clears `xcode_config.bzl:98` and next stops at
`rules_cc`'s unmodeled `ctx.fragments.apple.single_arch_platform.platform_type`.
Non-default Xcode, generated repository bytes and configured C++ remain open.

## Default Apple single-arch platform (WP852, 2026-09-24)

The existing structural Apple fragment now lazily validates the default
`apple_platform_type="macos"`, empty split CPU and empty macOS CPU list
when `single_arch_platform` is accessed. It exposes a distinct typed
`apple_platform` MACOS value, not the separate `apple_common.platform.macos`
struct: four pinned fields, lowercase repr and enum-like equality/hashability.
Nondefault selectors fail closed on this attribute without narrowing the
previously admitted fragment fields. Target/exec Legacy/Observed and real
configured Xcode minimum-OS tests pass. An isolated authentic generated
rules_cc cc_toolchain_config cquery succeeds through the Apple/Xcode call;
the broader Rust-toolchain cquery next stops at `@@platforms//os:none` being
treated as a config_setting. That selector and full configured C++ remain open.

## Native alias direct constraint selectors (WP853, 2026-09-29)

Configured native alias `actual` selectors now accept direct constraint values,
including alias keys, target/exec platforms and omitted-setting defaults.
Eligibility participates in the sole condition key's structural identity;
nested aliases obtain their own eligibility while an ineligible outer owner
still rejects a terminal constraint. Existing observed constraint/platform
owners retain source dependencies. Direct matching keys are peers; equal
selected values pass and differing values are ambiguous. Mixed direct and
config_setting multi-match fails closed, with existing config_setting
specialization preserved. Fourteen focused tests and independent reviews pass,
including owner cache separation, Need/cycles and same-DICE platform/source/
default-source A/B/A. The unchanged rebuilt authentic Rust-toolchain cquery
clears the `@@platforms//os:none` condition error and next stops at missing
`@@bazel_tools//tools/def_parser`. Other owners and full toolchain/C++ closure
remain deferred.

## Native filegroup direct constraint selectors (WP854, 2026-09-29)

Native srcs-only filegroup now shares WP853's structurally eligible condition
key with native alias; pinned FilegroupRule inherits enabled toolchain
resolution and owns a target platform. Effective/default constraint matching,
requested-key identity, alias resolution, peer ambiguity and mixed-match
refusal remain with the existing owners. Other nondefault filegroup attributes remain
unsupported. Target/exec matching and platform/selector/default-source A/B/A
pass, with existing forwarding, source lifecycle, refusal and alias controls.
The actual catalog def_parser forwards no_op.bat with one null source edge,
unchanged DefaultInfo/FilesToRun and zero actions through canonical and root
apparent routes. Linux selector safety does not claim Windows execution.
The unchanged authentic Rust-toolchain cquery next reaches cpp.grte_top at
rules_cc cc_helper.bzl:844; that field and full configured C++ remain open.

## Private C++ libc option getter (WP855, 2026-09-29)

The root/subrule C++ facade exposes grte_top() through the existing validated
nullable label projection. Both exec entry points map it from retained
host_grte_top as pinned builtin_exec_platforms requires; target state and the
existing LibcTop configuration_field share the same owner. No new option parser,
DICE key, fragment state, libc target lookup or action is introduced. Host
command admission stays deferred. Nineteen focused Configuration/Loading/Analysis
checks cover typed/error state, private current callers, occurrence identity,
config/source restoration and both exec paths. Independent reviews pass.
The rebuilt unchanged authentic Rust-toolchain cquery clears grte_top and next
stops at cpp.experimental_cc_implementation_deps() in rules_cc semantics.bzl:114.
M7A and full C++/toolchain closure remain partial; Windows execution is deferred.

## Private C++ requested-option getters

Retained CppOptions booleans back the private zero-argument methods
experimental_cc_implementation_deps, start_end_lib,
incompatible_use_specific_tool_files, incompatible_remove_legacy_whole_archive
and save_feature_state and their appended typed command options.
The first four default True; save_feature_state reads experimental_save_feature_state
and defaults False. All carry through the existing exec mapping. The specific-tool
files getter delegates to the existing cpp_compile_use_specific_tool_files
policy owner used by compile-action construction; no second producer is added.
Bazel9.2 CppConfiguration:468/866, CppOptions:150/970 and
builtin_exec_platforms:303/305 pin the first two projections; specific-tool
files uses CppConfiguration:790-796, CppOptions:828-837 and exec:313. The remove-
legacy-whole-archive projection uses CppConfiguration:484-491, CppOptions:790-799
and exec:309; it reads the existing retained Boolean and adds no whole-archive
heuristic. The save-feature-state getter uses CppConfiguration:782-783/934-937,
CppOptions:819-826 and exec:324; enabled feature-state artifacts and output-group
production remain unproved. Requesting start_end_lib
does not establish toolchain feature support. Shared textual Boolean conversion
rejects literal null/NULL; actual-null default materialization remains False.
Configuration/config_setting use the same converted structural state. Current
invoking private callers are checked even for transferred bound methods, which
retain their original configuration. Crossed-value root/subrule and target/exec
checks prove independent getters and same-DICE source/configuration restoration.
There is no command-side repair, new state owner or action registration.
Implementation-deps, start/end-lib and archive/link tool-input execution remain
unproved; Windows execution remains deferred.

## Disabled coverage APIs

Public ctx.coverage_instrumented accepts omitted/None or a genuine Target and
returns False when the caller's retained structural configuration disables
coverage. Argument validation and invocation-token checks precede projection;
an extracted method expires after successful or failed evaluation. Enabled
coverage refuses explicitly, retaining filter/test/provider ownership for a
future packet. A dependency Target cannot replace the caller's coverage state.

Public coverage_common.instrumented_files_info binds ctx positional/named and
all eight other fields named, preserving omitted versus suppliedNone. Pinned
Bazel9.2 CoverageCommonApi45-128/CoverageCommon47-110 require source/dependency/
extension strings, metadata/baseline Files, environment string dict, support
Files/nested File depsets and reported top-level tuples; empty ranges/depsets
are admitted as applicable. Only actual nonempty support/report/environment
contents require the existing default private allowlist, even when disabled.
Validate before Collector107-109's disabled shortcut; no attribute/dependency
traversal or baseline action. Enabled collection remains refused.

Existing evaluator root registration checks the exact supplied ctx and current
marker stack, followed by active root token. This current/live policy is
Slug-native; the upstream collector only rejects a closed/null context. A
synchronous borrowed service factory checks genuine AnalysisContext/sink/token
and original CcContextProjection configuration, then uses the unchanged native
InstrumentedFilesInfo.EMPTY materializer on the caller's frozen heap. Existing
ProviderOccurrence identity/lowering/rematerialization owns surviving facts;
no genericstruct, duplicate provider class, heap/Value retained in sink, new
DICE state or action is introduced. Both-route source/configuration restoration,
both exec resets, provider hashing/key membership/dependency rematerialization,
private negatives and original suspended-live context recovery protect this
boundary. Full coverage/linking/bootstrap remains unproved; Windows deferred.

## Default link-actions wrapper (WP858, 2026-09-30)

The private link facade composes over the original AnalysisActions handle.
Default positional None/False binding returns a fresh occurrence with the same
sink/token/context name/scope; the shared ordinary method table and genuine
base/wrapper unpack preserve declaration, registration and context recovery.
Wrapping permits suspended live frames, while ordinary mutations retain their
original top-frame restrictions. Wrapper-only shareable declarations and
alternate construction modes refuse explicitly. No new semantic action owner,
registry/configuration or graph state is added. Seventeen focused checks and
independent reviews prove shared FileWrite ownership, conflicts, caller/scope/
expiry checks, both-route same-DICE restoration and publication refusal. The
unchanged authentic cquery clears wrapping and next reaches dynamic_library_soname
on the PIC static-library path. Sharing, helper counters and full link actions
remain deferred; the facade is not complete Bazel wrapper parity.

The link metadata intern_seq surface uses existing sequence iteration,
AllocTuple and structural immutability capabilities, including user-provider
fields. It accepts genuine list/tuple/range with recursively immutable elements,
captures the outer sequence and preserves original element types/values as
Slug-native. Required positional binding and outer tuple length/slot order
match CcStarlarkInternal:397-405. Mutable elements refuse; upstream weak
representative substitution (also observable for equal mixed numerics/signed
zero), global canonical sharing and performance parity remain deferred. No
new interner/cache, retained Value owner or publication rule is introduced.

The immutable get_link_args adapter retains genuine native feature configuration
and original traced/freezable variables. Four required named inputs and mutation
refusal/module freeze match CcStarlarkInternal:408-440/Args:119-240. None remains
lazy at construction; supported string modes derive the first literal-placeholder
policy immediately. Consumption checks linker_param_file before expansion and
filters every matching argument, including None/empty/Boolean conversion cases;
percent escaping and exactly-one-placeholder validation precede retained policy.
None/UNQUOTED/SHELL_QUOTED reuse existing serializers. GCC_QUOTED appends the
retained Gcc format and pinned nine-character ASCII backslash encoder, including
empty arguments and one LF per argument; raw argv stays unchanged. Rust valid-
Unicode/UTF-8 storage remains Slug-native. All modes default to shell when
no policy is produced. Loading consumes on the current heap after its binding/
lifecycle checks and hands scalar recipes to the original ArgsWrite/Spawn sink.
Registration snapshots and later sink error ordering are Slug-native; upstream
execution-time expansion, nested structural classification/identity hashing,
multiple-placeholder flags, arbitrary legacy byte strings and Windows format
remain deferred. No new
retained action/key/cache/identity owner or native link action family is added.
Both-route root/subrule snapshots, forced parameter bytes, source/configuration
restoration, original-variable freeze and actual GC protect this handoff.

The sole pinned rules_cc0.2.17 link resource callback admits registration only:
concrete Def plus unique canonical/parsed/evaluated source identity and exact
retained opening call span authenticate its fixed module-global expression.
Failed authority remains ordinary late unsupported handling. SpawnSpec retains
one compact immutable kind in publication equality, without callback Value,
heap or estimate. Root wrapped registration and both-route source/configuration
restoration use unchanged public finalize; genuine child preserves earlier
suspended-root workspace_name refusal. Core source/prerequisite input traversal
and shared REAPI Spawn projection refuse this metadata before execution/CAS;
generic callbacks, estimates, resource overrides and scheduling remain deferred.

## Library SONAME

The private link facade owns dynamic_library_soname wrapper validation. Three
required positional arguments and True-mode Linux normalized basename match
pinned Bazel9.2 CcStarlarkInternal:247-253/SolibSymlinkAction:306/UnixOsPathPolicy:65.
A borrowed reverse segment reduction allocates only the resulting primitive
string; existing current/live/registration checks remain Slug-native. Static
archive names are admitted without suffix filtering. False uses existing Linux
NormalizedBazelPath and shared unshortened Solib lib+escaped-full-path formatter;
this is explicitly Slug-native without Bazel ST configuration mnemonic bytes.
Each caller supplies its original path: external short_path and root-relative
Solib input spellings remain distinct. False/False Solib constructor bytes,
shortening and full configured input identity remain unchanged. ST naming, exact configuration/
output identity and non-Linux path policy remain deferred. Existing root/subrule
proof retains True basenames, library sequences and FileWrite identities while
adding False strings under both-route source/target/exec restoration. No new
retained semantic owner or action mutation; full link closure remains unproved.

## Historical evidence

The original 23,981-line owner record is retained at `c5e7414d7`.
[Plan history](../plan-history.md) indexes the exact baseline sections and
acceptance inventories. The R2 snapshot is commit `27e9e9c0c` on
`review/output-conflict-r2`, with base `97dffd5d4` and original patch/receipt.
No preservation or document edit accepts that candidate.
