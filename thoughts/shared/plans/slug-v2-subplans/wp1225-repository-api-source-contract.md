# General repository execute/workspace API: pinned-source contract

Pinned Bazel 9.2 revision `8220c6198837d5c13d53fea211cf3282aa12408a` is the
reference. This source inventory establishes API semantics; the current packet
and [implementation design](wp1225-general-repository-workspace-design.md) own
selected behavior, classification and activation gates. Slug stays Rust-native.

## API behavior and current gaps

| Surface | Pinned Bazel contract | Required Rust owner change |
|---|---|---|
| `execute(arguments, timeout=600, environment={}, quiet=True, working_directory="")` | arguments is a positional sequence; validate each element before timeout/env processing. Non-remotable context accepts String, Path and Label in EVERY position. Remotable context's validation accepts only String/Label even if execution falls back local. Options annotated named=True also remain positional by Param's default positional=True. | Loading current Which-first/path-only argv and named-only options are narrower. Use typed String/actual Path/Label inputs with resolved provenance; retain exact arbitrary strings without source rewriting. Remote-capable repository declarations need explicit policy, not silent Path acceptance. |
| executable/Label conversion | Label resolves through the ordinary tracked Label path owner and may require Skyframe restart/materialization. String and Path use their actual text; no implicit `which()` requirement. Empty argv is refused before launch. | Preserve existing Need on source/materialization/path resolution; generalize first argument exactly like remaining arguments. Existing Which provenance can remain a typed route but cannot select executable semantics. The bounded argv/NUL/path capability policy remains explicit Slug-native. |
| timeout | Strict Starlark integer, checked Java signed-i32 conversion; bool/float/None/oversized integer do not become a timeout. Base validates argv, then converts timeout, then validates environment. Local milliseconds=round(timeout*1000L*timeoutScaling); builder requires positive timeout. | Bind checked integer and preserve order. Retain requested timeout in request equality. Initial scale1 must be explicit if scaling is not modeled. Keep existing30s operational ceiling as distinct Slug-native infrastructure/capability refusal; do not claim API timeout600 became30 or fabricate timeout256 at an earlier safety deadline. Default600 may still run a short command successfully. |
| environment | `environment=None` is invalid: argument must be Dict with string keys. Each value is String or None. Start with current repository environment, override String values, then remove None keys. Empty string value remains present. | Replace current SmallMap<String,String>/defaults-only refusal with checked override/removal enum. Full base environment plus overrides/removals participates in structural request equality and currentHost observation. No reserved compiler/Homebrew spelling dispatch. |
| working directory | Strict String; empty selects repository root, nonempty resolves via path() (relative to root or absolute); create both root and selected cwd directories before local execution. | Same reserved generation namespace as file/read/template/execute. Normalize/validate through actual owner; permitted absolute locations are a declared capability. Do not replace cwd by fixed `/exec` or rewrite embedded argv strings. |
| quiet | quiet=True suppresses terminal forwarding ONLY; both streams remain captured. Local quiet=False forwards BOTH stdout and stderr to current terminal stderr while also recording, before execute returns. Per-stream byte order is real; cross-stream interleaving follows actual scheduling. | Existing runner captures only. Need a bounded current-consumer output/event path; never inherit a request printer in an immutable record or write global stderr from DICE. On Need replay, selected invocation events must place this operation's output between surrounding real Starlark prints and deliver once. Buffering until operation completion is a separately classified timing divergence; streaming/timing parity cannot be inferred from captured bytes. |
| result | Immutable nominal `exec_result` with return_code/stdout/stderr. Normal and nonzero exits retain actual captured Latin-1 bytes. Bazel timeout yields256, stdout empty and diagnostic stderr; command-start CommandException also256/empty stdout/diagnostic stderr. Signal termination normally raw code>128; SIGTERM treated as interruption. | Current replay's Latin1 decoder already maps every raw byte to corresponding U+0000..00FF; keep it. Current AllocStruct exposes fields but is not a dedicated nominal exec_result type. Runner timeout/launch/signal behavior is NOT already Bazel parity: make exact subset/error divergence explicit; cancellation/cleanup never fabricated as successful result. |

Anchors: StarlarkBaseExternalContext.java1898-1911,1938-2075; Param.java78/94; StarlarkExecutionResult.java60-99,132-149,179-276. The exact Java exception message and process-wrapper diagnostics are not stable portable Rust promises; declared output and interruption classes need actual discriminators.

The inactive Linux runner's executable lookup is explicitly Slug-native:
absolute/relative-with-slash argv0 executes literally after selecting cwd; a bare
filename searches the complete effective payload PATH (empty segments mean cwd),
with absent PATH defaulting to `/bin:/usr/bin`. There is no implicit shell fallback
for ENOEXEC. Pinned `Command.java169-205` documents absolute, bare-name PATH and
cwd-relative forms, while `JavaSubprocessFactory.java161-178` delegates actual
lookup to ProcessBuilder. This source alone does not prove modified/removed PATH
or ENOEXEC corner parity with JDK startup behavior. Those corners retain the
explicit native classification; literal argument bytes and effective child
environment remain required. No JVM performs any Slug semantic work.

## Paths and sequential state

`path(x)` accepts String/Label/Path. A Path input returns that same Path. Relative String resolves under repository directory; Label goes through actual source resolution and can restart. Path string/repr denotes the actual absolute path. `get_child(*strings)` joins paths on ANY path, not just generated paths; zero arguments returns equivalent path. `dirname` and exists operate on the denoted path. exists does not implicitly watch it. read accepts String/Label/Path, defaults watch='auto', rejects directory, reads raw bytes as Latin1. Under working directory, auto does not watch (avoids dependency cycles), explicit watch='yes' is rejected. External reads need the normal observed/watch contract; `watch='no'` must not become a false complete freshness certificate.

