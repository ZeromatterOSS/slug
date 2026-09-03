# Current Slug V2 Packet

Packet: WP-4-5-7A-repository-context-template-implementation-r1

Milestone: M7A bootstrap-critical loading/repository execution closure. Admit
the bounded Bazel 9.2 `repository_ctx.template` shape reached by the authentic
rules_cc replay, using existing routed source-byte owners and generated-file
effect identity.

Status: ready for bounded implementation; independent terminal review is
required before acceptance.

Immediate predecessor `WP-5-7A-repository-context-template-audit` returns
`ACCEPT`. Pinned Bazel 9.2 source proves a reusable composition: template input
bytes already belong to the routed repository source-read keys, while output
bytes/mode already belong to the generated repository file-effect plan. No new
DICE key, source/materialization owner or effect representation is required.

## Frozen compatibility boundary

Implement as **exact** within the following admitted slice:

1. `repository_ctx.template(path, template, substitutions, executable)` accepts
   a normalized repository-relative string `path`, a `path` value previously
   returned by this invocation's admitted `repository_ctx.path(Label)` for a
   non-root canonical repository, a dictionary of at most 64 insertion-ordered
   string-to-string substitutions, and a Boolean `executable`. Omitted
   `substitutions` is empty and omitted `executable` is `True`.
2. `watch_template` must be omitted, selecting Bazel's `"auto"` behavior. The
   external template source is observed through the existing routed source
   owner; edits, deletion, kind changes and symlink changes invalidate normally.
3. Template bytes are limited to 2 MiB and final output to 8 MiB. Every
   substitution key and value must contain only U+0000 through U+00FF and their
   combined encoded bytes are limited to 64 KiB. Conversion to bytes is the
   one-byte ISO-8859-1 projection used by Bazel.
4. Apply every dictionary entry sequentially in insertion order. Each entry
   replaces all non-overlapping literal byte occurrences in the result of the
   prior entry; an empty key is a no-op. Unmatched bytes are preserved exactly.
5. Append the resulting raw bytes, normalized relative destination and mode to
   the existing invocation-local `GeneratedRepositoryFileEffectPlanBuilder`.
   Existing first-duplicate and invalid-path rejection remains unchanged. The
   method returns Starlark `None`.
6. A missing, non-file or unreadable source, invalid substitution, size-limit
   failure, route/source error or output-plan error is terminal and publishes no
   generated effect. Speculative path/template-demand attempts publish no
   prints, effects or dynamic-environment observations; a terminal evaluation
   failure retains the existing terminal-attempt print behavior.

Keep **Slug-native** physical temporary/materialization path bytes, native
Unicode storage and diagnostics, error text, retry count/sentinel transport,
bounded size failures, DICE cutoff mechanics and event carrier representation.
The admitted Latin-1 projection is exact only for strings whose scalar values
fit one byte; no Java UTF-16 behavior is claimed outside that subset.

Keep **unsupported/deferred**:

- root-repository, built-in-catalog and current generated-repository working-
  directory template sources;
- string or Label template arguments and string/Label/path output variants
  beyond the normalized relative string destination;
- explicit `watch_template="yes"`, `"no"` or `"auto"`, templates exceeding
  the admitted bounds, more than 64 substitutions, and non-Latin-1 keys/values;
- duplicate/overwriting output paths, normalization of `.`/`..`, absolute
  output paths and exact Bazel error strings;
- `symlink`, `read`, `watch`, path filesystem methods, execute/which,
  download/extract/patch and every other repository effect;
- module-extension path/template values, native repository rules, remote
  repository execution, lockfile mutation, configured analysis/actions and
  exact generated-repository layout; and
- any rules_cc, rules_rust, toolchain, repository-name, platform or host special
  case.

The authentic rules_cc 0.2.17/0.2.18 consumer calls
`template("BUILD", paths[build_path], {"%{name}": cpu})` with default mode and
watching. It is a discriminator, never an activation branch.

## Existing owners and implementation shape

`RepositoryRuleInvocationState` remains the sole synchronous evaluator/effect
owner. Extend its invocation-only prepared state with a bounded
`SmallMap<RepositoryLabelPathAddress, Arc<[u8]>>` for template bytes.
`RepositoryStarlarkPath` additionally retains the canonical Label-path address
used to create it. Its Starlark equality/hash/`str`/`repr` remain based only on
normalized physical path bytes; the address is routing provenance, not visible
path identity.

Add typed template argument, source-need and substitution/limit failures to the
existing invocation error algebra. The `template` method:

1. validates receiver, normalized destination, repository `path` source,
   dictionary strings, Latin-1 and size/count bounds;
2. returns a typed source need when the address is not prepared;
3. on a hit performs ordered literal byte replacement without decoding the
   template and pushes one ordinary generated-file effect; and
4. returns `None`.

The outer effect retry recognizes only the typed source need after the evaluator
has been dropped. Reuse the same canonical route computation used by Label-path
resolution, derive the repository-relative template source directly from the
address's typed package and target values, and call:

