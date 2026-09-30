# Current Slug V2 Work Packet

Packet: WP-7-154-m7a-def-parser-catalog-filegroup-select-r1
Status: accepted; M7A remains partial

## Outcome and compatibility

Advance the unchanged authentic Rust-toolchain `cquery //:probe --output=label`
past the missing `@@bazel_tools//tools/def_parser` package. Port the complete
pinned Linux installed package verbatim (BUILD and no_op.bat) through the
existing immutable catalog and admit its direct constraint selector through
configured native srcs-only filegroup. The Linux target/exec selection must
forward the exact no_op.bat source with no action. A synthetic matching
platform must choose a direct branch; the authentic Windows target branch
must not silently fall back to no_op.bat when its C++ source package is absent.

Installed Bazel 9.2 package bytes, release-archive file mode, configured native
filegroup direct constraint matching and already admitted source forwarding
are exact. Manifest route identity, structural configuration/DICE identity,
Rust valid-Unicode diagnostics and path tokens remain Slug-native. Windows
prebuilt/parser C++ closure, general filegroup attributes, mixed matching
constraint/config_setting specialization, other direct-key owner kinds and
full Rust-toolchain/C++ closure remain unsupported/deferred.
The user's current platform priority is Linux. Windows-specific functionality
may remain minimal stubs or the existing explicit refusal; do not add Windows
execution, binary/source closure or compatibility claims. The selected Windows
negative is only selector/fallback safety on Linux, not Windows testing.

## Source and ownership

Bazel 9.2 `tools/def_parser/BUILD.tools` is installed as package BUILD; the
source bootstrap BUILD is different and must not substitute. The pinned
installed package has exactly BUILD (971 bytes, SHA256
891d56515113353a675db4f62e51380f3816b703b18585361e02de84e16e95ca)
and no_op.bat (710 bytes, SHA256
4ebf44828201a3228a0521723c48d26db87565e44ef862fb65d5ead92840f64d).
Both release ZIP entries are mode 100644, so catalog executable=false;
installed extraction permissions are not authoritative. BUILD loads the
already cataloged tools/build_defs.bzl, selects @platforms//os:windows to
an unavailable prebuilt on a Windows host or third_party/def_parser on a
Linux host, and otherwise selects no_op.bat. No Java artifact enters Slug.

`FilegroupRule.java:27-80` inherits NativeBuildRule without disabling default
ENABLED toolchain resolution (`RuleClass.java:735`); its configured owner
therefore has a target platform. `ConfigConditions.java:59-88`,
`ConstraintValueInfo.java:65-96` and `ConstraintCollection.get` own direct
value eligibility and effective/default matching. `ConfigMatchingProvider.java:
199-229` gives direct values empty refinement criteria. WP853 at 6e0e2b55b
already implements and tests that shared condition semantics for aliases.

Reuse BuiltinBazelToolsSourceFileKey, catalog-derived directory listings,
manifest identity and existing canonical/apparent routes. Register the two
verbatim bytes with sorted entries and exact digest checks. The repository
BUILD.bazel sidecar and compile_data reference are build inputs only and must
not enter runtime catalog bytes. Catalog identity changes must update both
host variants in pinned tests without weakening manifest assertions.

Reuse the WP853 ConfiguredConditionKey eligibility Boolean, renaming its
private alias-only constructor to describe the admitted native selector
callers. Native alias and configured native filegroup alone supply true;
other callers remain false. Their condition truth is identical, so no
caller-kind identity or second key is required. The existing key remains the
sole retained DICE owner of truth and observed constraint/platform/package
dependencies, preserving requested selector labels. The filegroup's existing
source-selection, null source edges, provider publication and configuration
ownership remain unchanged. Nested condition aliases retain their independent
native alias eligibility. Direct peer equality/ambiguity and mixed-match
refusal are inherited unchanged from WP853. No cache, interner, Host read,
lock across DICE, platform store or new collection. The catalog is immutable
service-owned bytes; the Boolean is DICE-retained Allocative state; selector
declarations remain attempt scratch. The existing Arc/SmallMap/SmallSet and
DICE ownership contracts in docs/developers/dice.md and Stage 9 apply.

## Scope and validation

Allowlist: this manifest and canonical Current; Bzlmod builtin
bazel_tools/tools/def_parser/{BUILD,no_op.bat,BUILD.bazel},
app/slug_bzlmod_v2/{BUILD.bazel,src/builtin_repository.rs,
tests/builtin_bazel_tools.rs}; src/host_module.rs only for the existing
manifest identity test goldens; Loading's existing
external_subtree_package_set_tests.rs only for exact catalog inventories and
the inventory fixture's complete observed-input injection;
Analysis src/dice.rs (private constructor rename), src/dice/filegroup.rs
(eligibility), tests/native_filegroup/{mod.rs,authentic_builtin.rs} or one
new test-only Rust module registered there; compact Stage 5/6/9 notes after
acceptance. No Loading semantics, Configuration, Core, Build API, toolchain
execution, repository generator, C++ action or Windows binary import.

