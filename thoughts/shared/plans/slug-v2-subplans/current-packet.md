# Current Slug V2 Work Packet

Packet: WP-7-172-m7a-action-file-sibling-declaration-r1
Status: accepted; independent DESIGN/FINAL accepted; M7A remains partial

## Outcome and compatibility

Baseline `ed00e59da` (WP871). Admit genuine File `sibling` for the existing public
`ctx.actions.declare_file(filename, *, sibling=None)` method and current native
declaration sink. Advance the unchanged authentic Rust-toolchain query beyond
rules_rust private/utils.bzl:516. Exact bounded Linux File binding, parent-relative
path normalization, componentwise package confinement and current-context ownership;
structural artifact/configuration identity and output paths remain Slug-native.
No sibling root/owner is copied. Files may originate from other rules/configurations
or be source/directory/status Files; location alone determines eligibility.
Directory declarations with non-None sibling, alternate sibling-repository layout,
backslash/NUL path spellings and full action/execution/bootstrap closure remain
unsupported/deferred. Windows keeps its existing ordinary declaration behavior and
a minimal explicit sibling refusal; no Windows implementation or test expansion.

Pinned clean Bazel 9.2 `8220c619`: StarlarkActionFactoryApi.java:118–153 declares
filename positional/named and sibling named-only genuine File|None default None.
StarlarkActionFactory.java:141–162 checks mutable context, uses package directory
without sibling or Artifact.getOutputDirRelativePath(...).replaceName(filename),
checks componentwise package confinement, then current ruleContext/newFileRoot.
Artifact.java:507–508/783–785 uses root-relative derived paths and nonsibling-layout
source execution paths; status File root-relative names match short_path.
PathFragment.java:775–786 and PathFragmentTest.java:337–355 show replaceName uses
parent.getRelative, including nested/dot/parent/empty/absolute replacements; docs'
base-name wording does not replace that behavior. StarlarkRuleContextTest.java:
1144–1189 covers nested declarations and tool source siblings; directory-sibling
API examples are source guidance only while that public method remains deferred.
StarlarkIntegrationTest.java:3896–3950 covers outside-package sibling and absolute
output refusal. Existing rules_rust utility's following symlink call already has a
genuine native binder and action owner; declaration must not fabricate contents.

## Owner and invariants

Loading's AnalysisActions binder validates its existing active token before reading
or delegating the sibling. Forward a synchronous Option<&AnalysisArtifact> through
the existing AnalysisActionSink.declare_file method; no new facade/default bridge.
The same binder supports wrapped link-action facades and subrule scope; suspended
root or expired handles cannot mutate the sink. Bind the first argument as filename,
matching Bazel; historical path= spelling has no migration obligation.

Analysis SynchronousAnalysisActionSink remains the only declaration owner.
Source sibling uses its existing nonsibling-layout source path; derived sibling
uses output.root_relative_path, status File uses status.short_path. Derive only the
parent directory, combine a relative filename or keep an absolute replacement,
normalize through existing NormalizedBazelPath Unix owner, and enforce current
repository/package prefix componentwise (equality or prefix followed by slash;
empty root-package prefix admits relative paths). Refuse absolute paths and existing
unsupported output spellings before declaration state. Keep validation, declaration
SmallMap interning/type conflicts, current DerivedArtifactRoot/current configured
owner and action registration with declare_bin_relative_output. Do not require
sibling-owner equality and do not use execution-path configuration prefixes.
No action is created by declaring a File. Existing write/symlink registration and
input identity remain authoritative and may consume the resulting declaration.

New path strings/borrowed sibling are call scratch; existing evaluation-scoped
SmallMap holds declarations until evaluation exit, then only immutable structural
Derived artifacts/actions/providers survive. No new DICE key, retained collection,
interner, cache, lock, async task, evaluator heap borrow or Host observation.
Existing token, declaration mutex and registry validity/release boundaries apply;
no lock spans a DICE computation. Reuse the existing path and compact declaration
utilities; no donor or performance claim. Stage 9 default link-action phase facade
reuse (WP858) and DICE ownership doc apply. Existing large production modules remain
the binding table and synchronous sink owners; a small method extension does not
justify adjacent extraction. Independent DESIGN/FINAL required for public sink and
artifact/lifecycle boundary.

## Scope and proof

Allowlist: Loading src/subrule_invocation.rs (trait + declare_file binder only),
its link_actions/tests.rs, and signature-only test sink implementations in
builtin_restriction_tests.rs, cc_common/solib_tests.rs, build_info_transform/tests.rs;
Analysis src/starlark_rule.rs (declare_file only), existing context_lifecycle_tests.rs,
signature-only analysis_value/cc_features_tests.rs and starlark_rule/cargo_runfiles_tests.rs,
and two signature-only callers in starlark_rule/build_info_transform/tests.rs;
Analysis tests/starlark_rule.rs one focused genuine integration test (reuse authored
scratch/root helpers), manifest/canonical and Stage 6 current declaration paragraph.
No action registry, artifact identity, source materializer, configuration/path utility,
CLI or dependency changes. Estimate production <90 and proof <250 net Rust lines;
reuse existing fixtures and only tiny authored rule/source files, no copied module.

