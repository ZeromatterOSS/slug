# Current Slug V2 Packet

Packet: `WP-6-m2-integrated-toolchain-resolution-context-design`
Milestone: M2 successful toolchain/platform selection
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: design one real DICE-owned selection and prepared `ctx.toolchains`
vertical
Predecessor: accepted Bazel 9.2 first-compatible evidence `ed4baf08`, ordered
root registrations `4a3af8df`, native declarations `6a457406`, and frozen rule
requirements/load-only ToolchainInfo `1d6106bd`.

This packet is read-only design. Inspect the live root registration anchor,
root package-loading value, native declaration representation, frozen
`required_toolchains`, configured-analysis recursion, provider decoder, rule
context preparation, and dormant toolchain scaffolding. Read
`docs/developers/dice.md` before proposing DICE ownership or locking.

The design must specify one bounded vertical that:

- maps ordered root execution-platform and registered-toolchain labels to
  canonical root package targets without losing MODULE order;
- validates the exact accepted declaration kinds and constraint references;
- selects the first compatible execution platform and matching registered
  toolchain for the one mandatory requested type;
- analyzes the selected implementation through the existing configured target
  owner;
- decodes one dedicated builtin ToolchainInfo value without masquerading as a
  user provider; and
- prepares the requesting Starlark context so
  `ctx.toolchains["//:demo_type"].marker` observes the accepted marker.

Freeze exact DICE key/value ownership, dependency direction, semantic equality,
Need/error/event precedence, canonical/apparent identity handling, first-match
ordering, implementation allowlist, line caps, and discriminating cold/warm,
reorder/edit/A-to-B-to-A/delete/recreate tests. Reuse the six accepted oracle
rows; request new Bazel evidence only for a demonstrated semantic gap.

Reject a dormant resolver-only key, the existing digest-string
`RegisteredToolchainsKey` as owner, a second package/source graph, lock held
across DICE compute, user-provider ToolchainInfo, configuration checksum,
query/cquery/aquery expansion, public failure diagnostics, optional/multiple
types, aliases, external repositories, host fallback, target-platform
constraints, exec groups, actions, execution, REAPI, or JVM/Bazel delegation.

No Rust, fixture, oracle, or generated evidence change is authorized until the
design and an independent reserved-boundary review return `ACCEPT`.
