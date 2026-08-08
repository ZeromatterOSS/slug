# Current Slug V2 Packet

Packet: `WP-6-m2-slug-native-configuration-identity-boundary-design`
Milestone: M2 analysis graph
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: one reviewed semantic-identity firewall and observable successor.

## Goal and required evidence

Inventory every live consumer of `ConfigurationChecksum`/`ConfigurationKey`,
stable serialization, `ConfiguredTargetKey` and DICE equality,
`BazelLayout::bazel_out`, configured paths/aquery, action ownership, and REAPI
AC/CAS. Freeze distinct domains for complete structural semantic identity,
versioned display/path projection, deferred Bazel checksum, deferred Bazel
ActionKey, and exact REAPI/content digests.

Classify each affected surface as exact, Slug-native, or unsupported/deferred.
Preserve exact graph, transition, platform/toolchain, provider/action,
invalidation, artifact content/type/mode/symlink, lifecycle, and CAS behavior.
Only configuration/path/ActionKey identity bytes and named Host/regex edge
behavior may use the approved Slug-native contract.

## Stops and budget

No Rust, Cargo/dependency, hash algorithm, DICE key, wire, output path, aquery,
action/cache activation, fixture, oracle, JVM, Java bytecode/helper/probe, or
Bazel delegation is allowed. Add no caller of placeholder `first-build` or a
digest-only semantic key. Do not use `DefaultHasher`, a truncated/unprefixed
Bazel-looking token, configured path, or REAPI digest as configuration equality.
Unmodeled configuration-affecting inputs must fail closed.

The design must schedule one complete observable successor:
`WP-6-m2-slug-native-default-configuration-vertical`. It replaces production
`first-build` end-to-end for the admitted no-argument/default configuration and
accepted root string transition, consumes one Rust-native process Host snapshot,
uses structural typed defaults, produces a versioned namespaced Slug display/
path projection, proves one-shot/daemon C0 -> C1 -> C0 equality/invalidation,
and rejects unsupported explicit inputs. Independent identity/cache review is
required before implementation. Owner-plan documentation cap is 260 lines;
terminal scheduling may change this manifest and at most 15 canonical lines.
