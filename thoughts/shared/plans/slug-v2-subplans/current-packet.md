# Current Slug V2 Work Packet

Packet: WP-7-10-m7a-spawn-paramfile-source-audit-r1
Status: result ACCEPT; callback admission is the first known CLI Spawn blocker

## Outcome and classification

Identify the first implementation boundary on the CLI root's `rules_rust`
Spawn parameter-file path without running Bazel, Cargo, a build or a test.
This packet admits no new behavior. Pinned Bazel 9.2 source establishes the
exact virtual-input mechanism; Slug's eventual structural identity and REAPI
Action digest remain separate Slug-native domains until implementation proof.

The generated-input aquery at `e4c7435e2` proved two first-party build-script
tree edges, not execution. Independent design review rejected an explicit
`ArgsWriteSpec` executor proposal because that family is not demonstrated in
the finite CLI closure. It also rejected an `--include_param_files` aquery
proposal before invocation: the demanded Spawn files are virtual inputs, not
artifact-backed `ParameterFileWriteAction`s. Neither query ran or advanced a
readiness row.

## Pinned-source result

In pinned `rules_rust`, `rust/private/rustc.bzl:1167-1168` sets multiline
format and `@%s`, conditionally always writing a param file;
`cargo/private/cargo_build_script.bzl:354` sets
`--cargo_manifest_args=@%s` and `use_always=True`. These policies belong to
Spawn Args snapshots, not standalone ArgsWrite actions.

[Bazel 9.2 `CommandLines.java`](https://raw.githubusercontent.com/bazelbuild/bazel/9.2.0/src/main/java/com/google/devtools/build/lib/actions/CommandLines.java)
`expand:88-157` receives the primary output as the param-file base, numbers
materialized files from zero, makes replacement argv from each policy's flag
format, and returns `ParamFileActionInput` virtual files. A non-always policy
can keep arguments inline when the conservative command-length check passes;
its result depends on command-line limits and previous argument lengths.
`CommandLines.java:200-239` writes a virtual file through
`ParameterFile.writeParameterFile` and exposes its execution path.
[Bazel 9.2 `SpawnAction.java`](https://raw.githubusercontent.com/bazelbuild/bazel/9.2.0/src/main/java/com/google/devtools/build/lib/analysis/actions/SpawnAction.java)
`getSpawn:358-379` passes expanded argv and virtual files to `ActionSpawn`,
using the primary output exec path as the base. Its `getArguments:206-209`
instead calls `allArguments`, which does not make param-file replacement argv.
Pinned `ActionGraphDump.java:143-160,225-236` populates aquery `paramFiles`
only from artifact-backed `ParameterFileWriteAction` content and Artifact
inputs; the virtual files are not observable through that field. Exact
virtual-file byte serialization, path mapping, size fallback and CAS staging
still require an implementation contract and discriminating tests.

Slug's `RetainedSpawnArgsSnapshot` preserves recipe and policy, but
`SpawnSpec::render_argv` flattens the recipe; typed Spawn REAPI lowering
still fails closed. Earlier on the demanded source path, `rustc.bzl:1169`
calls `rustc_flags.add_all(..., map_each = _get_crate_root_path)`, and
`cargo_build_script.bzl:357` calls
`args.add_all(..., map_each = _runfiles_map, allow_closure = True)`.
`app/slug_loading_v2/src/subrule_invocation.rs:287-302,335-347,446-453`
rejects callback options during Args evaluation. Thus callback
retention/evaluation is the first known Slug blocker for those paths;
solving virtual parameter-file staging alone cannot produce their configured
Spawn actions.

## Next implementation boundary and checks

The next packet should admit only demanded `map_each` callback cases with
source-backed callable lifetime, dependency/equality and expansion semantics,
then preserve each Args snapshot's policy through Spawn projection. Keep
configured identity distinct from the final REAPI digest: effective rendered
argv, virtual-file bytes, outputs, environment and selected effective
properties determine that digest, not recipe object identity or platform
label alone. Path derivation, forced/length-based selection, format bytes and
virtual CAS ownership remain later proof obligations. The full CLI graph,
execution, compilation and M7A stay open; these need not be the only blockers.

Allowlist: this manifest, canonical Live Status, Stage 7 and bootstrap
readiness. Source/structure review, `python3 scripts/v2_plan_status.py` and
`git diff --check` passed without a test or long command. Independent final
review `ACCEPT` confirmed the pinned source anchors, Slug callback guard,
no-execution claims and successor order. No compile, test, build or Bazel
action query ran; review elapsed time was not measured. This read-only packet
records no runtime result.
