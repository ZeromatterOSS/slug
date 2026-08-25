# Current Slug V2 Packet

Packet: `WP-6-7A-generated-external-build-bazel-9-2-evidence-refresh`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Authority base: retained bridge candidate over accepted docs `0e28f29b`

Result: refresh the existing hermetic generated-repository exported-source
build fixture from historical Bazel 9.1.1 output to the canonical Bazel 9.2.0
baseline before the retained bridge candidate receives its proof/cap correction.

## Oracle-only contract

Change exactly:

- `tests/v2_oracle/fixtures/module-extension-use-repo/fixture.toml`; and
- `tests/v2_oracle/fixtures/module-extension-use-repo/expected/oracle.json`.

Keep all three workspace files byte-identical. Their entry SHA-256 values are:

- `BUILD.bazel`: `1f1994e887d6f6165509e3d4b4810fc46f90dd68613ce981baff8f8bbe004192`;
- `MODULE.bazel`: `e5d1e9283df75188619978cb077ac0111cb9ad4da4e3c7ea7f606faf31518b21`;
- `ext.bzl`: `395e3ca7accd421c949531a382c1ea8d01525ee36b4a36163cf03a8225854703`.

The fixture remains one self-contained public-surface command:
`build @generated//:generated.txt`. Its local module extension creates the
repository, writes a `BUILD.bazel` that exports `generated.txt`, and imports it
with `use_repo`. Require exit 0, the canonical
`@@+ext+generated//:generated.txt` analyzed/source-file message shape and
successful build completion. This discriminates apparent mapping, extension
repository generation, generated package loading and exported-source success.

Add the Stage 1 provenance contract with Bazel release `9.2.0`, immutable
source commit `8220c6198837d5c13d53fea211cf3282aa12408a` and these exact anchors:

- `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileGlobals.java#useRepo`;
- `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/SingleExtensionEvalFunction.java#compute`;
- `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleExtensionRepoMappingEntriesFunction.java#compute`;
- `src/main/java/com/google/devtools/build/lib/skyframe/PackageFunction.java#compute`;
- `src/main/java/com/google/devtools/build/lib/skyframe/TargetCompletor.java#createSucceeded`;
- `src/test/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleExtensionResolutionTest.java#generatedReposHaveCorrectMappings`; and
- `src/test/java/com/google/devtools/build/lib/analysis/BuildViewTest.java#testTopLevelInputFile`.

Require every path and named method to resolve at the pinned commit. Record that
the fixture composes the generated-repository mapping test with the top-level
exported-source build test into one smaller public CLI discriminator; it is not
a verbatim migration of either workspace. Add translation notes and generation/
verification commands. Generate and replay with `/usr/bin/bazel`, which must
report exactly `bazel 9.2.0` before either run. Preserve `message_shape`; do not
claim timing, process counts, output-base paths or server text.

The entry sizes are 11 manifest lines and 34 generated JSON lines. Physical
ceilings are 30 and 45 lines respectively, with at most 75 aggregate physical
lines. The workspace is frozen; add no fixture, payload, harness, Rust, Cargo,
BUILD, lockfile or unrelated oracle change.

## Compatibility and proof

The observed Bazel command exit, canonical generated target identity,
source-file classification and successful completion are exact Bazel 9.2
evidence. Path, duration, server/progress and process-count normalization remain
Slug-native harness presentation. Slug command activation, public/query
generated-repository publication, other platforms and exact configuration or
output identity bytes remain unsupported/deferred.

Validate serially:

1. record `bazel --version` as exactly `bazel 9.2.0` and the sibling source tag
   commit as exactly `8220c6198837d5c13d53fea211cf3282aa12408a`;
2. independently generate with
   `python3 -B -m tools.v2_oracle run --fixture module-extension-use-repo
   --tool bazel --bazel /usr/bin/bazel --update-expected`;
3. replay without `--update-expected` and require a clean comparison;
4. inspect the generated record for exit 0, canonical target/source-file shape,
   successful completion, Bazel 9.2 server provenance and normalized paths;
5. run the focused oracle-harness test, fixture listing, workspace hashes,
   exact allowlist/physical ceilings, credential/home-path scan and
   `git diff --check`.

STOP on any workspace drift, non-9.2 execution, stale 9.1.1 record, comparison
failure, missing generated canonical label, credential-derived material,
fixture/harness/Rust expansion or cap breach. After ACCEPT, retain the Rust
candidate non-writable and schedule only the docs-only
`WP-6-7A-generated-package-load-bridge-proof-cap-correction-design`. That design
alone may authorize measured cap changes, test-only materialization-helper
visibility, fixture consumption and command-glue algebra; do not accept or edit
the bridge from oracle evidence alone. M7 remains partial and M7A -> M8 -> M7B
remains.
