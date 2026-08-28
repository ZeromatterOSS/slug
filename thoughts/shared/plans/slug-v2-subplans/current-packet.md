# Current Slug V2 Packet

Packet: `WP-4-5-7A-contextual-command-overlays-architecture`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `3b8a353ef`.

Result: freeze the zero-Rust architecture and bounded implementation sequence
for contextual Starlark build-setting occurrences plus native
`--extra_toolchains` and `--extra_execution_platforms` overlays. This packet
changes no Rust and admits no configured alias/constraint selection, provider
payload, toolchain implementation analysis or ruleset-specific control flow.

## Accepted predecessor and category boundary

Commit `3b8a353ef` completes typed build-setting/config-condition category 2.
It resolves every retained selector and concatenation through one typed
resolver, batches the sole configured-condition key, exposes the complete
admitted `ctx.attr` and `ctx.outputs` shapes, derives dependencies only from
selected branches, and applies the same two-phase condition path to native
toolchain `target_settings`. Independent terminal review returns `ACCEPT`.

Category 3 must now replace the singular `@@//:setting` command bridge without
adding another setting value, converter, configuration identity or registration
parser. It owns command occurrences and their contextual preparation only.
Configured target-platform facts, registered aliases, declaration eligibility,
provider payloads, selected implementation exec configuration and
`ctx.toolchains` remain the following frozen categories.

Buck2-derived Rust remains the sole generic Starlark syntax/evaluator owner.
BCR-delivered Starlark owns every rule definition and control path, including
`cc_internal`; `cc_common` is only a demanding future client of the generic
evaluator/provider/host ABI. No command-overlay packet may add a Rust C++ rule
parser or engine.

## Learned facts and source basis

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is behavior authority.

- `StarlarkOptionsParser` receives unresolved command occurrences after native
  option parsing, loads each target, rejects non-rules and build settings with
  `flag = False`, converts text through the declaration's build-setting type,
  canonicalizes by the loaded target label, and elides effective defaults.
- Direct scalar settings are last-wins. `allow_multiple` strings and repeatable
  list/set settings accumulate occurrences in command order. String sets
  normalize to unique membership; integer overrides remain arbitrary precision;
  Boolean flags admit explicit values and the value-less positive/`no` forms.
  Every occurrence is loaded and converted even if a later scalar occurrence
  replaces it.
- Apparent root and external labels are interpreted with the selected root
  repository mapping. The existing `HostRootRepositoryMappingKey` and its
  observed counterpart already own that final mapping; loading already owns
  Root/Canonical package and build-setting declarations.
- `PlatformOptions.extra_execution_platforms` uses the comma-list converter and
  is non-repeatable, so a later occurrence replaces the earlier list.
  `extra_toolchains` uses the same converter with `allowMultiple = true`, then
  `PlatformOptions.getNormalized` deduplicates while keeping the last copy.
- `RegisteredExecutionPlatformsFunction` prepends final command patterns to
  Bzlmod patterns in configuration order. `RegisteredToolchainsFunction`
  reverses the normalized command list first because the last command
  toolchain has highest priority, then appends Bzlmod patterns.
- `TargetPatternUtil` parses command patterns as signed patterns. Expansion is
  sequential: positive rows add, negative rows remove, and the final ordered
  set deduplicates labels. Bzlmod registration patterns are always positive.
- Slug already has the complete typed native descriptor/converter row for both
  extra-registration options, one structural Starlark-option map, the final
  root mapping, Root/Canonical loading carriers, complete contextual target
  pattern syntax and two loading-owned MODULE registration expansion keys.
  Analysis currently consumes only those MODULE expansions.
- `slug_commands_v2::ParsedFlag` retains raw name/value/order, but build and
  cquery collapse only `--//:setting` to a string. The CLI, daemon wire, core
  runtime, build root and cquery roots repeat that singular bridge. Core creates
  a final configuration before DICE can resolve Bzlmod context.
- `SlugConfiguration` has no public native update boundary. Its native option
  vector and generic converter are already the sole typed identity owner, so
  extra registrations must enter through a batched descriptor-driven update,
  not new parallel fields.
