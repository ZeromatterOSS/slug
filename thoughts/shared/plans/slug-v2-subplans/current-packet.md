# Current Slug V2 Packet

Packet: `WP-6-7A-configured-action-environment-owner-implementation-r1`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 action declaration.

Status: Implementation `ACCEPT`. The generic Args/spawn/symlink architecture
is accepted in `94fd24e9f`; this packet terminally accepts only its configured-
action-environment prerequisite and authorizes the bounded FDO action successor
for planning.

Base: `94fd24e9f`. The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked;
do not edit or stage it.

## Observable result

One structural `SlugConfiguration` owns the complete configured environment
needed by a later generic spawn action. The admitted joined command forms for
`action_env`, `host_action_env`, and
`incompatible_strict_action_env`/`experimental_strict_action_env` convert into
the existing native option vector. Target-to-Exec conversion copies
`host_action_env` to `action_env`. A public immutable projection exposes
canonical fixed variables and inherited names, and composes them with a
validated per-action environment exactly as Bazel does for
`use_default_shell_env`.

This packet registers no action and changes no parser, evaluator, rule,
provider, executor, ActionKey, REAPI digest, or output-path behavior. It is a
generic configuration prerequisite; rules_cc FDO is only the first later
consumer.

## Learned facts and authority

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is the sole semantic authority. The relevant authenticated sources are:

- `ActionEnvironment.java` SHA-256
  `8bca177613e8ee21181728e81b8ae04455631ab8ae91abb05b648828cb555ef5`:
  semantic state is a fixed map plus inherited-name set; `split` canonicalizes
  through sorted scratch and action fingerprints consume the two domains
  separately.
- `BazelRuleClassProvider.java` SHA-256
  `a7de1ba5a700468ead269865f2563378ea0851d3430844ee6491591e52fd3d91`:
  strict/non-strict PATH, `LD_LIBRARY_PATH`, `BAZEL_SH`, Windows `PATH` and
  `SYSTEMROOT`, action option precedence, and `RUNFILES_MANIFEST_ONLY` order.
- `CoreOptions.java` SHA-256
  `89835ed74107b21f7c51b4723e16be8b96b3c1bf43855fc63220b1dd21f5c67a`
  and `FragmentOptions.java` SHA-256
  `b796aff8846c477982775743833b64a5da2817333e8a992f7f222cdd38f423d4`:
  allow-multiple conversion is normalized by name with the last occurrence's
  value while the first key position is retained in the native option list.
- `Converters.java` SHA-256
  `808b7fa13239fb48783552d504803e07f42ce36ca1608d7edbf644ae6f02fbd8`:
  `NAME=VALUE`, `NAME`, and `=NAME` mean set, inherit, and unset; empty input
  and bare `=` reject.
- `BuildConfigurationValue.java` SHA-256
  `5e715c71ebdf3f3df2cf978c7435397d0fac8fcfb9e98e3715006a3a3d911bf9`:
  `enable_runfiles=auto` is enabled except on Windows.
- `builtin_exec_platforms.bzl` SHA-256
  `b61da947cdbd18f1d12411a057c3b88b26fff399e80d6f903e8d88eb4215956a`:
  the Exec transition assigns `host_action_env` to `action_env` and propagates
  the strict, runfiles, and shell option state.
- `SpawnAction.java`: `createActionEnvironment` removes action-provided keys
  from configured inherited names before fixed action values override the
  configured fixed map; false uses only the action map.

Pinned proof sources are `action_env_test.sh` SHA-256
`94c4e0f8c47051e821d97f8e6dcc7d58e5f6d06af6519a03d47c9ef22a1ca03c`
and `BazelRuleClassProviderTest.java` SHA-256
`f0421b38b5f761ec6d03feac5f4070916ef3b5ecca22d6686a5a94ada99cfe63`.
They already discriminate set/inherit/unset, repeats, empty fixed values,
fixed/inherited override, target/Exec separation, strict defaults and Windows
path construction. Add no fresh oracle unless implementation exposes a source
ambiguity.

