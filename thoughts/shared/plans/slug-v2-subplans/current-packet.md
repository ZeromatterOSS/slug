# Current Slug V2 Packet

Packet: `WP-4-5-7A-typed-scoped-option-map-migration`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `57b1e8a1f`.

Result: replace the singleton root string-setting slot throughout Slug's
configuration identity and direct consumer closure with one compact, sorted,
canonical-label-keyed typed scoped-option map. Version the Slug-native
canonical-byte projection, implement target-to-exec scope projection, and
remove the singleton API in one no-shim migration. This packet creates the
general retained carrier; it does not load declarations or parse new command
or transition values.

## Accepted predecessor and boundaries

Commits `b949ce8da` and `57b1e8a1f` accept the full-category architecture and
its loading half. Loading owns all five definition/default shapes and magic
scope observations. The following value-resolution packet will authenticate a
referenced declaration, convert an input and elide a default-equal row.

Buck2-derived Rust remains the sole syntax owner. BCR Starlark owns every rule
and control path including `cc_internal`; `cc_common` is only a generic
evaluator/provider/host-ABI consumer. Pinned Bazel 9.2 at
`8220c6198837d5c13d53fea211cf3282aa12408a` is behavior authority. Clean Zabel
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer architecture and
optimization guidance only.

## Live preflight

- `SlugConfigurationData` retains one `Option<RootStringSettingValue>` beside
  the native row and encodes it under projection version 1.
- `to_exec()` blindly carries that singleton. The accepted architecture needs
  scope-aware projection: universal survives, target is removed, project
  fails closed, and default follows existing native propagation policy.
- The public singleton reaches configuration, analysis keys/DICE/context,
  command/CLI/server adapters and tests. All direct consumers change together;
  the prototype gets no compatibility alias.
- Workspace `num-bigint` is already locked with `Allocative` and `StrongHash`
  support. Only the configuration crate dependency row is missing.
- This is retained hot-path data. Follow `slug-buck2-utility-reuse`: use
  immutable `Arc` slices and canonical labels, not another general map,
  interner, cache or text identity.

## Implementation contract

### One typed scoped-option representation

Add public evaluator-independent values for arbitrary-precision integer,
Boolean, compact string, ordered duplicate-preserving string list, and sorted
unique string set, plus scope `Default`, `Universal`, `Target` or `Project`.
One entry owns a `CanonicalLabel`, kind-preserving value and scope. One map owns
a canonical-label-sorted unique `Arc` slice. Construction sorts once, rejects
duplicate canonical labels and normalizes set membership once. Get/replace/
remove preserve immutable structural identity and cheap clones. Retain no text
label, evaluator value, parallel scope map or kind-specific store.

The accepted final semantic map contains only nondefault effective overrides,
but this representation-only migration cannot yet enforce that invariant. The
live singleton path copies the loaded declaration default into configuration
identity when no explicit input exists, and cannot recognize an explicit
default-equal input without the declaration lookup reserved for the next
packet. Preserve those current observable semantics through the new typed map:
the temporary bridge may therefore contain a default-equal effective string
row. It is not a second store, source tag or compatibility singleton.

This is an explicit fallback-ledger item. The immediately following
`WP-4-5-7A-typed-build-setting-value-resolution` packet must delete the bridge
behavior by authenticating the declaration and omitting/removing default-equal
rows. Its required proofs are: absent input reads the loaded declaration
fallback with no retained row; explicit default-equal input removes/omits the
row; nondefault input retains one typed scoped row; and A/B/A restores map and
configuration identity. No packet after that resolution may preserve or
reintroduce copied defaults.

### Configuration identity and exec projection

Replace the singleton in production and legacy identities with the map. Rename
all accessors/constructors to the general API; delete `RootStringSettingValue`,
`root_string_setting` and `with_root_string_setting` rather than keeping shims.

Bump the Slug-native projection version/context and encode every entry with
explicit label, scope and value-kind tags plus length-delimited contents. All
typed values, including empty and zero, remain distinct where semantically
distinct. Map input order cannot affect Eq, Hash, ordering, display or path
tokens.

`to_exec()` filters target rows, carries universal rows and carries default
rows under the admitted native default-propagation policy. Any project row
returns a typed error until `PROJECT.scl` has its own owner. Native options and
configuration kind remain unchanged.

### One no-shim direct-consumer cutover

Migrate every direct singleton consumer in configuration, analysis,
commands/CLI, core runtime and server code. Preserve the admitted root string
flag by constructing one canonical string entry at the same observation
boundary; do not broaden flag grammar, interpret other kinds, demand loading
or add a compatibility slot.

Starlark `ctx.build_setting_value` reads the typed entry only for the admitted
string case and preserves its default fallback. Existing transition
preparation replaces/removes the same canonical-label entry. General typed
declaration resolution belongs to the next packet.

## Compatibility classification

- **Exact:** typed distinctions, arbitrary-precision configured integer
  capacity, canonical-label identity, per-row scope identity, universal/target
  exec propagation and current admitted root-string behavior.