- `registration_expansion.rs` owns the one contextual package/subtree expansion
  engine, but its public keys intentionally read positive-only MODULE rows.
  Command rows must reuse/generalize that engine without entering the MODULE
  key or selected-module carrier.

Primary pinned evidence is `StarlarkOptionsParser`,
`StarlarkOptionsParsingTest`, `PlatformOptions`, `PlatformOptionsTest`,
`RegisteredToolchainsFunction`, `RegisteredExecutionPlatformsFunction`,
`TargetPatternUtil` and their registration-order tests. Use a Bazel 9.2 oracle
only for a named admitted shape those sources/tests do not discriminate.

Clean Zabel `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance only.
Its `RequestSession` injects immutable option occurrences, its configuration
owners normalize typed Starlark/native final options, and its extra-registration
rows remain configuration facts rather than MODULE facts. Its keep-last native
list normalization and borrowed typed-option projections are useful ownership
and allocation ideas. Copy no Zig representation, table layout, parser,
scheduler, diagnostics, checksum policy or compatibility claim.

## Frozen architecture

### One immutable command-configuration overlay

Add one configuration-owned immutable occurrence projection shared unchanged by
build and cquery across command parsing, one-shot invocation, daemon wire and
core runtime. It is an `Arc` slice in command order with only these admitted
variants:

1. a direct Starlark build-setting occurrence retaining apparent label text,
   optional raw value and Boolean negation form;
2. one raw `extra_toolchains` value occurrence; and
3. one raw `extra_execution_platforms` value occurrence.

The projection retains no full argv, UI/remote/Bzlmod flag, parsed canonical
label, declaration, evaluator value or final configuration. Its structural
equality includes variant, raw valid-Unicode bytes, missing-versus-present
value, negation and occurrence order. An empty projection reuses one shared
empty allocation. The existing `root_string_setting` field and
`explicit_starlark_option` bridge are deleted across command, CLI, server and
core surfaces in one no-shim cutover.

`slug_commands_v2` remains the sole argv classifier. It recognizes direct
label-shaped Starlark flags and the two native names, validates the admitted
joined `--name=value` and Boolean no-value spellings, and builds the shared
projection. The daemon serializes the same typed occurrence rows, not a second
raw argv for reparsing. Unknown configuration-affecting flags continue to fail
at command admission; presentation, remote and Bzlmod inputs stay in their
existing owners.

The carrier is generic configuration infrastructure. It contains no
`cc_common`, `cc_internal`, rules_cc or rules_rust discriminator and reserves no
per-ruleset fields.

### One contextual configuration-preparation DICE boundary

Add one analysis/configuration preparation key identified structurally by
workspace, base target configuration and the immutable overlay. The build and
cquery roots retain the base configuration plus overlay until this key
completes. They do not construct or claim the final configured-output identity
before preparation.

The preparation key demands the existing final root repository mapping after
Bzlmod selection, resolves every apparent Starlark label under that mapping,
and loads every distinct Root/Canonical target declaration through existing
loading keys. It admits direct build-setting targets only. Unknown targets,
non-rules, non-build-settings, `flag = False`, wrong value types, illegal
Boolean negation, invalid scope and unsupported canonical routes fail before
any configured target or action publishes.

All occurrence labels are resolved, loaded and converted even when later
scalar rows win. Group occurrences by final canonical setting identity only
after every apparent spelling has resolved. Use the accepted declaration/value
resolver and the sole existing `StarlarkOptionValue` category:

- ordinary scalar settings select the last converted occurrence;
- allow-multiple strings and repeatable list/set settings accumulate converted
  elements in command order;
- nonrepeatable string-list/set text uses the existing build-setting converter;
- sets normalize to sorted unique membership while lists preserve order and
  duplicates;
- each final value derives scope from its loading declaration and default-equal
  values remove the configuration row.

Apply native extra-registration occurrences through one descriptor-driven
configuration update over the existing native option vector. Convert every
occurrence with the existing pinned comma-list converter. The final execution-
platform row is the last occurrence's list. Toolchain rows concatenate in
command order, then apply the existing Bazel keep-last duplicate normalization.
Explicit empty lists and lists containing empty strings remain distinct exactly
as the converter represents them. Add no `extra_toolchains` or
`extra_execution_platforms` field beside the native vector.

Build the final structural target configuration once from the base native
options plus the complete canonical nondefault Starlark map. The prepared value
retains only that configuration; declarations, mappings, grouping tables and
conversion buffers are compute scratch. The observed wrapper additionally
retains the exact union of source observations already required by its mapping
and package inputs.

Demand all independent mapping/package/declaration inputs before returning.
Observed outer frontier failure outranks Need, Need outranks semantic failure,
and deterministic cancellation publishes no prepared configuration. A corrected
same-graph request recovers without stale rows. One-shot and daemon requests use
the identical key and final configuration.

### Shared signed command-registration expansion

Keep the accepted positive-only MODULE expansion keys unchanged. Factor their
internal contextual pattern/package/subtree walker so a second loading-owned
command expansion key can supply borrowed rows with root final mapping, source
family and sign. There remains one target-pattern parser and one filesystem/
package expansion engine.

The command key is identified by workspace, final structural configuration and
registration family. It reads the two native pattern lists through a generic
typed native-option projection; it never reads the raw command overlay. For
execution platforms it walks the configured list in order. For toolchains it
walks the configured keep-last list in reverse. It strips and retains a leading
sign before parsing the remaining apparent target pattern under the root final
mapping.

Expansion applies rows sequentially to one ordered set. Positive expansions
append unseen labels; negative expansions remove every matched label; a later
positive row may reinsert a removed label at its later position. The existing
family filters and missing-target errors remain. MODULE rows stay positive and
retain each declaring module's own canonical repository/mapping context.

Replace analysis's `PreparedModuleRegistrations` scratch with one prepared
registration owner that computes both command and MODULE expansions for both
families, then folds command results before MODULE results with the same ordered
set semantics. Do not copy command patterns into `HostSelectedRegistrationPatterns`,
change a MODULE key, or parse raw MODULE text in analysis. Candidate/platform
loading consumes only the merged canonical label slices.

Across both families and sources, observed outer failure outranks the union of
Needs, Need outranks semantic failure, and no partially merged registration or
configured candidate publishes. Empty command overlays preserve accepted
MODULE-only output and DICE equality.

### Root/cquery cutover and lifetime

Build and cquery command roots carry the base configuration and overlay, demand
the prepared configuration before constructing configured target keys, and use
that same final configuration for every literal root and action-closure edge.
Configured-output claiming moves after successful preparation. Delete every
fixed-label `@@//:setting` construction and per-root explicit option copy.

