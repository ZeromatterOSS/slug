# Current Slug V2 Packet

Packet: `WP-6-m2-pure-native-value-default-and-rendering-kernel-retry`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: retry the discarded pure native value/default/rendering kernel against
the now-closed pinned-source discriminator set; every contextual or Java-regex
path must refuse explicitly.
Predecessor: `WP-6-m2-pure-native-value-default-and-rendering-kernel` reached
`REPLAN` after its permitted correction and a later DottedVersion source miss;
all unaccepted Rust was discarded cleanly.

Within `slug_configuration_v2`, add:

- a closed structurally equal `NativeValue` algebra for primitive/text/enum/
  dotted-version/list/entry/ordered-map/environment/fission values and a
  positive-only internally constructible `RunsPerTestSeed`;
- source-default materialization with exact annotation `"null"` semantics,
  repeatable empty behavior, all literal categories, and a private pinned table
  for the six symbolic label expressions, without resolving labels;
- descriptor-directed one-occurrence conversion for exactly the 287 pure
  descriptor paths and explicit typed refusal for eight Java-regex, five Host,
  and 41 repository/package/loading descriptors; and
- exact source-backed Java `value.toString()` projection into the accepted
  outer cache grammar.

Freeze the retry corrections in source-pinned tests:

- `TestTimeoutConverter` uses decimal `Integer.parseInt`, Guava six-token-limit
  empty-token behavior, uppercase enum-map keys, and canonical Java duration
  text (`PT1M`, `PT5M`, `PT15M`, `PT1H`, and mixed H/M/S);
- `StripMode` renders lowercase; an empty fission list routes to outer `EMPTY`;
  the numeric runs seed cannot represent zero/negative or be constructed by a
  public caller; and Void expansion rows remain absent/default-only rather than
  rendering a scalar `"null"`;
- DottedVersion implements case-insensitive component grammar
  `(\\d+)([a-z0-9]*?)?(\\d+)?` and descriptive terminator grammar
  `([a-z]\\w*)`; both numeric captures use decimal Java `Integer.parseInt`
  signed-32-bit bounds with no sign/radix spelling, the ASCII descriptive
  component `[A-Za-z][A-Za-z0-9_]*` is accepted only after a numeric component
  and stops later validation, and the complete original input governs both
  structural equality and rendering; and
- discriminate `2147483647`/`2147483648`,
  `1alpha2147483647`/`1alpha2147483648`, `1.internal_build`,
  `1.2.internal_build.!`, and exact retained `1.0.0` text.

Apply `.codex/skills/slug-buck2-utility-reuse/SKILL.md`. Use `CompactString`
for retained dynamic scalar text, immutable `Arc<[T]>`/`Arc<[(T,T)]>` slices,
and `Allocative`. Use `Dupe` only for a demonstrably pointer-cheap aggregate;
never mark owned leaves cheap. Preserve structural equality and null-versus-
empty identity. No runtime descriptor map, interner, cache, global, hash,
generic unordered map, or derived Rust `Debug`/`Display` cache text.

Allowlist:

- `app/slug_configuration_v2/Cargo.toml`
- `app/slug_configuration_v2/src/native/mod.rs`
- `app/slug_configuration_v2/src/native/cache_grammar.rs`
- `app/slug_configuration_v2/src/native/tests.rs`
- `app/slug_configuration_v2/src/native/value.rs`
- `app/slug_configuration_v2/src/native/defaults.rs`
- `app/slug_configuration_v2/src/native/convert.rs`

Caps: 1,550 production, 1,250 test, and 2,800 total formatted net lines across
the seven files. Add only demonstrated retained workspace dependencies; no
external dependency/version, registry, root workspace, ignored lockfile,
generated source/data, fixture, oracle, scheduling, or downstream change.

Acceptance also requires all 341 routing results and exact 287/8/5/41 counts;
all default families and six symbolic values; special-null versus explicit
`null`; repeatable empty versus scalar/list empty; every pure converter and
enum spelling; exact runs seed; list/entry/map/env/fission/duration/dotted text;
`NULL`/`EMPTY`/escaping; structural equality; valid-Unicode Java UTF-16 order;
and typed refusal of every deferred family. Reuse the independent 341-row
metadata table.

Stop and `REPLAN` on any new material source/rendering correction; Java pattern
generation or a general `PerLabelOptions`/`runs_per_test` regex branch;
lone-surrogate/lossy UTF-8 behavior; Host or repository/package/
loading/Starlark access; argv/RC/repeat/expansion/implicit/alias behavior; any
P/C/T normalization; generic map/record rendering; checksum, command, wire,
DICE, analysis, path, platform, ActionKey, aquery, or execution work; retained-
state ambiguity; cap breach; or any change outside the allowlist. Configured-
target dependency cycles remain deferred with user approval.
