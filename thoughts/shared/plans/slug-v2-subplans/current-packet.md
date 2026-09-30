# Current Slug V2 Work Packet

Packet: WP-7-173-m7a-target-files-default-projection-r1
Status: accepted; independent DESIGN/FINAL accepted; M7A remains partial

## Outcome and compatibility

Baseline `bbe0c7ddd` (WP872). Expose legacy `Target.files` for admitted immutable
Targets that actually provide DefaultInfo, by returning their existing frozen
DefaultInfo.files view. Advance unchanged authentic Rust-toolchain query beyond
rules_rust toolchain.bzl:142. Exact bounded default-bearing Target files/depset
projection at Bazel's default legacy-field policy; structural Target/configuration
and artifact identity remain Slug-native. This is not a files-to-run, executable,
runfiles, output-group or action-discovery union. Other legacy Target fields and
Targets without DefaultInfo remain unsupported/deferred. Package-group legacy
FileProvider.EMPTY requires a separate provider-boundary decision; do not fabricate
DefaultInfo or a fallback empty graph here. Windows stays minimal/deferred.

Pinned clean Bazel 9.2 `8220c619`: ConfiguredTarget.java:44–45 names files;
AbstractConfiguredTarget.java:125–140 gates default legacy fields on
incompatible_disable_target_default_provider_fields, :185–200 reflects fields,
:203–224 returns getDefaultProvider().getFiles. DefaultInfo.java:149–159 uses
FileProvider.filesToBuild. StarlarkStringRepresentationsTest.java:201–202 reads
source/generated Target.files; StarlarkRuleImplementationFunctionsTest.java:
563–590 supplies executable Target.files as depset tools/inputs.
BuildLanguageOptions.java:422–434 defaults the disable flag False. Slug has no
modeled flag or semantics owner for it: NativeCommandOption.from_name does not
admit it; commands/common.rs:518–594 returns no occurrence, cquery.rs:122–156 then
explicitly refuses unknown flags. Preserve/prove refusal for true/false/no-form;
do not silently accept a flag or pretend the current default projection models it.
PackageGroupConfiguredTarget.java:42–43 supplies FileProvider.EMPTY upstream;
Slug package_specification.rs:56–61 intentionally provides PackageSpecificationInfo
without DefaultInfo. Missing-default reflection/field access stays refused in this
bounded packet, explicitly distinguishing a deferred upstream behavior.

## Owner, invariants and scope

AnalysisConfiguredTargetValue in analysis_value.rs:443–501 is the sole immutable
Target facade, retaining ConfiguredTargetValue and a SmallMap of frozen provider
views. Its existing materializer allocates DefaultInfo.files at :708–715. Return
that exact FrozenValue through the existing BuiltinProviderView fields; add files
to reflection only when it is genuinely available. Preserve label, provider-key
lookup/membership, unknown-field refusal, Target equality/hash by identity and
provider publication equality. No second depset or retained field/cache, and no
allocation of a replacement File graph. Existing target lowerer :981–983 retains
the original semantic value; ordinary lowering/rematerialization must preserve
source Null, generated/configured Target and File identity/order/content. Target
and files are immutable data valid after producer teardown; no action token should
be introduced. DICE-configured providers already own selected contents/invalidation,
so source/provider/configuration A/B/A must restore full existing results without
fresh-graph or global state bypass.

Getter/reflection handles are call scratch; provider heap ownership/references and
semantic graph publication/release stay with existing materializer/lowerer and
ConfiguredTargetValue/AnalysisDepset owners. No new retained representation,
collection, interner, Host observation, graph key, lock, action or async owner.
Buck2 utility skill applies no donor change because storage/representation is
unchanged. The large analysis_value.rs module remains its existing facade and
materializer/lowerer owner; small field projection does not justify extraction.
Independent DESIGN/FINAL required for public immutable Target boundary.

