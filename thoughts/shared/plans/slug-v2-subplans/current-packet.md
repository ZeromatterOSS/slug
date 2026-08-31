# Current Slug V2 Packet

Packet: `WP-6-7A-typed-files-to-run-provider-core-implementation-r1`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 standard providers.

Status: Terminal `ACCEPT` under the category architecture accepted in commit
`8911a99f2`. Base `8911a99f2`. The first terminal review accepted the
implementation except that the public support schema omitted its frozen
mandatory `RetainedRunfiles` field. The bounded correction added the exact
retained runfiles/support shape without opening behavior; terminal rereview
returned `ACCEPT`.
The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked;
do not edit or stage it.

## Result and boundary

Replace the current string-backed `FilesToRunProvider` and `DefaultInfo`
executable fields with typed `AnalysisArtifact` identity and a stable typed
files-to-run depset. Materialize one dedicated Starlark FilesToRun value with
File-or-`None` public fields, and carry the complete typed provider through
root executable-attribute and subrule hidden-dependency provenance.

This is only successor 1 of the accepted category. Executable providers are
marked incomplete because Slug does not yet own their Bazel-required runfiles
tree. Direct FilesToRun action use and associated File action use continue to
fail before publication. `ctx.runfiles`, the other DefaultInfo constructor
parameters, support artifacts/actions, Spawn expansion, execution, and C++
families remain deferred.

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` and the source/test
hashes frozen in commit `8911a99f2` are sole semantic authority. Authenticated
rules_cc 0.2.17 `collect.bzl` and `tool.bzl` are generic consumers only. Zabel
`0795445f...` supplies peer guidance for typed sparse occurrences and late
manifest derivation; copy no code, representation, behavior, or claim.

## Compatibility

**Exact:** typed executable identity; public FilesToRun field names/types;
stable typed files-to-run topology for the admitted no-support slice;
explicit DefaultInfo files versus executable fallback; source/generated File
target singleton providers; root/subrule scope separation; provider
materialization/lowering; equality, A/B/A invalidation, and fail-before-
publication behavior.

**Slug-native:** generated path spelling, compact Rust layout, structural DICE
identity, and the internal incomplete-provider migration flag.

**Unsupported/deferred:** every nonempty runfiles-support tree or manifest;
FilesToRun/associated-File action expansion; `ctx.runfiles`; constructor
`runfiles`, `data_runfiles`, and `default_runfiles`; execution/REAPI/aquery;
aspects; collection flags; symlink entries; and all rule-family special cases.

## Frozen implementation

`FilesToRunProvider` owns:

```text
files: AnalysisDepset<File>
executable: Option<AnalysisArtifact>
support: Option<Arc<RunfilesSupport>>
complete: bool
```

Reserve the final typed `RetainedRunfiles`, `RunfilesSymlink`, conflict-policy,
and support shapes from the accepted architecture, including support-owned
runfiles plus tree/manifest/repository-mapping Artifact fields; construct none
and expose no runfiles behavior in this packet. Empty and nonexecutable
file-only providers are complete. Every executable `DefaultInfo` is incomplete
until the support-action successor. The files-to-run depset contains effective
files plus executable without flattening the explicit files topology.
`DefaultInfo.executable` is the same typed Artifact as
`files_to_run.executable`.

The dedicated Starlark value owns one retained provider occurrence and frozen
public File/`None` fields. It has provider identity
`FilesToRunProvider`, cannot be constructed, and is recognized directly by
analysis lowering and action binding. Its private files/complete state cannot
be accessed from Starlark. Generic `BuiltinProviderView` no longer represents
FilesToRun.

Change executable provenance from scoped Artifact sets to scoped
Artifact-to-`FilesToRunProvider` maps. Existing Spawn rejection consults those
maps but does not expand them. Subrule configured executable values materialize
as the same dedicated Starlark value. No global association, path lookup,
second provider owner, parser change, or DICE key.

## Allowlist, caps, and proof

Production:

- `app/slug_build_api_v2/src/providers/mod.rs` and `src/lib.rs`;
- `app/slug_analysis_v2/src/{analysis_value.rs,starlark_rule.rs,dice.rs,subrule.rs}`;
- `app/slug_loading_v2/src/subrule_invocation.rs` only if the action-sink ABI
  needs a typed predicate; and
- `app/slug_core_v2/src/runtime/dice.rs` only for the existing bounded run-view
  typed adapter.

Proof:

- `app/slug_build_api_v2/tests/{providers.rs,analysis_value.rs}`;
- `app/slug_analysis_v2/src/analysis_value.rs` colocated materialization proof;
- `app/slug_analysis_v2/tests/{starlark_rule.rs,subrule.rs}`; and
- exact mechanical constructor/assertion adapters in existing direct dependent
  tests when typed compilation requires them.

Plans may update this manifest, canonical, Stage 6, and Stage 9. Add no crate,
production file, action kind, runfiles Starlark method, or dependency. Cap
added Rust at 520 production, 520 proof, and 1,040 total lines. No touched
production file may newly cross 2,000 lines and no changed function may cross
150 lines. `REPLAN` before cap excess, support/action implementation, string
compatibility fields, flattened topology, global provenance, evaluator value
retention, or scope widening.

Focused proof must show:

1. empty, file-only, implicit-executable, and explicit-files executable
   providers have correct typed topology and completeness;
2. executable and files topology affect provider/publication equality;
3. DefaultInfo materialization exposes a dedicated FilesToRun value whose
   public executable is a File and manifests are `None`;
4. provider round-trip lowering preserves typed private state and rejects a
   fabricated generic provider in the executable slot;
5. root association and subrule hidden dependency carry the typed provider
   without crossing scopes;
6. direct provider and associated File action attempts still fail atomically as
   incomplete, while unauthenticated bare Files retain accepted behavior; and
7. existing run semantic view and source/generated DefaultInfo behavior remain
   valid without path-string identity.

Run serial focused and full `slug_build_api_v2`, `slug_analysis_v2`, and
`slug_loading_v2` suites plus `cargo check -p slug_core_v2`. Finish with fmt,
metadata, archive status, diff check, caps, physical sizes, independent terminal
review, and parked-file SHA-256 verification.

## Implementation evidence

The typed provider core, final retained runfiles/support data shape, dedicated
Starlark value, scoped provider provenance, and bounded run-view adapter are
implemented. During the full analysis suite,
the existing publication-cutoff discriminator exposed that direct derived
equality on the new private files-to-run depset would compare occurrence
identity. Files-to-run now participates in the existing publication-equality
state instead, preserving both structural equality and the alias partition
between `DefaultInfo.files` and `files_to_run.files`; the focused cutoff and
full analysis suite pass.

Serial validation:

- full `slug_build_api_v2`: pass;
- full `slug_analysis_v2`: pass;
- full `slug_loading_v2`: all packet-related tests pass; the suite-level
  `glob_invalidation` binary remains order-sensitive and failed on two
  different unrelated cases across two runs, while each failing discriminator
  passes alone;
- `cargo check -p slug_core_v2`: pass with baseline warnings;
- `cargo fmt --all -- --check`, metadata, and `git diff --check`: pass;
- archive status reports only the three pre-existing non-V2 thoughts-path
  allowlist failures; and
- parked proof SHA-256 remains
  `36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`.

Measured against `8911a99f2`, excluding the parked proof: production
`+387/-142`, proof `+113/-13`, total additions `500`. No touched production file
newly crosses 2,000 physical lines: the files already above that threshold
shrink or grow only six lines, while the remaining touched owners stay below
it.