Raw occurrences live only in the command request/root/preparation key. Final
typed configuration and command/MODULE expansion results are normal DICE
semantic values. Mappings, declarations, converted occurrence groups, signed
pattern rows, expansion worklists and merge sets are scratch. No lock spans a
DICE computation, and no evaluator heap value escapes synchronous evaluation.

## Bounded implementation sequence

After this architecture is independently accepted, run two implementation
packets in order.

1. `WP-4-5-7A-contextual-command-setting-preparation`: add the shared compact
   overlay and daemon cutover; add the descriptor-driven native batch update;
   add the sole contextual preparation key for all five admitted Starlark kinds
   and both native extra-registration options; delete the fixed setting bridge;
   and cut build/cquery roots over to the prepared final configuration. Do not
   consume extra registrations yet.
2. `WP-4-5-7A-command-registration-overlay-consumer`: reuse/generalize the
   loading registration walker for signed root-context command rows; compute
   command and MODULE expansions independently; merge exact command-first
   family results in configured analysis; and preserve the existing candidate
   eligibility boundary.

The first packet owns the public request/configuration representation and must
receive independent terminal retained-identity/DICE review. The second owns
loading expansion and configured consumption and must receive independent
terminal ordering/lifecycle review. Do not split by build-setting kind, command
kind or registration family and do not add an intermediate string-only
configuration.

## Compatibility classification

- **Exact:** direct root/apparent-external Bazel 9.2 build-setting occurrences
  in the admitted joined-value and Boolean forms; declaration authentication;
  all five admitted value kinds; scalar last-wins; multi/repeat accumulation;
  set normalization; default elision; native extra-registration comma
  conversion, replacement/accumulation and keep-last normalization; signed
  command expansion; command-before-MODULE registration order; and unchanged
  MODULE-only behavior.
