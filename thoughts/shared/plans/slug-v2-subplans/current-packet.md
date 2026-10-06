# Slug V2 current work packet

Packet: WP-7-221-m7a-build-info-recipe-render-r1
Status: Public compile producer/query accepted; finite BuildInfo recipe renderer selected

## Observable result and source basis

Implement pure rendering on the existing authenticated BuildInfoCallbackRecipe:
a borrowed supplied status string map and template produce one independent header
String, or the exact missing BUILD_TIMESTAMP diagnostic. Saved WP905 has four
TranslateBuildInfo rows. WP778 checked registration and WP917 class query are accepted;
finite callback/template rendering remains a concrete runtime prerequisite.
Reuse pinned Bazel9.2 commit8220c6198837d5c13d53fea211cf3282aa12408a callback, templates,
BuildInfoFileWriteAction and LocalTemplateExpansionStrategy anchors from
[WP921 discovery](wp921-public-compile-query-2026-10-06.json), with the complete branch,
order and authentication report at target/wp921/successor-source.md.

Exact finite callback and literal replacement algorithms apply to supplied strings;
the valid-Unicode String domain remains Slug-native. Latin1 artifact conversion,
raw status parsing/production, tracked template reads, volatile execution/cache policy,
general callback evaluation, REAPI/execution, Windows breadth and authentic full-root/
M7A/whole-workload memory fit remain deferred. Rendering does not waive these prerequisites.

## Ownership, allocation and scope

The checked recipe owns kind/canonical source/digest. Loading already verifies callback
shape, full recorded/evaluated source identity and exact callsites in the capture-free
builtin module. Implement only its finite Info/Version branches, without name-based
fallback or a general evaluator. Existing transform identity/metadata, query and Core
execution guards stay unchanged. No DICE keys, ownership or locks change.

Status/template inputs are borrowed synchronously; output owns all bytes after their
drop. Use fixed ordered borrowed key/value pairs, without an owned substitution map,
retained cache/interner/key/graph or global pool. Review buffer growth and copy costs in
the concrete contract. Reuse a literal utility only if it preserves nonrecursive
same-pass insertion and later-pass substitution. Existing scoped producer interners
remain appropriate for retained repeated immutable values; unique status/template
scratch must not be globally interned. Prior allocation refusal and explicit caps persist.

Provisional allowlist: BuildAPI src/actions/build_info_transform.rs and its tests.rs;
Analysis tests/rustc_map_each/compile_context/provider/build_info_transform.rs for a few
assertions through actual installed checked recipes and unchanged builtin ASCII templates.
Freeze the exact API/map/error/utility boundary with three DESIGN reviews before editing.
Production estimate<100 lines; new tests<200 lines, with responsibility review for growth.
No builtin/corpus/Loading/Core/query/CLI edits. Root owns contract/integration/compile/
commit; independent source/ownership/evidence DESIGN and FINAL reviews. Operations live
in target/wp922. No network, external oracle, source16/17/18 retry or action/tool execution.

## Discriminators and validation

Info substitutes EMBED_LABEL (empty default), HOST (hostname) and USER (username), in
that order. Version first requires TIMESTAMP presence, then substitutes REVISION (0),
STATUS (empty) and TIMESTAMP. Empty values stay empty, unknown keys are ignored, and no
trimming, C escaping or numeric validation occurs. The timestamp diagnostic also applies
to an empty template or one without its placeholder. Replace all literal occurrences;
an inserted same key does not recurse in its pass, later passes see introduced keys,
and unselected placeholders remain. Compare independent full headers including final LF,
unchanged borrowed inputs, repeat equality and output after input/recipe release.
Preserve source/digest/wrong-role/output-metadata negatives. Supplied string-map tests do
not establish LF/CRLF/duplicate status-file parsing or original Latin1 artifact-byte parity.

Use normal installed/main/external producers with real checked recipes; no synthetic
status/materialization/certificate. Preserve stamped non_redacted action ordering and
separate redacted symlink group regardless --stamp, plus existing callback refusals.
Resolve pinned nightly tools; separate offlineJSON no-run preparation <=60s TERM3 for
BuildAPI and Analysis direct dependent. Compile the default CLI when production changes.
Fresh exact nonignored preflight precedes serial10s TERM3 runtime. Run protected BuildAPI
registration controls and new renderer cases, plus the two Analysis selectors:
installed_build_info_and_unchanged_main_external_instances_register_real_actions and
installed_build_info_restores_full_configuration_and_rejects_java_callbacks.
No concurrent Cargo/shared target or ancestor-sensitive native fixtures. Reuse nonstale
passes and correct only missing gates; format/diff/archive/status/compact receipt/root
commit. REPLAN only for an observed evaluator/source/retained-owner/runtime prerequisite.

## Immediate predecessor

WP921 proves public cc_common.compile to typed query with fourteen recipes across main/
external Target/selected Exec. Optimized Exec PIC owners retain ordered PIC then NOPIC;
the separate Analysis direct to_exec twelve-case proof retains its fastbuild provenance.
Complete ordered class blocks, fixed PATH/env/recipe-last info, borrowed roles/context
roots, source/configuration A/B/A and released owned text pass. Seventy-one distinct Core
checks are proved by qualified69+1+1 union, plus three Analysis gates. Production and
retained storage remain unchanged; authentic source/runtime/full-root/M7A/whole fit stay open.
