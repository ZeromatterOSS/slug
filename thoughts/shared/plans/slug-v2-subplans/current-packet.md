# Current Slug V2 Work Packet

Packet: WP-7-153-m7a-alias-direct-constraint-select-r1
Status: accepted; M7A remains partial

## Outcome and compatibility

Advance the rebuilt authentic Rust-toolchain `cquery //:probe --output=label`
past `@@platforms//os:none is not a config_setting`. Pinned rules_rust
0.73.0 `ffi/cc/allocator_library/BUILD.bazel:3-22` uses `none`, `wasi`, and
`windows` constraint values as direct keys in native `alias.actual`'s
`select()`. Under the Linux target platform all three must miss and select
the default; under a selected matching platform a direct branch must win.
This packet admits direct constraint-value keys for configured native alias
selection only. It does not admit the Rust-toolchain or configured C++ closure.

For that bounded alias owner, configured target-platform matching, explicit or
default constraint values, selected branch and invalid-key rejection are
`exact` to Bazel 9.2. Structural configuration/DICE identity, Rust valid-
Unicode diagnostics and path tokens remain `Slug-native`. Direct constraint
keys for other owner kinds, mixed direct-constraint/config_setting multi-match,
nondefault Apple/Xcode, full toolchain
and configured C++ closure are `unsupported/deferred` and fail closed rather
than claiming general `select()` parity.

## Pinned source and semantic owner

Bazel 9.2 `ConfigConditions.java:59-88` accepts `ConstraintValueInfo` only
when the owning rule has a target platform. `ConfigurableAttributesTest.java:
1660-1790` proves direct matching, rejection without toolchain resolution,
and alias-with-select eligibility. `ConstraintValueInfo.java:65-96` compares
the requested value to the target platform's effective value, including a
setting default through `ConstraintCollection.get`. `Alias.java:80-94`
enables toolchain resolution when alias has selector keys.
`ConfigMatchingProvider.java:199-229` gives direct constraint providers empty
refinement criteria.
The authentic selector comes from pinned rules_rust `allocator_library` via
`rust_host_tools`'s `allocator_library=None`, the rule default label flag and
that flag's alias default.

The existing `ConfiguredConditionKey` is the sole DICE owner of condition
truth and its observed package/constraint/platform dependencies. Encode alias
direct-constraint eligibility structurally in that key, so a completed value
cannot be reused for an ineligible owner. Keep the requested selector label
as dependency identity while resolving aliases to the terminal config_setting
or constraint value. Use the existing `ConfiguredTargetPlatformKey`,
`compute_configured_constraint` and `platform_has_actual_constraint`; do not
infer platform from host OS, execution platform, paths or displays. The
prepared selector declaration is attempt-local scratch and must distinguish
config_setting from direct constraint value for refinement. Direct values
have empty provider refinement criteria. Preserve prior config_setting
behavior; reject a mixed direct-constraint/config_setting multi-match before
choosing a potentially wrong branch. Two matching direct keys remain peers:
equal selected values may pass, differing values are ambiguous.

Only native alias callers supply eligibility, and only when their `actual`
has selector keys. A nested alias's `actual` selector obtains eligibility
from that nested alias independently; it does not inherit the outer condition
owner's gate. An ineligible outer owner still rejects a direct terminal
constraint key reached through an alias. Other current callers are
ineligible in this packet even if Bazel may allow some in a later
owner-specific packet. This is a bounded
fail-closed subset, not a statement that Bazel disallows them. Alias-chain
Need/error/cycle behavior, target/exec configuration, package tracking and
source A/B/A remain with the existing observed DICE graph. No lock across a
DICE compute, new cache, interner, second platform store, Host read or donor
import. The key is DICE-retained and `Allocative`; selector declarations and
normalization are analysis-attempt scratch. Existing Buck2-derived `SmallMap`,
`SmallSet`, Arc and structural equality patterns are retained, with no new
hot-path collection. `docs/developers/dice.md` and retained Buck2 DICE key
ownership remain the architecture reference.

## Evidence, validation and scope