- **Slug-native:** compact raw occurrence layout, DICE key/result decomposition,
  Rust structural configuration identity and unproved diagnostic wording.
- **Unsupported/deferred:** `.bazelrc`/`--config` expansion, split-token native
  option spelling, `--flag_alias` and MODULE `flag_alias`, unconfigured alias
  chains to build-setting targets, label-valued native settings, feature flags,
  project scope, configured target-platform constraint truth, registered target
  aliases/eligibility, provider payloads, selected implementation analysis,
  broader commands, exact Bazel checksum/output bytes and any Rust
  implementation of BCR rule flow.

## Proof obligations

1. One overlay round-trips one-shot and daemon build/cquery with exact order,
   missing value, negation, empty string and Unicode identity; the singular
   setting fields/types and every fixed `@@//:setting` construction are absent.
2. Root and apparent-external direct flags resolve through the selected final
   mapping. Mapping, declaration kind/default/scope and raw occurrence changes
   invalidate independently; equal final values converge structurally.
3. Every admitted scalar/list/set kind covers single, repeated, default-equal,
   malformed and non-flag cases. Earlier malformed/unknown scalar occurrences
   still fail even when a later row would win.
4. Outer > Need > semantic ordering, deterministic cold cancellation, no
   partial final configuration and same-graph recovery hold across multiple
   setting packages and both native families.
5. Native option scans prove both extras live only in the existing option
   vector and canonical bytes. Execution-platform last-wins, toolchain
   accumulation/keep-last, explicit empty and embedded empty members are
   discriminated.
6. Signed single/package/recursive command patterns reuse the existing
   contextual expansion engine. Positive/add, negative/remove and later
   reinsertion preserve exact order for root and mapped external packages.
7. Command toolchains reverse after keep-last normalization; execution
   platforms do not. Both command families precede positive MODULE rows and
   deduplicate/remove against the combined ordered set.
8. Empty overlays retain accepted MODULE expansion/output identity and activate
   no command expansion packages. Unrelated command settings do not invalidate
   MODULE expansion keys.
9. Build and cquery roots analyze every target with the same prepared final
   configuration; changed settings/registrations produce changed structural
   keys and A/B/A restores results in one-shot and stable daemon modes.
10. Source/retained-size scans prove one argv classifier, one overlay, one
    Starlark option map/converter path, one native vector, one contextual
    registration parser/walker, no raw command field in MODULE keys, no copied
    defaults/declarations, no evaluator retention and no ruleset discriminator.

## Allowlist, cap and validation

This is a zero-Rust architecture packet. Writable files are only:

1. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
2. `thoughts/shared/plans/slug-v2-subplans/current-packet.md`; and
3. `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`.

Cap: 700 net documentation lines. No source, test, fixture, oracle, Cargo,
lockfile, BUILD, Zabel, Buck2 or routing-log file is admitted.

Validate canonical/current packet ID and base agreement; pinned Bazel 9.2
source/test anchors for every occurrence and registration rule; clean Zabel
commit plus guidance-only wording; zero Rust diff; exact allowlist/cap;
`scripts/v2_archive_status.sh`; `git diff --check`; and independent
architecture/DICE/retained-representation review.

## Architecture review and stops

This packet edits only the canonical plan, Stage 6 owner plan and current
manifest. Obtain independent review of Bazel occurrence semantics,
configuration identity, DICE ownership, signed registration order, retained
memory and the BCR/Buck2 boundary before committing the architecture.

Independent pre-review returns `ACCEPT`: the pinned occurrence conversions,
toolchain normalize-then-reverse order, signed command-before-MODULE fold,
post-Bzlmod DICE ownership, compact lifetime, two-packet cut and generic
BCR/`cc_common` boundary are coherent and bounded.

STOP and `REPLAN` for a second argv/configuration/target-pattern parser; a
parallel Starlark or native option store; raw command input in a MODULE key;
configuration construction before final repository mapping; skipped earlier
occurrence validation; copied build-setting defaults/declarations; source-order
loss; command patterns appended after MODULE rows; unsigned command expansion;
provider/platform eligibility breadth; evaluator values in retained state; a
bootstrap-only or C++-specific path; Rust `cc_internal`/rule control flow;
Zabel authority; a lock across DICE; or a material contract correction.
