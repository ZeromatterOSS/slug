# Current Slug V2 Work Packet

Packet: WP-7-137-m7a-generated-repository-symlink-r1
Status: ACCEPTED; independent DESIGN and FINAL reviews accepted; M7A remains partial

## Outcome and compatibility

Admit the two authentic generated `rules_cc` 0.2.17 Linux
`repository_ctx.symlink` calls: a source path returned by
`repository_ctx.path(Label(...))` for an existing external repository file,
and a nonempty safe repository-relative string destination. Produce a real
POSIX symlink in the generated repository, in invocation order, without
overwriting an existing destination. The Starlark call shape and selected
link/file loading behavior are **exact** for this subset under Bazel 9.2.
Slug's generated-root location, materialization/source-association bytes,
safe-path rejection diagnostics and Rust Host observations are **Slug-native**.
Direct String/Label or host-path sources, root Label sources, dangling sources,
directory links, generated-source links, Windows copy/junction behavior,
symlink/archive mixtures, arbitrary destination path objects, complete C++
toolchain setup and root Build are **unsupported/deferred**. Existing exact
slices remain exact; no Java runtime/helper enters Slug.

## Pinned source and authentic demand

Pinned Bazel 9.2 `StarlarkRepositoryContext.java:197-254` resolves
`symlink(target, link_name)`, checks the destination under the repository,
creates parents and calls native `createSymbolicLink` without pre-deleting
the destination. `StarlarkBaseExternalContext.java:313-322` owns the lexical
destination check; `RepositoryUtils.java:54-76` checks a Label package but
not the named source file. Unix `UnixFileSystem.java:299-307` uses POSIX
symlink; Windows behavior is different and deferred. Positive upstream
evidence is `StarlarkRepositoryContextTest.java:566-575`,
`StarlarkRepositoryIntegrationTest.java:91-123` and
`starlark_repository_test.sh:151-173`. The cached `rules_cc` 0.2.17
`cc/private/toolchain/unix_cc_configure.bzl:348-370` resolves two `@rules_cc`
Labels to path objects, then links them as `cc_toolchain_config.bzl` and
`armeabi_cc_toolchain_config.bzl`. An isolated ignored-RC Bazel 9.2 probe at
`/tmp/slug-wp837-symlink-O6wkPT` queried `@linked//:through_link` with exit
zero and observed a native BUILD symlink; that probe is diagnostic until a
checked-in offline fixture is generated and independently verified.

## Semantic owner and decision

The existing selected repository-rule effect DICE key and replay loop own
the call, prepared Label path and observed external source bytes. The
`repository_ctx.symlink` method accepts only a prepared external Label-derived
`RepositoryStarlarkPath` and relative string destination, requests
`RepositorySourceNeed` as `read`/`template` do, and records no partial plan on
Need/error/cancellation. Both source path/route and exact source bytes must
participate through existing observed dependencies; A→B→A source-byte edits
and source rematerialization must restore the result. Do not infer bytes from
a physical path or read the filesystem outside its DICE owner.

Extend the existing `GeneratedRepositoryFileEffectPlan` with one ordered
Symlink variant; keep `File` and `Archive` unchanged. Retain destination as
`CompactString`, source as the existing immutable Label-path value, and one
shared `Arc<[u8]>` observed source value for structural equality and exact
content identity. Reuse the existing `SmallSet` destination de-duplication,
`Allocative` and immutable plan Arc; add no interner, global cache, DICE key,
evaluator-heap borrow or Buck2/V1 donor import. The plan's retained memory
releases with its DICE value/request. A symlink-containing plan uses a new
domain-separated ordered Slug source association including kind, destination,
source path/namespace and exact bytes; never reuse the file-only association
without those inputs. Keep symlink/archive mixtures fail-closed.

Core's existing provisional generated-repository writer creates parent
directories and the actual Unix symlink inside its temporary root, rejects
duplicate/ancestor destination conflicts and symlinked destination ancestors,
and publishes only after all ordered effects succeed and session validation
accepts. No link may escape via a destination ancestor. Before exact
acceptance, prove with a lifecycle test that the external immutable source
root remains retained while an accepted generated link can be read across
warm/reselected same-daemon requests, and that abort/discard/shutdown release
both roots. If existing retention does not prove this, add the smallest
transfer-owned Core lease inside this packet only after independent design
review; do not put a tempdir handle in DICE or silently copy the source.
The existing large repository context and Core writer files remain cohesive
owners because this change extends their current method/effect application
tables, not a new repository execution path.

## Evidence and validation