Reuse the pinned Bazel source/tests above. Add a focused Slug synthetic alias
test with sibling OS values, Linux select() default and a selected non-Linux
branch. Add an omitted-platform-setting direct-key match through
`default_constraint_value`, then an explicit different value overriding that
setting default. Cover target and exec configurations, alias-to-constraint,
invalid target kind, ineligible key, Need/cycle, and mandatory same-DICE
target-platform and source A/B/A restoration in the existing
configured-analysis fixtures. A separate public fixture is only
needed if an existing focused fixture cannot show the owner gate; any new
fixture must follow the Stage 1 provenance contract. Preserve existing
config_setting and alias-selector tests. Rebuild `slug_cli_v2`, clean stale
`slugd` before/after daemon-sensitive work, then run one bounded unchanged
authentic `//:probe` cquery and record its first new labelled stop or success.
Use Bazel RC discovery without reading or copying `~/.bazelrc`; optional
BuildBuddy transport does not replace local NativeLink real-transport gates.
Serialize Cargo, run focused tests, `git diff --check`,
`scripts/v2_archive_status.sh` and `scripts/v2_plan_status.py`. Independent
DESIGN review gates implementation; FINAL review gates acceptance.

Allowlist: this manifest and canonical Current; `app/slug_analysis_v2/src/
dice.rs`, `app/slug_analysis_v2/src/configured_attribute.rs` and focused
`app/slug_analysis_v2/tests/` files; compact Stage 6/9 owner notes after
acceptance. `dice/filegroup.rs` is allowlisted only for adding an explicit
ineligible call argument. No Loading, Configuration, Bzlmod, Core, Build API,
remote-cache/action or repository-generator change. The large `dice.rs`
remains the cohesive configured-analysis owner for this bounded change; no
broader split or performance claim. `REPLAN` if an alias cannot obtain the
target platform through existing structural owners, exact alias eligibility
requires new Loading metadata, or preserving mixed-selector safety needs a
new general refinement owner. Compiler/test selector corrections inside this
contract are not `REPLAN`.

## Immediate predecessor

WP852 at `570d14118` accepted default MACOS `single_arch_platform`. Its
isolated generated rules_cc cquery passed, while broader Rust-toolchain
cquery first stopped at `@@platforms//os:none is not a config_setting`.

## Acceptance receipt

Independent DESIGN and FINAL reviews: ACCEPT. The existing condition key now
includes alias direct-constraint eligibility in structural Eq/Hash/Allocative;
requested selector labels remain dependency identity, and existing observed
constraint/target-platform owners determine effective matching. Attempt-local
condition kinds preserve config_setting specialization, direct peer equality
and ambiguity, and fail closed on mixed matching provider kinds.

Pinned Bazel 9.2 source/tests are the source regression. JSON `--no-run`
compilation and batched exact-name preflight succeeded; 14 selected Analysis
integration tests pass (0.21 seconds). They cover target/exec and omitted-setting
defaults, explicit overrides, platform/source/default-source A/B/A, both owner
cache orders, nested alias eligibility and terminal refusal, peer equal-value
selection and ambiguity, mixed-match rejection, wrong kinds, Need recovery and
literal/nested selector cycles. Protected config_setting, condition-alias,
external-condition invalidation, specialization and alias-Need controls pass.
The final test compile took 8.69 seconds; the final current-source offline
`cargo build -p slug_cli_v2` passed in 0.11 seconds. Continuation validation and
review occupied approximately 14 minutes from the first recorded compile;
the inherited unfinished candidate's earlier implementation time is unmeasured.

The unchanged authentic fixture `/tmp/slug-wp797-authentic-2YQmSt` retains its
BUILD, MODULE and lockfile hashes. Rebuilt one-shot
`cquery //:probe --output=label` exits 2 at the new labelled stop
`@@bazel_tools//tools/def_parser`: missing BUILD file. The earlier
`@@platforms//os:none is not a config_setting` error is absent. The replay took
approximately 88 seconds based on output timestamps, inside its supervised
180-second bound; this is an advancement receipt, not a successful toolchain
query. Sandbox registry denial and 30/60-second preparation timeouts did not
prove the gate. A 20-second diagnostic showed active CPU/network preparation;
direct registry access was verified and the final unchanged network-enabled
replay established the terminal. No credential contents were read or copied.
No stale slugd was present before/after. Raw compile, preflight, runtime and
replay receipts remain under `target/wp853/`.

Rustfmt, `git diff --check`, archive and plan-status checks pass. Full
Rust-toolchain/configured-C++ closure, the new bazel_tools package boundary,
other direct-key owners and mixed matching specialization remain deferred.