The existing Slug converter already retains `EnvValue::{Set, Inherit, Unset}`
and default descriptors already contain empty `action_env`/`host_action_env`,
strict true, `enable_runfiles=auto`, and absent `shell_executable`. The missing
owners are command admission, effective projection, Host facts, and the Exec
rewrite.

Zabel `0795445f3ab60f4e49070bdd0b94425c5610f73a` is concept-only peer
guidance for canonical immutable environment storage and action-finalization
boundaries. Copy no code, names, fingerprints, tests, or compatibility claim.
Buck2-derived guidance selects compact `Arc` slices, `CompactString`, `Dupe`,
and `Allocative`; no Buck2 map, interner, option store, or action environment is
imported.

## Compatibility classification

**Exact:** admitted joined command conversion and diagnostics; both strict
option names and boolean/no-form behavior; env set/inherit/unset conversion;
last-operation-wins effective values; target-to-Exec environment rewrite;
strict/non-strict shell-environment construction for every modeled Host OS;
default `enable_runfiles=auto` and absent shell option; fixed/inherited split;
`use_default_shell_env` composition and explicit action override; canonical
map/set equality.

**Slug-native:** valid-Unicode environment strings, Rust Host OS and server-
environment observation, compact retained layout, configuration canonical
bytes/projection, and Rust allocation/latching mechanics. Map/set order is an
internal canonical order, not a Bazel display-order promise.

**Unsupported/deferred:** Windows `BAZEL_SH` values containing an 8.3 short-path
candidate, because Bazel's `WindowsPathOperations.getLongPath` observes the
Host filesystem; separate-token native flag values; explicit command
mutation of `enable_runfiles` or `shell_executable`; action registration;
client-inherited value resolution and its execution invalidation; exact Bazel
configuration checksum, output path and ActionKey; REAPI Command/Action
digests; callbacks, C++ rules, and any ruleset-specific path. A configuration
without the retained Host observation or with an explicitly mutated deferred
option fails before environment publication.

## Decisions and non-decisions

### Retained Host fact

Add an `ActionEnvironmentHost` value to
`slug_configuration_v2::native::host`:

```text
ActionEnvironmentHost {
  os: Linux | Windows | Macos | Freebsd | Openbsd | Unknown,
  bazel_sh: Option<CompactString>,
  path: Option<CompactString>,
  system_root: Option<CompactString>,
}
```

It is one immutable, `Arc`-backed, `Dupe` and `Allocative` value. Non-Windows
observations retain only OS because Bazel's admitted environment algorithm
does not consume the three server variables there. Windows retains presence or
valid-Unicode value for all three. Public constructors make irrelevant
non-Windows variables unrepresentable.

`HostConversionInputs::new` remains source-compatible and produces no action
Host fact. A named `with_action_environment_host` projection returns a new
shared input carrier. This avoids editing unrelated test constructors while
making absence explicit. `SlugConfiguration::new_default` carries the optional
fact into `SlugConfigurationData`; `configured_action_environment` rejects its
absence. Every configuration mutation and Target/Exec transform preserves the
same fact.

The optional Host field participates in structural equality and canonical
bytes under one new tagged root field when present. Keep the existing
`slugcfg-v2` domain/version: it names the clean-restart identity domain, there
is no decoder or stability obligation, and omission preserves test-only
configurations which cannot publish an action environment. Do not hash a Host
fact separately or use a digest in place of structural bytes.

### Process observation owner

Extend the existing runtime-owned `ProcessHostOwner`, not DICE, with one
`ClassCell<ActionEnvironmentHost>`. Its `ProcessHostSource` gains typed
`BazelSh`, `Path`, and `SystemRoot` properties. The cell first consumes the
already-latched OS, then reads the three properties exactly once and only on
Windows. Production uses `std::env::var`, preserving the approved valid-
Unicode Rust Host boundary; absence and non-Unicode both project as absent.

`default_configuration_inputs` installs the latched fact into the existing
request input. Concurrent requests share one initialization or one latched
failure. No action/configuration code reads `std::env`, no DICE compute reads
ambient state, and no second Host owner or key is added.