Anchors: BaseExternalContext1544-1626,1633-1645,2354-2381; StarlarkPath52-74,127-169,198-210. Current Loading Generated paths stringify as `@@repo//relative`, only generated get_child is admitted, exists uses an effect-name set, and read accepts only external Label/ASCII. These cannot support actual process-written files by merely deleting guarded_argv. Preserve actual path normalization/capability distinctions: a platform-confined escape refusal is not an assertion Bazel syntactically rejects every `..`/absolute child corner.

The SAME reserved eventual root must back all APIs before string conversion and after final adoption. Use one predecessor-bound ordered workspace state, not separate execute/filesystem streams that prohibit mixing. A real child nonzero exit leaves its actual writes/deletes/modes visible to subsequent exists/read/template/execute; replay returns exact result AND advances to that complete accepted next state. Immutable prior versions remain retained; root handles/leases remain Core-owned. Completed equal requests replay only at exact currentlease+operationindex+predecessor; new operations run even equal argv; fresh command starts empty and samples Host again before materialization reuse. Revocation/foreign/unused-tail/controlflow changes fail closed.

File/template integration reuses existing operations with broader actual path ownership. BaseExternalContext1443-1497 permits String/Label/Path destinations, writes after output-directory check and replace, default executable=True, legacy_utf8 no-op. RepositoryContext259-352 accepts String/Label/Path for destination/template, uses ordered literal substitutions over Latin1 data, then replaces output after output-directory check; watch_template defaultsauto. Current Rust file uses raw UTF8 and read is ASCII-only; arbitrary Unicode/UTF16 template conversion is NOT exact by this source evidence. Preserve the existing admitted encoding slice, explicitly classify validUnicode-to-Latin1 policy Slug-native or declare unsupported characters until separately proven; no Homebrew/value selector.

## Event/lifecycle boundary to choose before implementation

Core execution occurs outside DICE after full Need request preparation. For exact quiet=False output, a required borrowed current command sink (binary bytes, stderr destination, fallible delivery/current cancellation) or an owned bounded operation-event batch must be part of the physical attempt, with selected replay/event reconciliation preventing duplicates. Store captured result bytes/operation event data, NEVER consumer closures/print handlers. Loading's existing InvocationPrintCapture supplies ordered Starlark print events but has no process-output event today. A deterministic stdout-then-stderr replay may preserve exact bytes/per-stream order but is not evidence of Bazel live forwarding or original interleaving; root must select/classify this real API decision.

No accepted next state before actual child/descendant quiescence and bounded complete-tree capture, including nonzero exits. Existing128calls/1MiB-per-stream/64MiB-private-workspace/30s-operation safety bounds remain. The Host-volatility/confinement and namespace decisions are those in current implementation design; no generic Host hermeticity or cross-command result caching is inferred.

## Smallest cohesive owners and disjoint authoring split

- Bzlmod protocol: repository_execute.rs (general arguments/options/result/currentepoch), repository_filesystem.rs and source_preparation.rs (one sequence/reservation/realization), a small new private workspace-state leaf if needed. Full structural equality and actual predecessor/nextstate, not recipe metadata.
- Loading original invocation: repository_rule_context.rs plus filesystem.rs/deletion.rs/archive.rs where needed; module_extension_repository_file_effect.rs joins actual reservation/currentepoch/Need/event replay and removes Homebrew selectors. Test new behavior in existing repository-rule fixtures and existing filesystem_replay_tests.rs; no copied ruleset corpus.
- Core physical owner (root can own): repository_process.rs; repository_filesystem_progress.rs; repository_io.rs and source_generations.rs; dice.rs orchestration. A small snapshot/reservation leaf may separate bounded algorithms. This is one capability even across these owners; Root alone composes overlapping dice/materializer edits.
- Independent regressions: existing Bzlmod repository_execute tests; existing Loading descendants; Core repository_filesystem_manifest_tests.rs/general_generated_effect_materialization_tests.rs plus existing process unit controls. Actual unrelated original must cover str/Path/Label argv, removals/emptyenv, cwd, quiet-before/after print, nonzero writes/delete/read/template, A/B/A, missing/foreign/incomplete/revoked records, exact/+1/cancel/kill-reap and old-root retention. GCC/Homebrew originals then exercise SAME general route; no source/caller/callsite/expected-effect profiles remain.

Source provenance is recorded in the [backing receipt](wp1225-repository-root-backing-2026-11-07.json).
Platform experiments do not establish Starlark API compatibility or public
bootstrap and workload-memory acceptance.

## General archive checksum omission

Pinned `StarlarkBaseExternalContext.java494–562,900–1038,1041–1177` accepts empty
sha256/integrity, computes actual compressed-file SHA256, and returns success,
sha256 and SRI integrity. Both nonempty options conflict; malformed declarations
remain errors. `getUrls443–480` removes plain HTTP when no checksum is given.
`DownloadManager.java240–315` gates destination/cache/distdir lookup on a provided
checksum;359–367 may publish a newly acquired file under its actual digest.
Canonical ID restricts checksum-keyed hits, never identifies bytes by itself.

Use the original tests `starlark_repository_test.sh::test_starlark_repository_context_downloads_return_struct`
and `bazel_repository_cache_test.sh::test_write_cache_without_hash` as pinned
regressions. Existing Slug declared-checksum parsing is correct but intentionally
refuses omission. The [selected design](wp1225-general-repository-workspace-design.md#optional-expected-checksum-for-general-archive-acquisition)
adds it only under command-fresh General ownership, with actual digest/FD/history
proof and strict legacy/natural Source integrity retained. Existing format/output/
auth/headers/allow_fail/rename/strip-components boundaries remain deferred.
