# Current Slug V2 Packet

Packet: `WP-6-m2-option-label-context-identity`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: add an identity-owned, mapping-provenance-free resolved option-label
value plus the closed three-mode option-label parsing/resolution seam needed by
the accepted 41 contextual label descriptors.

Predecessors are authoritative: the live provenance-bearing identity types,
the accepted `287 + 8 + 5 + 41` descriptor partition, pure kernel in
`e7067bfc`, and Host/repository conversion-context design. Reuse the pinned
Bazel option-label source paths and existing identity validation. Add no oracle,
fixture, loader, configuration value, or target behavior.

Preserve `CanonicalLabel` exactly as the loading/resolution value whose derived
equality/order/hash and `StableSerialize` include `mapping_id`. Do not weaken,
reinterpret, or migrate that behavior. Add a distinct resolved option-label
type containing only canonical repository, package, and target identity. Its
equality, ordering, hashing, Bazel rendering, and stable serialization must be
independent of repository-mapping provenance.

Implement one closed parser mode over supplied facts only:

- `FirstRoundCanonical`: the deliberately mapping-free first option parse; it
  is not an empty repository mapping and may not reuse second-round output;
- `MainRepository { mapping }`: second-round option parsing from the main
  repository through the supplied `RepositoryMapping`; and
- `Package { base_package, mapping }`: package/Starlark-relative parsing through
  the supplied `PackageIdentifier` and mapping.

The seam implements only pinned option-label grammar. It must distinguish
package-relative, main-repository absolute, apparent-repository, and canonical
forms exactly; apply Bazel's option-specific main-repository prefixing only in
the modes that do so; resolve apparent repositories through the supplied
mapping; and project to the mapping-free option-label result. No mode may load
a package, materialize or discover a repository, access a filesystem, parse a
command prefix, or construct a configuration/target.

Required direct tests:

- existing `CanonicalLabel` provenance-sensitive equality, hash, stable
  serialization, and rendering remain unchanged;
- different mappings that resolve an apparent spelling to the same canonical
  repository/package/target produce equal, identically rendered option labels;
- mappings that resolve differently produce different option labels;
- first-round, main-repository, and supplied-package parsing are directly
  discriminated, including package-relative and apparent-repository forms;
- invalid or mode-inappropriate forms reject without loading or fallback;
- option-label equality/order/hash/stable rendering use only repository,
  package, and target; and
- retained allocation and clone behavior use the existing identity string
  representations without an interner or hidden map.

Allowlist:

- `app/slug_identity_v2/src/lib.rs`
- `app/slug_identity_v2/src/label.rs`
- `app/slug_identity_v2/src/serialization.rs`
- `app/slug_identity_v2/tests/label_roundtrip.rs`
- terminal scheduling only:
  `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`,
  `thoughts/shared/plans/slug-v2-subplans/current-packet.md`, and
  `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- routing only on `REPLAN`:
  `.codex/skills/slug-agent-orchestration/references/routing-log.md` and
  `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

Caps: 650 formatted production, 450 test, and 1,100 total net lines. Add no
dependency, Cargo/lockfile change, generated source, runtime registry/map/cache,
global, interner, fixture, or oracle.

Validation: formatting, serial `cargo test -p slug_identity_v2`, direct
dependent compile checks for the new public identity seam, applicable
GNU-Windows no-run, `scripts/v2_archive_status.sh`, `git diff --check`, scope
and cap checks, then independent latest-diff review.

Stop and `REPLAN` on unresolved exact option-label grammar, a required package
parser/loading call, repository materialization or discovery, a change to
`CanonicalLabel` provenance semantics, dependency/lockfile expansion, DICE,
filesystem/Host access, configuration or target construction, command-only
`RunUnder` tokenization, or any configured-target dependency edge. Cycles
remain explicitly deferred by user approval.