Preserve complete pinned catalog byte/hash/mode/manifest and exact recursive
package inventories. Extend the existing authentic built-in filegroup fixture
to exercise real catalog def_parser through root apparent reference and
canonical target/exec configurations: exact no_op.bat source, single source
edge, unchanged DefaultInfo/FilesToRun forwarding, zero actions. Test selected
Windows branch refusal and Linux restoration on the same DICE. Add focused
native filegroup direct-key target/exec matching, omitted setting default,
explicit override, source/default and platform A/B/A with alias-to-constraint
keys. Reuse WP853 discriminators for peer/mixed handling, ineligible owners,
Need/cycles and nested condition aliases, plus existing filegroup forwarding,
config_setting selector and source lifecycle controls. This extends existing
fixtures without new public oracle scaffolding.

Independent DESIGN review gates implementation; FINAL review gates acceptance.
Resolve pinned Rust toolchain, serialize Cargo, JSON --no-run compile selected
Bzlmod and Analysis executables plus the Loading inventory executable, batch
exact-name preflight, execute focused tests serially. Rebuild slug_cli_v2
before the unchanged authentic replay. Check stale slugd before/after; no
output_base is used, so the replay is one-shot. A network-enabled supervised
180-second replay is justified by WP853's approximately 88-second fresh source
preparation; smaller synthetic tests cannot prove installed toolchain
advancement. This is an operation bound, not a new user limit. Compilation
preparation remains bounded to 60 seconds per invocation. Record first new
labelled stop or success, leaving full toolchain closure unclaimed. Run
rustfmt, git diff --check, archive and plan-status checks. Commit accepted
work with compact compile/runtime/review/replay evidence.

REPLAN only if pinned package bytes require a new source owner, native
filegroup direct constraints lack the pinned target-platform prerequisite,
or the default source cannot use existing forwarding without a new action/
provider contract. Invocation/compiler/test corrections are not REPLAN.

## Immediate predecessor

WP853 at 6e0e2b55b accepted native alias direct constraints. Fourteen focused
tests and independent DESIGN/FINAL reviews passed. The unchanged authentic
Rust-toolchain replay cleared platforms//os:none and reached the missing
bazel_tools//tools/def_parser BUILD. M7A remains partial.

## Acceptance receipt

Independent DESIGN and FINAL reviews: ACCEPT. The two runtime files are
byte-identical to pinned source and the pinned release ZIP; mode 100644 is
verified from archive metadata, not installed extraction permissions. The
sidecar is excluded from the runtime catalog. Both manifest variants retain
exact framed-hash goldens, and package/directory inventories remain exact.
Native alias/filegroup condition truth shares the existing structurally gated
key; other owners remain ineligible and the existing mixed-match gate remains.
No new source, platform, action or DICE owner is introduced.

Twenty-nine selected tests pass: Bzlmod catalog 5/5 and owners 7/7,
Loading source/route/inventory 3/3, Analysis 14/14. JSON --no-run compiles and
batched exact-name preflights selected every executed test. Real canonical and
root-apparent built-in filegroup routes forward the exact no_op.bat source,
DefaultInfo/FilesToRun and zero actions in target/exec configurations. Synthetic
matching/default/override and configuration/platform/selector/default-source
A/B/A controls pass, along with inherited alias cache/Need/cycle/refinement and
filegroup forwarding/rejection/source-lifecycle tests. The selected Windows
negative proves no silent Linux fallback on this Linux host; it claims no
Windows execution or parser parity.

The existing Loading inventory fixture had stale expected whitelist entries
from WP845 and injected only the legacy epoch. Its exact inventories now include
both that existing package and def_parser, and same-epoch observation shards are
fully injected; the corrected three-test gate passes without missing-key task
panics. An initial preflight used file names instead of registered module paths;
listing and exact-selector correction established selection before execution.
An additional host_module.rs test golden was updated to the same derived
manifest hashes and its structural route test passes. No production Loading
semantics changed.

The current-source offline CLI build passes (25.12 seconds). The unchanged
`/tmp/slug-wp797-authentic-2YQmSt` one-shot
`cquery //:probe --output=label` exits 2 after 79.064 seconds,
inside its 180-second operation bound. BUILD/MODULE/lockfile hashes are unchanged.
It clears the missing bazel_tools def_parser package and first reports
`Object of type cpp has no attribute grte_top` at
`@@rules_cc+//cc/common:cc_helper.bzl:844`, called by cc_library's invalid-options
check. This is bootstrap advancement, not a successful full-toolchain query.
No stale slugd was present before/after, and the supervised replay stopped its
process group. Raw evidence remains under target/wp854/.

Recorded packet window from manifest draft to acceptance is approximately
20.3 minutes. Final Bzlmod compile 4.56 seconds (initial preparation 14.99),
Loading compile 7.85 seconds (initial 21.84), Analysis compile 27.86 seconds;
focused runtime totals approximately 0.45 seconds. Independent review elapsed
was not measured separately. Rustfmt, diff, archive and plan-status checks pass.
M7A and the next cpp.grte_top/full C++ closure remain open. The user's Linux
priority and minimal/deferred Windows implementation are recorded canonically.
