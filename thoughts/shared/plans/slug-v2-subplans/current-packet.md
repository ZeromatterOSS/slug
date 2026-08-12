# Current Slug V2 Packet

Packet: `WP-5-host-command-module-override-owner-implementation`
Milestone: cross-stage M7 prerequisite implementation
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: implement one normalized command-module-override semantic input from
command parsing through DICE, without activating discovery or a graph.

## REPLAN predecessor and source authority

`WP-5-host-selected-module-graph-owner-design` ended `REPLAN` because the
live model cannot represent command override precedence.
`BzlmodCommandPolicyKey` carries only yanked-version and dev-dependency
policy, `RootModuleOverrides` carries only root-MODULE declarations, and
`HostDiscoveredModuleKey` therefore cannot distinguish an explicit command
override from the built-in default sentinel. Legacy `ResolvedGraph` is not a
production seam.

Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is authoritative:

- `RepositoryOptions.ModuleOverrideConverter` splits once at the first `=`,
  requires `module-name=path`, validates `VALID_MODULE_NAME`, and normalizes
  the path lexically through `PathFragmentConverter`;
- `BazelRepositoryModule` folds the repeated option through a
  `LinkedHashMap`: a later nonempty value replaces the same module, an empty
  path removes its prior value, and paths become absolute against invocation
  CWD after `%workspace%` substitution;
- `ModuleFileFunction` overlays command overrides after root-MODULE
  declarations with last-value precedence, rejects an override of the root
  module only after root evaluation, and installs built-in overrides only when
  no winning explicit override is present.

The official 9.2 command reference confirms `--override_module` is repeatable,
relative paths use invocation CWD, `%workspace%` uses the workspace root, and
empty paths remove earlier overrides. No repository content is read while
normalizing this command input.

## Accepted design

### Parsing and normalization

All admitted Build, Run, Query, Cquery, and Aquery command parsers classify
`override_module` as a Bzlmod parse-only flag. The common parser preserves raw
occurrence order and emits the existing first argv error before targets or
runtime work. It accepts only an explicit value; the value splits at its first
`=`, so the path may contain later equals characters.

Module names use the pinned Bazel regex and diagnostic facts: lowercase ASCII
letters, digits, dot, hyphen, and underscore; first character lowercase; last
character lowercase or digit. Missing `=`, empty/invalid module name, and an
invalid OS path are typed command parse failures. The root module name is
grammar-valid here; Bazel's later root-override rejection stays deferred to the
effective-override consumer.

Normalization is client-owned because the client alone owns invocation CWD and
constructs both one-shot and daemon requests. The admitted current Slug boundary
requires invocation CWD to equal the normalized workspace root. Relative paths
and `%workspace%` therefore resolve against that same root; subdirectory
workspace discovery is unsupported/deferred and must fail closed rather than
guess a parent workspace. Absolute paths remain absolute. Reuse
`slug_workspace_v2::NormalizedAbsolutePath` for lexical `.`/`..`
normalization without filesystem IO, symlink resolution, canonicalization, or
existence/kind checks.

Fold occurrences in argv order into a compact effective map. A nonempty later
path replaces the value for its module; an empty later path removes it; a later
nonempty occurrence after removal re-adds it. Equality and DICE identity use
the effective module-to-normalized-absolute-path mapping, not irrelevant
distinct-module insertion order or discarded spelling/history. Error order
still follows raw argv order.

### Representation and DICE ownership

Extend `BzlmodCommandPolicyKey` with one
`CommandModuleOverrides` value. It is a V2-owned wrapper over
`SmallMap<CompactString, NormalizedAbsolutePath>`, derives/preserves
`Allocative`, and uses `Dupe`/an immutable `Arc` wrapper only if the live
clone boundary requires it. Do not add a hash digest, interner, global cache,
`BTreeMap`, raw `String` path, or second input key. Deterministic stable
serialization is display/evidence only; structural fields own equality.

`RootModuleCommandPolicy` retains the normalized overrides separately from
`RootModuleOverrides`, and the existing request injection installs the whole
policy in the same transaction before its sole commit. This packet adds no
effective override merge and no consumer. A later owner will overlay
root-declared overrides first and command overrides second; command values win,
including an explicit `bazel_tools` path that bypasses the default built-in
owner.

The value contains no `RepoSpec`, module version, root declaration,
repository source/materialization identity, file bytes, observation generation,
registry result, selected graph, or mapping. Those remain dependencies of later
owners. Complete normalized policy values are valid/equal; parse or daemon
normalization errors are terminal before DICE injection.

### One-shot, daemon, and public wire