- `HostRepositorySourceRoute::source_read_key` in legacy mode; or
- `HostRepositorySourceRoute::source_read_observation_key` in observed mode.

Project their existing root-request/observation result variants exactly as the
external BZL source loader does. Merge the observed source epoch before retrying.
Reject a root address before route/read ownership and preserve built-in
fail-closed behavior. Do not read by physical path, call `std::fs`, introduce a
new byte key, reconstruct a Label from display text or route through
`HostRepositoryPathKey`.

The template byte map, evaluator, heap, builder, captures, substitutions and
replacement buffers are invocation scratch and never enter effect identity.
The routed source key owns source identity/invalidation; the finished plan owns
destination, transformed bytes, executable mode and order structurally.

## Pinned evidence and proof

Pinned authority is Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`:

- `StarlarkRepositoryContext.createFileFromTemplate` lines 245-346 freezes the
  signature, directory check, watch call, sequential literal substitution,
  ISO-8859-1 byte projection, replacement write and executable behavior;
- `StarlarkBaseExternalContext.getPath` and `maybeWatch` freeze input routing
  and default auto-watch behavior; and
- `StringUtilities.replaceAllLiteral` lines 54-82 freezes non-overlapping
  literal replacement and empty-key no-op behavior.

There is no focused upstream template unit. Add pinned-source regression proof
that discriminates:

- defaults and explicit `executable=False`, Starlark `None`, raw byte
  preservation and Latin-1 key/value projection;
- insertion-order cascading with template `a %{x} %{xy}` and substitutions
  `%{x} -> %{xy}`, `%{xy} -> Z`, producing `a Z Z`;
- empty/unmatched keys, invalid dictionary element types, non-Latin-1 input,
  substitution count/bytes and template/output bounds;
- invalid/duplicate destinations and missing/wrong-kind/read source failures
  before effect publication;
- Label-path demand followed by template-byte demand, repeated template-source
  reuse and multiple distinct sources without an extra source read;
- speculative print/effect/environment discard and terminal success/failure
  publication;
- direct-local and immutable canonical sources, exact observed file/symlink
  epochs, legacy/observed result parity, needs/cancellation, warm reuse and
  source A/B/A restoration; and
- path-visible equality remaining independent of retained routing address.

Rebuild the V2 CLI and rerun the authenticated rules_rust replay. It must clear
only the admitted `repository_ctx.template` call and stop at the next independent
generic boundary. Do not implement that boundary or add a consumer special case.

## Allowlist, caps and complexity

Production Rust may change only:

- `app/slug_loading_v2/src/repository_rule_context.rs`; and
- `app/slug_loading_v2/src/module_extension_repository_file_effect.rs`.

Proof Rust may change only adjacent `#[cfg(test)]` modules in those files.
Scheduling records may change only the canonical plan, Stages 4 and 5, and this
manifest. Do not change Bzlmod/workspace owners, generated-plan representation,
Cargo metadata, fixtures, pinned rules_cc content, repository-rule
definition/call/certificate shapes, Label parsing, source-route/materialization
shapes or file-effect publication.

Caps are 400 gross production Rust additions, 500 proof additions and 900 total.
No new function may exceed 80 lines; no existing function may grow by more than
25 lines. Keep replacement in one pure helper and source orchestration in small
mode-specific helpers because the two allowed production files already exceed
the plan guide's complexity trigger. No benchmark is required; bounded buffers,
warm DICE reuse and A/B/A proof are mandatory.

## Validation and terminal stops

Run serially:

- focused repository-context and repository-file-effect template tests;
- `cargo test -p slug_loading_v2 --lib -q` and loading integration tests touched
  by the changed private interface;
- `cargo test -p slug_query_v2 --lib -q`;
- `cargo build -p slug_cli_v2 -q` before authentic replay;
- stale `slugd` cleanup before and after replay;
- `cargo fmt --check`, `git diff --check`, archive checker and allowlist/cap
  verification.

Return `REPLAN` before or during Rust if:

- exact source bytes/epochs require a new DICE key, direct filesystem access or
  a change to Bzlmod/workspace/source/materialization owners;
- a root, built-in, self-generated, string or Label template source is needed;
- physical-path identity replaces canonical source-route identity;
- source bytes, prepared maps, substitution buffers or retry counts enter a
  retained global/cache/frontier or escape the invocation;
- an evaluator, heap, builder, capture, lock or `RefCell` borrow crosses DICE;
- source changes do not invalidate, observed epochs cannot be merged, or partial
  attempt state escapes;
- byte behavior widens beyond the Latin-1/bounded claim or replacement order is
  not exactly discriminated;
- the retained effect representation, file allowlist, caps or complexity limits
  must change; or
- another repository API or any ruleset/toolchain/platform special case is
  required to advance replay.

Architecture result: `ACCEPT`. The audit selects a direct implementation because
all semantic state already has a natural owner: routed source reads own exact
input bytes and observations, invocation state owns speculative retry scratch,
and the generated-file plan owns output bytes/mode/order. Independent terminal
review must still accept the integrated Rust and proof.
