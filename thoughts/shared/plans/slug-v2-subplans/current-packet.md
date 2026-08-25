# Current Slug V2 Packet

Packet: `WP-6-7A-generated-repository-package-policy-lookup-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design base: audit accepted 2026-08-24 / Rust `846ef196`

Result: docs-only design for the smallest same-crate core generated-repository
package lookup owner — canonical deleted-package policy, `REPO.bazel` plus
`.bazelignore` semantics, ordered `BUILD.bazel` then `BUILD` selection, and
complete epoch composition — consuming the now-visible private route/
source-observation surface. Linux under WSL is the only platform target.

## Frontier facts (accepted audit, 2026-08-24)

The core private chain reaches `Generated` routes end to end and is sibling-
nameable after `846ef196`, with zero production callers. Public bzlmod
`RootRepositoryRoute` admits only DirectLocal/BuiltinBazelTools; root package
policy checks deleted packages against `CanonicalRepoName::root()` only;
marker/BUILD-order semantics exist only inside the root/direct-local lookup.
Raw-source publication would bypass all of these and is rejected.

## Active design contract

Docs only; every Rust file, Cargo/BUILD, fixture and oracle is read-only.

The design must:

- name one cohesive core lookup key family (legacy + observed modes) whose
  natural producer owns generated-package identity for a canonical repo;
- reuse root marker/BUILD-order discriminating evidence unchanged for the
  generated families; add no fixture/oracle unless a demonstrated gap appears;
- keep deleted-package policy, repository-ignore, and BUILD selection in the
  natural policy/lookup owners without duplicating root policy state;
- compose complete epochs left-first with outer > compatible Need > semantic
  ordering; parents retain one local Result Arc plus compact epoch only;
- classify exact / Slug-native / unsupported-deferred per the guide;
- bound file allowlist, caps, validation and REPLAN stops; and
- defer public publication, bzlmod route widening, package source/load reuse,
  command/bootstrap activation and other platforms explicitly.

STOP any Rust edit, new caller/adapter/key beyond the named family, public/
crate-root exposure, fixture growth without a demonstrated gap, milestone
closure, M8/M7B or exact identity work. REPLAN if no bounded single-owner
lookup exists or if bzlmod/core dependency inversion becomes unavoidable.

After design ACCEPT, schedule exactly one implementation successor. M7 remains
partial and M7A -> M8 -> M7B remains.