### Canonical configured environment

Add a focused `native/action_environment.rs` module because
`native/configuration.rs` already exceeds the 2,000-line split trigger. It owns:

```text
CanonicalStringMap(Arc<[(CompactString, CompactString)]>)
CanonicalStringSet(Arc<[CompactString]>)
RetainedActionEnvironment { fixed, inherited }
```

Construction uses bounded temporary ordered maps/sets, then publishes only
sorted unique slices. Duplicate input keys keep the last value. All three
types are structural, hashable, ordered, `Dupe`, and `Allocative`, expose
borrowed iteration/lookup, and retain no process-global interner or mutable
container.

`SlugConfiguration::configured_action_environment` reads only the sole option
vector and retained Host fact. It creates Bazel's base PATH/
`LD_LIBRARY_PATH`, applies action-env rows in option order, then applies
runfiles state. `RetainedActionEnvironment::for_action` applies the
`use_default_shell_env` branch and action-map precedence. It never resolves
inherited values.

Windows shell-derived PATH calculation is a private configuration helper over
the retained Host fact. It must reproduce the pinned `PathFragment` parent,
`usr/bin`/`bin`, drive-letter, separator and `..` normalization cases used by
`pathOrDefault`. If any segment matches Bazel's 8.3 short-path candidate grammar
(`WindowsPathOperations.isShortPath`), fail before publication: resolving it
would require an additional Host-filesystem observation outside this packet.
Later public `NormalizedBazelPath` may reuse or promote the normalizer, but this
packet may not publish an action path type.

### Command and Exec behavior

Add three typed `NativeCommandOption` variants. Both strict spellings map to
one canonical option. Generalize boolean `no` handling only across the closed
typed native set; joined values on a no-form reject and non-boolean no-forms
remain unrecognized. `action_env` and `host_action_env` require the existing
joined-value command path.

The native option vector continues to preserve Bazel's normalized list
behavior. Effective environment construction applies rows by name so set,
inherit, unset, and later reset are last-operation-wins. The retained
environment's equality does not inherit native-list insertion order.

`to_exec_for_platform` copies the complete retained `host_action_env` value to
`action_env` in the same loop that already copies `host_compilation_mode` and
sets the platform. The test-only platformless `to_exec` keeps its established
kind-only contract and is not claimed as Bazel's platform Exec transition.

## Request, revision, and memory behavior

Command overlays remain immutable request inputs. Converted option scratch is
request/phase-local and publishes atomically with the complete Starlark option
map. Invalid earlier or later occurrences publish no partial configuration.

`ProcessHostOwner` is service/container state released with the runtime.
`ActionEnvironmentHost`, configuration option slices, canonical bytes, and the
configured environment are immutable graph/configuration-retained state.
Ordered map/set builders and action composition are bounded phase scratch.
There is no async transfer, cache, eviction policy, callback, filesystem read,
lock held across DICE, or client-environment snapshot in this packet.

Configuration equality cutoff sees every retained Host value and option row.
Environment equality sees only canonical fixed/inherited semantics. A/B/A
tests must prove configuration restoration for option and Host fact changes,
and repeated/concurrent default-input requests must prove the environment
properties are not reread.

## Allowlist and caps

Production files:

- `app/slug_configuration_v2/src/command.rs`
- `app/slug_configuration_v2/src/native/action_environment.rs` (new)
- `app/slug_configuration_v2/src/native/configuration.rs`
- `app/slug_configuration_v2/src/native/host.rs`
- `app/slug_configuration_v2/src/native/mod.rs`
- `app/slug_configuration_v2/src/lib.rs`
- `app/slug_commands_v2/src/common.rs`
- `app/slug_core_v2/src/runtime/process_host.rs`

Proof files:

- colocated tests in the new environment module, `native/host.rs`,
  `native/configuration.rs`, `commands_v2/common.rs`, and `process_host.rs`;