- **Slug-native:** Rust layout, deterministic set/map order, versioned
  canonical bytes, projection/display/path tokens and diagnostic wording.
- **Unsupported/deferred:** declaration-authenticated conversion and final
  default-row elision (the temporary effective-row bridge is explicitly
  Slug-native migration state), project scope, command occurrences, transition
  values, configured conditions/selectors, provider payloads, platform/
  toolchain choice and Bazel checksum/output bytes.

## Proof obligations

1. Every value kind, empty value and scope participates in structural Eq,
   Hash, ordering and version-2 bytes; BigInt values do not narrow.
2. Construction sorts labels, rejects collisions, normalizes sets and is
   invariant to input order.
3. Exec projection carries universal/default, removes target and rejects
   project without changing native options.
4. Old singleton symbols disappear from production and tests; all direct
   consumers compile with no shim or second field.
5. Root string command, transition, analysis, Starlark context,
   cquery/build/run and daemon A/B/A behavior remains unchanged, including the
   temporary default-equal effective row when no override exists.
6. Tests name the bridge explicitly and do not claim nondefault-only identity;
   the next packet owns absent-without-row and explicit-default-removal proofs.
7. Compact-clone and retained-size checks cover empty, singleton and mixed
   maps; canonical bytes are built once per immutable configuration.

Reuse accepted Bazel scope/value anchors and current root-string lifecycle
tests. Add no oracle: this changes Slug-native identity while preserving the
already proved observable string slice.

## Ownership and memory

The map lives only in immutable configuration results and participates
structurally in DICE keys. Use `Arc<[Entry]>`, `CompactString`,
`CanonicalLabel`, `BigInt`, `Dupe`, `Allocative` and bounded construction
scratch. Add no retained `HashMap`, interner, evaluator heap, text hash, second
byte buffer, global store or lock. Read `docs/developers/dice.md` before key
consumer edits and hold no lock across a DICE computation.

## Allowlist and caps

Production/configuration:

1. `app/slug_configuration_v2/Cargo.toml`;
2. `app/slug_configuration_v2/src/lib.rs`;
3. `app/slug_configuration_v2/src/native/mod.rs`;
4. `app/slug_configuration_v2/src/native/configuration.rs`;
5. `app/slug_analysis_v2/src/key.rs`;
6. `app/slug_analysis_v2/src/dice.rs`;
7. `app/slug_analysis_v2/src/starlark_rule.rs`;
8. `app/slug_commands_v2/src/build.rs`;
9. `app/slug_commands_v2/src/cquery.rs`;
10. `app/slug_cli_v2/src/commands/build.rs`;
11. `app/slug_cli_v2/src/commands/cquery.rs`;
12. `app/slug_cli_v2/src/commands/run.rs`;
13. `app/slug_core_v2/src/runtime/configured_output.rs`;
14. `app/slug_core_v2/src/runtime/dice.rs`;
15. `app/slug_core_v2/src/runtime/mod.rs`;
16. `app/slug_server_v2/src/lib.rs`;
17. `app/slug_server_v2/src/server.rs`.

Proof:

18. `app/slug_configuration_v2/src/native/tests.rs`;
19. `app/slug_analysis_v2/tests/configured_target.rs`;
20. `app/slug_analysis_v2/tests/root_analysis.rs`;
21. `app/slug_analysis_v2/tests/starlark_rule.rs`;
22. `app/slug_core_v2/src/runtime/tests/cquery_command_tests.rs`;
23. `app/slug_server_v2/src/tests.rs`.

Completion docs remain the canonical plan, this manifest and Stage 6 owner
plan. Caps: 1,500 production Rust lines, 1,800 proof Rust lines, 3,300 total
Rust lines and 220 completion-ledger lines. `configuration.rs` and `dice.rs`
remain cohesive owners for this single public migration. Cargo lock/root
manifest, loading/query crates, fixtures, oracle and Zabel files are excluded.

## Validation

Run serially: focused configuration identity/exec tests; focused analysis
root-string/transition/context lifecycle tests; `cargo test -p
slug_configuration_v2`; `cargo test -p slug_analysis_v2`; direct checks/tests
for commands, CLI, core and server consumers; `cargo fmt --all -- --check`;
`git diff --check`; exact allowlist/caps; named archive baseline; and an
independent retained-identity/DICE review.

## Stops

STOP and `REPLAN` for a required file outside the allowlist; lockfile/root
dependency change; retained hash map/interner/evaluator value; copied loading
defaults beyond the named temporary effective-row bridge; parallel scope/value
stores; incomplete singleton removal; i32
configured integer narrowing; list/set conflation; unversioned byte grammar;
project-scope acceptance; loading lookup or command/transition conversion;
condition/selector/provider/platform/toolchain work; Rust BCR rule flow,
`cc_internal` or `cc_common` parsing; Zabel authority; cap overflow; or a
second material contract correction.