Loading binding proves positional/named filename, omitted/None/genuine mutable and
frozen Files, named-only sibling and invalid types, forwarding to sink, wrapped and
subrule active scope, suspended/expired bound-method refusals before sink mutation.
Analysis real sink proves nested source sibling, other-owner/config derived sibling,
directory and status File locations, current root/owner, repeat interning, type
collision, normalization-equivalent declarations, componentwise confinement and
absolute/path refusals with unchanged declaration state, no own actions. Root,
nested and external packages and target/exec configurations must preserve owners.
One genuine rule integration uses ctx.file input and derived dependency sibling,
then existing symlink registration; both analysis routes and same-DICE source/
declaration/configuration A/B/A prove action input/output ownership and restoration.
Do not claim execution success. Existing context teardown and wrapper tests remain
protected; public directory sibling refusal remains explicit.

Root serial pinned Cargo JSON Loading/Analysis lib and Analysis starlark_rule no-run
60s; exact ordinary nonignored preflight and focused runtime 20s per executable;
Core check and CLI rebuild before unchanged three-hash authentic 180s supervised
replay. Observed 83.533s setup is necessary to verify real toolchain progress;
small authored tests cannot establish it. Actual process cleanup, scoped format,
diff/archive/status and fixture hygiene gates. These are operation caps, not user
limits. REPLAN only for a new artifact/path owner, copied sibling root/configuration,
lifecycle bypass, broad directory/Windows activation, or genuinely unsupported
required path semantics. Later query refusal is successor evidence, not closure.

## Predecessor

WP871 `ed00e59da`: independent DESIGN/FINAL, 15 focused tests in 1.165s, Analysis
compile 7.03s, Core 4.33s and CLI 11.85s. Authentic 83.533s exit 2 clears implicit
host-tools source admission and reaches declare_file sibling. Original hashes and
actual cleanup pass; source/macro ownership retained. Receipts: target/wp871.

## Validation receipt

Independent DESIGN accepted current-context ownership, Unix normalization and the
borrowed public sink argument. Actual production 70/proof 552 net Rust lines
versus <90/<250 estimate. The proof growth is the required independent binder,
owner/path/kind matrix and both-route source/declaration/configuration restoration,
plus mandatory recording-sink and signature-only adapters; one new binder selector,
one native-sink selector and one genuine integration selector reuse existing modules.
No copied fixture/module asset or authentic workspace mutation; tiny authored root,
nested and external repository fixtures only. No Windows proof or implementation
expansion; the sibling branch is a minimal explicit refusal.

Analysis lib preparation initially found two existing build-info test callers that
needed an explicit None sibling. They are signature-only and now in the allowlist;
production design unchanged. Initial Loading proof imported a frozen symbol as
private, then used Module.get (public-only); binding now explicitly republishes the
same owned frozen value after retaining its heap reference, preserving the mutable/
frozen discriminator. Initial integration expectation used plain to_exec instead of
selected host-platform execution configuration; corrected to the existing exact
transition owner, with full equality. A subsequent external fixture preparation
Need was corrected by authored empty REPO.bazel/.bazelignore like existing helpers.
These proof/invocation failures remain recorded and are not accepted gates.

Final pinned prepares: Analysis lib 6.76s, Loading lib 4.83s, Analysis integration
7.15s. Exact ordinary nonignored preflight/runtime: Analysis 8/8 in 0.114s,
Loading 6/6 in 0.016s and integration 1/1 in 0.314s (15 focused, 0.444s total).
Native ownership/location/interner/kind guards, wrapped/root/subrule lifecycle,
source and exec-derived symlink ownership, both routes and complete A/B/A
restoration pass. Core check 3.39s and CLI rebuild 9.90s pass.

Unchanged authentic query 76.973s exit 2 clears sibling declaration and native
symlink construction; next refusal is rules_rust toolchain.bzl:142 legacy Target.files
missing in _symlink_sysroot_tree. All three original hashes unchanged and actual
pre/post process checks empty. This is progress, not query/execution/bootstrap
closure. Receipts and preserved failures: target/wp872.

Independent FINAL accepted current-context ownership, location-only siblings,
lifecycle/path guards and justified proof growth. Scoped rustfmt, git diff --check,
archive/status and plan consistency gates pass. Packet window approximately
13:48–14:26 UTC (38 minutes, including independent review and proof corrections).
Observable gate: genuine sibling declarations and following symlink construction
now pass in the authentic toolchain; legacy Target.files is the next frontier.
No performance, execution or milestone-closure claim.