- `app/slug_configuration_v2/src/native/tests.rs` for command/Exec A/B/A;
- `app/slug_server_v2/src/tests.rs` only if the typed command enum changes an
  existing transport round-trip assertion; and
- one existing exact output assertion in
  `app/slug_core_v2/src/runtime/tests/build_command_tests.rs` only if the new
  production Host field lawfully changes its Slug-native projection.

Caps are 600 production, 620 proof, and 1,140 total net added Rust lines. The
line-level implementation preflight corrected the production allowance from
520 to 600: the accepted architecture ceiling was already 600, while the
implemented split is 597 production lines and remains below the unchanged
proof and total caps. No
Cargo dependency, lockfile, DICE key, wire type, fixture, Java/JVM artifact,
global cache/interner, action/executor file, or unrelated dirty file is
allowed. `configuration.rs` may receive only carrier preservation, encoding,
projection delegation and Exec-copy edits; new environment algorithms belong
in the split module.

## Validation

Run serially:

1. `cargo fmt --all -- --check` after formatting the allowlisted Rust.
2. `cargo test -p slug_configuration_v2 --no-fail-fast`.
3. `cargo test -p slug_commands_v2 --no-fail-fast`.
4. Focused `slug_core_v2` process-Host and changed-output tests, then
   `cargo test -p slug_core_v2 --lib --no-fail-fast` if the focused rows pass.
5. `cargo test -p slug_analysis_v2 --no-fail-fast` as the direct Exec-
   transition dependent.
6. `cargo test -p slug_server_v2 --no-fail-fast` if its proof file changes;
   otherwise `cargo check -p slug_server_v2` as the command transport
   dependent.
7. `scripts/v2_archive_status.sh`, packet allowlist/cap/isolation checks, and
   `git diff --check`.

Required discriminators are: set/inherit/unset/empty value and reset; repeated
keys; invalid empty/bare-equals; both strict names and no-forms; strict and
non-strict branches on all Host OS classes; Windows BAZEL_SH/PATH/SYSTEMROOT
presence/absence and admitted path normalization plus short-path rejection;
runfiles auto Windows/non-Windows;
action false/true composition and inherited-name override removal; reordered
map/set equality; target versus Exec host environment; missing/deferred-state
failure; Host and option C0/C1/C0; one-read concurrent Host latching;
`Allocative`/`Dupe`; and absence of retained mutable standard maps.

## Review and stop conditions

Independent review is mandatory because this changes cross-crate retained
configuration identity and process observation lifetime. The reviewer must
answer whether the implementation packet:

- has one natural option/Host owner with no ambient action/DICE read;
- preserves exact Bazel option, default, Exec and composition behavior;
- makes published map/set identity insertion-order-independent;
- keeps client values and execution invalidation deferred;
- uses compact immutable retained storage with explicit memory lifetime;
- preserves configuration identity across every transform; and
- remains generic, with no rules_cc, `cc_common`, parser, `set`, C++ or action
  special case.

`REPLAN` before Rust if exact Windows construction needs a second process read,
a public path type outside this packet, a new DICE key, or more than the caps.
During implementation, stop and replan for a second option/configuration
store, retained mutable map, missing Host fact fallback, insertion-order
semantic environment, client values in configuration, action/execution work,
unallowlisted public breakage, or a second material contract correction.

Terminal `ACCEPT` authorizes only
`WP-6-7A-fdo-basic-args-run-symlink-implementation-r1` authoring. It does not
authorize Rust for that successor until its own bounded packet is ready.

Terminal implementation review returned `ACCEPT`: the sole structural option
and process-latched Host owners, exact Bazel composition and Exec behavior,
canonical compact identity, Windows fail-closed boundary, generic scope,
allowlist, and 597/489/1,086 Rust-line caps all passed. Focused validation
passed 58 configuration, 27 command, 11 process-Host, 102 analysis tests, the
updated FileWrite action-token baseline, server compilation, formatting, and
diff checks. Three unrelated core failures reproduce unchanged at base
`94fd24e9f`; the archive checker retains only its three known documentation-
path failures.