The one-shot client normalizes before calling the retained runtime. Daemon mode
normalizes in the same client path before sending. The current
`BzlmodRequestInputs` wire has no generic normalized-flag carrier, so the
successor must add exactly one serde-defaulted
`command_module_overrides: Vec<(String, String)>` field containing the
effective module names and normalized absolute paths. No raw argv, CWD,
discarded occurrence, environment value, repository content, or secret crosses
the wire.

The server independently validates module names, requires every wire path to be
absolute and lexically normalizable, rejects duplicates instead of silently
refolding an untrusted wire value, then reconstructs the same command policy.
Old/default wire input means an empty map. Build, Run, Query, Cquery, and Aquery
forward the value; Test remains unsupported/deferred with the rest of its
runtime. Stable-daemon A/B/A must show request isolation and restored DICE
equality without daemon restart.

### Compatibility

Exact: Bazel 9.2 admitted flag grammar; module-name validation facts; first-`=`
split; ordered replace/remove/re-add fold; workspace-root invocation path
resolution; lexical absolute normalized identity; command-over-root precedence
reserved for the later merge; explicit `bazel_tools` bypass represented as a
winning command input.

Slug-native: Rust error/type spelling, normalized OS-native path storage,
compact `SmallMap` representation, DICE key names, local JSON wire framing,
and non-Bazel identity/display bytes.

Unsupported/deferred: invocation from a workspace subdirectory, native Windows
path behavior, root-name rejection timing beyond the input, RepoSpec creation,
filesystem observation or materialization, discovery/MVS, selected graph,
canonical mappings, extensions/registrations/flags, lockfile products,
package/Bzl loading, configured toolchains, Test, execution/results/BEP/
coverage, JVM/Java, and exact Bazel identity bytes.

## Frozen implementation successor

After independent design `ACCEPT`, run only
`WP-5-host-command-module-override-owner-implementation`.

Production allowlist:

- `app/slug_bzlmod_v2/src/dice.rs`;
- `app/slug_bzlmod_v2/src/module_eval.rs`;
- `app/slug_commands_v2/src/common.rs`;
- `app/slug_commands_v2/src/build.rs`;
- `app/slug_commands_v2/src/query.rs`;
- `app/slug_commands_v2/src/aquery.rs`;
- `app/slug_commands_v2/src/cquery.rs`;
- `app/slug_cli_v2/src/commands/build.rs`;
- `app/slug_cli_v2/src/commands/run.rs`;
- `app/slug_cli_v2/src/commands/query.rs`;
- `app/slug_cli_v2/src/commands/aquery.rs`;
- `app/slug_cli_v2/src/commands/cquery.rs`; and
- `app/slug_server_v2/src/server.rs`.

Test allowlist:

- `app/slug_commands_v2/tests/commands.rs`;
- `app/slug_cli_v2/tests/cli.rs`;
- `app/slug_bzlmod_v2/tests/dice_inputs.rs`; and
- `app/slug_server_v2/src/tests.rs`.

No other file, public export, Cargo/BUILD metadata, dependency, fixture, asset,
cache, lock, interner, global, filesystem observation, RepoSpec, discovery,
graph, mapping, or consumer is authorized. The existing wire struct is already
public; the sole serde-defaulted field is the only public/schema change.

Cap formatted net growth at 620 production lines, 700 test lines, and 1,320
total. This margin covers five admitted command forwarding paths and public-wire
validation, not another behavior family.

Required proof:

- parser table for absent, missing value, missing `=`, first-`=` path,
  valid edge names, invalid name classes, absolute/relative/`%workspace%`,
  lexical spelling equality, and fail-closed subdirectory invocation;
- duplicate replace, empty removal, remove/re-add, distinct-module order
  equivalence, and A/B/A path restoration;
- policy/root overlay pure proof that command wins while maps remain distinct,
  including explicit `bazel_tools` bypass representation;
- real-DICE absent/present/path A/B/A and cold/warm reuse with no content read;
- one-shot and stable-daemon Build plus one query-family discriminator, with all
  five active command request decoders forwarding or rejecting consistently;
- wire default compatibility, absolute-path guard, duplicate rejection, and no
  raw argv/CWD/environment/content fields;
- existing command, bzlmod, server, and direct core/runtime dependent suites;
  formatting, archive status, structural forbidden-edge scan, exact scope/cap/
  diff checks; and
- independent public-wire/DICE/identity implementation review.

## Terminal stops

Return `REPLAN` on unresolved pinned grammar/precedence, path normalization
requiring filesystem IO, semantic raw spelling/history in equality, loss of raw
argv error order, normalization in the daemon, relative/duplicate untrusted
wire acceptance, public field beyond the one frozen addition, a second command
policy or DICE key, root-map mutation, RepoSpec/materialization/discovery/graph
activation, eighteenth file, cap excess, or independent-review blocker.