Add one minimal offline Stage 1 fixture `repository-rule-symlink` using an
external local module source file, `path(Label)` and generated BUILD symlink.
Its query row must distinguish successful generated BUILD loading; Rust
materialization tests must separately assert native symlink kind/target.
Generate with fresh ignored-RC Bazel 9.2, verify in an independent fresh
root, and replay with rebuilt Slug. The fixture records pinned provenance,
exact anchored stdout, no credential-bearing RC or copied upstream archive.
It is expected to add about 8 files/1 row, below the Stage 1 growth trigger.
Protect existing generated-source, file-effect and Label-path fixtures.

Focused Bzlmod builder tests cover order, structural equality, cross-kind
duplicate/ancestor conflicts, path traversal and source-byte distinctions.
Focused Loading tests cover typed ABI, path-then-source Need, selected call
atomicity, external route/source edits, A→B→A and rejection of unsupported
forms. Core tests prove real symlink, link target, order, nested destination,
conflict, failed/aborted provisional cleanup, source-root retention and
reselection. Resolve pinned nightly via `rustup which --toolchain`; serialize
Cargo, compile with JSON `--no-run`, preflight all exact selectors with
`scripts/v2_test_preflight.py`, run focused/protected tests and named Core/CLI
dependents. Format Rust; run diff, plan, archive and fixture-hygiene checks.
Rebuild `slug_cli_v2`, clean `slugd` before/after one authentic `//:probe`
smoke, and record its next first error without claiming full closure.
Independent DESIGN and FINAL reviews are required for the new retained effect
and Core lifecycle boundary.

## Scope and stops

Base clean main `b29d00c76`. Allowlist: this manifest and canonical Current;
`app/slug_loading_v2/src/repository_rule_context.rs`, the exhaustive plan-error
renderer in `app/slug_loading_v2/src/registration_diagnostic.rs`, and existing
focused tests in `app/slug_loading_v2/src/module_extension_repository_file_effect.rs`;
`app/slug_bzlmod_v2/src/generated_repository_file_effect.rs` and `lib.rs`;
`app/slug_core_v2/src/runtime/repository_io.rs`;
`tests/v2_oracle/fixtures/repository-rule-symlink/**`; compact Stage 4/5 and
Stage 9 owner/disposition notes after acceptance. Review triggers are roughly
150-300 production and 200-400 proof lines, not caps. No generic path API,
archive extractor, repository route/key, action symlink, remote transport,
configuration identity or Windows implementation change. `REPLAN` if source
bytes cannot be observed through the existing selected owner, if a source
root can disappear while an accepted generated link remains live without a
bounded Core lease, if exact link behavior requires unmodeled external path
forms, or if the only viable correction is a copied file masquerading as a
symlink. A smaller source copy may be proposed only as explicitly Slug-native
in a new decision, never as this packet's exact completion.

## Immediate predecessor

WP836 at `b29d00c76` admitted configured `config_setting` aliases for
toolchain settings and moved the authentic first stop to generated `rules_cc`
`repository_ctx.symlink`. M7A remains partial.

## Acceptance evidence (2026-09-24)

The existing selected repository-rule owner now replays an external
Label-derived path and DICE-observed bytes before publishing a distinct
ordered symlink effect. Core writes a real Unix link, checks destination
ancestors and conflicts, and keeps accepted source roots alive through warm
reselection; discard and shutdown release provisional/retained roots. The
source Label-path key computes the repository materialization result key,
whose request is selected from the DICE activation closure. The new observed
test proves source bytes A→B→A and a changed materialization root/instance;
the Core lifecycle test proves acceptance, reselection, discard and teardown.
No new DICE key, retained cache or lease was needed.

The one-row offline `repository-rule-symlink` fixture passed fresh ignored-RC
Bazel 9.2 generation and independent fresh verification, then rebuilt Slug
replay (exit zero, exact `@linked//:through_link`). Its canonical `@@source+`
Label avoids a separate root `.bzl` apparent-repository visibility boundary;
the selected external path-object-to-link call shape is unchanged. Two new
Bzlmod builder tests, three new Loading tests, two new Core tests, all 24
repository-context tests, nine retained-plan tests, six generated Core tests,
and four protected selected-effect tests pass. Pinned JSON no-run compiles,
`slug_cli_v2` rebuild, rustfmt and independent FINAL review pass. The
authentic Rust-toolchain `//:probe` cquery now first stops at selected
`rules_shell++sh_configure+local_config_shell` `repository_ctx.which` limits
(exit 2); this is progression evidence, not full C++/root Build closure.
The final scope remains the exact Unix existing external Label-derived file
slice stated above; omitted path forms, dangling/directory/Windows links,
symlink/archive mixtures, exact Bazel identity bytes and M7A completion stay
deferred.