Allowlist: Analysis src/analysis_value.rs (Target getter/reflection and focused
unit/roundtrip proof), existing tests/native_filegroup/{defs.bzl,fixture.toml},
existing tests/file_output_providers/mod.rs, Commands tests/commands.rs cquery
parser negatives; manifest/canonical and Stage 6 current Target paragraph.
No Loading, provider/action/identity/configuration, CLI or dependency changes.
Estimate production <25/proof <120 net Rust lines. Reuse existing authored fixtures,
no new copied assets or generated-fixture mutations.

## Proof, validation and stops

Unit proof must distinguish explicit DefaultInfo.files from an independent
executable/files-to-run set, preserve ordered source/derived File identities and
Target identity/hash, and prove pointer reuse of the existing frozen files view.
Empty DefaultInfo files is admitted; missing-default Target does not gain synthetic
provider, files or reflection. Unknown fields and provider-key failures remain
closed. Extend existing source/generated nested Target roundtrip through publication
and a second heap; new files projection must survive original heap release.
Genuine native-filegroup observer/consumer uses direct and nested captured Target.files
and compares it to DefaultInfo.files and ctx.files; existing root/external target/
exec, provider/source/configuration A/B/A and zero-action proofs must remain green.
Existing file-output-provider capture must distinguish source/alias/generated File
Targets from a producer whose explicit files omit its predeclared output, carrying
transitioned File owners and validation behavior. Preserve declaration/sibling
proofs and prove legacy flag remains an explicit public command refusal.

Root serial pinned Cargo JSON Analysis lib/starlark_rule and Commands commands no-run
60s, exact ordinary nonignored preflight/runtime 20s; Core check and CLI rebuild
before unchanged three-hash authentic supervised 180s query replay. Its ~77s setup
is necessary for actual toolchain progress; authored projection tests cannot verify
bootstrap. Actual process cleanup, scoped format/diff/archive/status and fixture
hygiene gates. Operation caps are not user limits. REPLAN for a new retained Target
fact/semantics key, silent nondefault flag admission, fabricated provider/files,
changed identity/lifetime or need to activate missing-default legacy behavior.
A later query refusal is successor evidence, not query/execution/bootstrap closure.

## Predecessor

WP872 `bbe0c7ddd`: independent DESIGN/FINAL, 15 focused tests in 0.444s, Core3.39s
and CLI9.90s. Authentic76.973s exit2 clears sibling declaration/native symlink and
reaches Target.files; original hashes and actual cleanup pass. Windows sibling is a
minimal refusal, directory sibling/full closure remain deferred. Receipts target/wp872.

## Acceptance

Independent DESIGN/FINAL ACCEPT. Target.files returns the existing DefaultInfo
FrozenValue; conditional reflection shares its availability check. No new graph,
retained field, action token, configuration input or identity owner. Production
net +18 Rust lines; proof +178 Rust/+10 Starlark exceeds the +120 Rust estimate
because pointer/order, explicit executable separation, empty/missing providers,
identity/hash and original-heap-release each discriminate a separate invariant.
Existing genuine fixture publication/restoration and producer-vs-generated files
proofs justify the remaining additions. No new copied assets or setup mutation.

Pinned serial preparation: Analysis lib6.41s, starlark_rule14.66s, Commands1.77s;
exact ordinary nonignored preflights pass. Analysis6/6 in0.016s, Commands3/3 in
0.001s, integration19/19 in1.416s: 28 focused tests in1.433s. Core check9.14s,
CLI rebuild11.59s pass. Unchanged authentic79.222s exit2 clears Target.files and
reaches duplicate sysroot symlink output at toolchain.bzl:156. Three original
hashes and supervised/actual process cleanup pass. This is successor evidence,
not bootstrap closure; do not relax duplicate-output ownership without review.
Scoped fmt, diff, archive/status and fixture hygiene pass. Receipts target/wp873.
Packet window from predecessor commit14:27UTC to acceptance about14:56UTC (~29m);
no workflow-performance claim. Windows and previously named boundaries stay deferred.
