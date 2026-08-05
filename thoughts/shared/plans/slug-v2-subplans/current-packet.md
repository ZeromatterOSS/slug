# Current Slug V2 Packet

Packet: `WP-6-m2-option-label-context-identity-retry`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: retain exact non-visible repository identity and add the distinct,
mapping-provenance-free resolved option-label value plus its closed three-mode
parser in `slug_identity_v2`.

Predecessors are authoritative: the live identity APIs, the accepted
Host/repository context design, the stopped first attempt, and the accepted
non-visible repository identity/source-order design. Reuse pinned Bazel 9.2
`RepositoryMapping#get`, `RepositoryName`, `SpellChecker`, label option
conversion, and natural-order behavior. Add no oracle or fixture.

Phase 1 is tests and private scaffolding only. Before production behavior,
activate direct tests for:

- existing `RepositoryMapping::resolve`, mapping equality, `CanonicalLabel`
  provenance equality/hash/order/rendering/stable serialization, and live label
  parsing remaining unchanged;
- mapped same-result/different-mapping-ID and different-result cases;
- unmapped same-apparent main/package owners, unmapped versus direct visible
  canonical, first-round one-`@` canonical versus second-round apparent lookup,
  and explicit `@//` versus unqualified `//`;
- exact non-visible rendering with no suggestion and with
  ` (did you mean '<candidate>'?)`;
- source-order tie candidates `baa, aab` for missing `aaa`, and the reversed
  order, proving first-wins suffix/identity while mapping content equality is
  unchanged;
- lawful structural equality/order/hash versus the separate Bazel-natural
  comparison that may return equal for unequal visible/non-visible or
  different-owner values; use Java UTF-16 order; and
- the accepted first-round/main-repository/package grammar matrix: First/Main
  prepend `//` only when input starts with neither `/` nor `@`, so `pkg/t:bin`
  becomes root `//pkg/t:bin`, `:bin` is root package, and bare `bin` is
  `//bin:bin`; Package instead resolves `:bin` and bare `bin` in its base and
  rejects `pkg/t:bin`; Package `//tools:bin` uses its current repository except
  special `//conditions:default`, which is main; mapping modes resolve
  apparent `@repo` (including shorthand), while direct `@@repo` bypasses the
  mapping and FirstRound treats one-`@` as a visible literal; reject leading
  single `/`, triple-dot package forms, and invalid repository names.

Independent latest-test review gates Phase 2. Production then:

1. Retain final unique repository-mapping keys in insertion order beside the
   existing `BTreeMap`; replacement keeps first position. Preserve `resolve`
   exactly and implement mapping equality over the existing ID plus entry
   contents only, ignoring candidate order.
2. Add an option-only lookup that returns mapped visible identity or exact
   non-visible requested/owner/did-you-mean-suffix identity. Port only the
   source spellchecker path: Java-compatible lowercase/UTF-16 length and
   bounded Levenshtein, strict-better first-wins traversal in retained order,
   and exact suffix formatting.
3. Add the distinct resolved option-label value and closed parser modes:
   `FirstRoundCanonical`, `MainRepository { mapping }`, and
   `Package { base_package, mapping }`. Main supplies root owner; Package uses
   `base_package.repo()`; direct `@@` and first-round canonical results are
   visible. Preserve explicit apparent-root syntax through lookup.
4. Give the new label lawful structural `Eq`/`Ord`/`Hash`, `Allocative`, exact
   canonical/unambiguous rendering, and a separate non-key
   `bazel_natural_cmp`. Do not implement `StableSerialize`; checksum/wire is
   deferred. Preserve `CanonicalLabel` and every existing identity API.

The parser consumes supplied facts only. It does not load a package,
materialize/discover a repository, access a filesystem, parse a command prefix,
or construct a configuration/target. Repository-use failure and its later
`No repository visible...` diagnostic remain deferred.

Allowlist:

- `app/slug_identity_v2/src/lib.rs`
- `app/slug_identity_v2/src/label.rs`
- `app/slug_identity_v2/src/repo_mapping.rs`
- `app/slug_identity_v2/tests/label_roundtrip.rs`
- terminal scheduling only:
  `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`,
  `thoughts/shared/plans/slug-v2-subplans/current-packet.md`, and
  `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- routing only on `REPLAN`:
  `.codex/skills/slug-agent-orchestration/references/routing-log.md` and
  `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

Caps: 850 formatted production, 550 test, and 1,400 total net lines. Add no
dependency, Cargo/lockfile change, generated source, second runtime map/cache,
global, interner, fixture, or oracle. Reuse existing owned repository/name
strings and `BTreeMap`; one ordered key vector is the accepted retained cost.
No `Dupe` or per-label `Arc`.

Validation: Phase-1 focused test compile/run and independent latest-test
review; then formatting, serial `cargo test -p slug_identity_v2`, direct
dependent compile checks for the public seam, applicable GNU-Windows no-run,
`scripts/v2_archive_status.sh`, `git diff --check`, exact scope/cap checks, and
independent latest-diff review.

Stop and `REPLAN` on source-order loss, source spellchecker ambiguity, a change
to existing visible mapping/label behavior or stable serialization, a required
package parser/loading call, repository materialization/discovery, dependency
or lockfile expansion, cap breach, DICE, filesystem/Host access, configuration
or target construction, command tokenization, any dependency cycle, or any
configured-target edge. Configured-target dependency cycles remain explicitly
deferred by user approval.
