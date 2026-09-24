# Current Slug V2 Work Packet

Packet: WP-7-141-m7a-gcc-template-prefix-r1
Status: ACCEPTED; M7A remains partial

## Outcome and compatibility

Advance the authentic Rust-toolchain `cquery //:probe --output=label` past
the first post-WP840 Core stop, `generated GCC probe prefix has an
unexpected effect`, then report its first new stop. Admit only the
source-ordered, generated three-file template prefix before the already
guarded Linux GCC linker-discovery/option probes. Do not claim linker
success, full rules_cc `local_config_cc`, root C++ toolchain or M7A closure.

The prior WP839 exact source-ordered GCC argv/result slice and WP840 exact
Unix `which` lookup slice stay exact; their Rust Host, sandbox/runtime
and finite-cap divergences stay **Slug-native**. The new prefix permits
exact ordered destination/type/mode shape, while rendered file bytes are
Loading-owned and structurally equal/invalidated, not Core-attested to a
template-source label. That shape-only authorization and rendered-byte
caps are **Slug-native**, never byte-for-byte Bazel parity. The
`HOMEBREW_RUBY_PATH`-present rules_cc template branch, unrelated
repository templates/commands, later generated BUILD/path forms,
Windows and full C++ toolchain remain **unsupported/deferred**. Preserve
the already accepted first two GCC `-v` probes even when that variable is
present; fail closed before the first newly admitted template could retain
secret-bearing rendered bytes.

## Source basis and exact boundary

Pinned rules_cc 0.2.17
`cc/private/toolchain/unix_cc_configure.bzl:362-372,430-495` authors,
in order, links `cc_toolchain_config.bzl` and
`armeabi_cc_toolchain_config.bzl`, executable
`tools/cpp/empty.cc`, then executable templates
`validate_static_library.sh`, `cc_wrapper.sh` and
`deps_scanner_wrapper.sh` before `_find_linker_path(..., "lld")`.
`_get_tool_paths` at lines 68-87 always gives `nm` and `c++filt` keys,
making the first template conditional true on this admitted path.
The later GCC linker argv at lines 166-190 are already parsed by Core's
`ProbeForm`; the current failure is only `repository_process.rs:286-337`
rejecting a trailing effect. Pinned Bazel 9.2
`StarlarkRepositoryContext.java:259-358` owns literal Latin-1 template
replacement and executable default. Loading already resolves the source
Label and bytes as DICE-observed facts and records rendered bytes as
ordered File effects (`repository_rule_context.rs:945-994`); Core receives
only those rendered path/content/mode fields and cannot prove their
template-source provenance.

Pinned `unix_cc_configure.bzl:237-253,446-470` calls `get_env` for two
wrappers. When `HOMEBREW_RUBY_PATH` exists, it can embed almost every raw
environment entry in their content. `cc_configure.bzl:114-150` declares
that variable for `cc_autoconf`. The selected rules_cc owner must check
presence through the already tracked declared-environment frontier and
fail with a static, redacted unsupported diagnostic no later than the
first template call, before `get_env` constructs wrapper substitutions or
Loading retains rendered bytes. Do not inspect, print or persist the
variable's value. A Core defense must reject an expanded prefix when that
variable is present before staging, but must not reject the earlier
accepted three-effect `-v` prefix. Current one-shot host presence was
checked by name only and is absent; any authentic path that needs the
present branch triggers `REPLAN`, not silent environment sanitization.

## Owner, policy and memory

Keep the selected repository-file effect producer, authenticated
repository-rule definition, tracked raw environment, Label-source path
and content observations, ordered `GeneratedRepositoryFileEffectPlan`,
execute request/prefix/call-index/nonce transcript, and Core scratch
materializer as the only semantic owners. Every template source/content,
substitution-dependent environment/tool path and rendered byte must
structurally affect retained plan equality and invalidation. An
unsupported Homebrew presence terminal must depend on that tracked
presence and release invocation scratch; absent→present→absent in the
same DICE must not reuse a successful generated prefix. No new DICE key,
lock, global cache, interner, byte-hash identity or retained data
structure is admitted. Existing Arc-backed file bytes and Allocative
accounting remain in place; Core's staged root and pipe buffers remain
request scratch and are released on abort/cancellation.

Core keeps the sole-file offline form and existing two-link/empty.cc
prefix. For the new form it requires exactly six ordered effects:
those same two links and executable exact-content empty.cc, followed by
three regular File effects with the exact destination names above, each
executable, each at most 64 KiB and together at most 128 KiB; reject
archives, swapped names, missing files, a seventh effect, oversized
content and other modes before staging. These generous Slug-native caps
cover the pinned template raw sizes (1,881/1,398/237 bytes) and
admitted bounded tool-path substitutions without admitting arbitrary
multi-megabyte `ctx.file` effects. The rendered bytes themselves may
vary with observed tool paths; do not hard-code one host's output hash or
pretend Core knows `ctx.template` provenance. The three wrapper files
are inert for the finite guarded absolute `/usr/bin/gcc` argv forms: no
admitted arg names them, child PATH is `/usr/bin:/bin`, and `/exec` stays
read-only. Preserve the entire ordered prefix in request equality and
stage it via the existing symlink-safe writer; do not omit, filter,
synthesize or execute a file effect.

## Proof, validation and scope

Extend `repository-rule-execute-gcc-probes` with one independent
offline repository/query row. Add a tiny local-override source module
with an exported inert `.bzl` file; resolve its external Label and link
it to `cc_toolchain_config.bzl` and
`armeabi_cc_toolchain_config.bzl` in that order. Then write the same
`tools/cpp/empty.cc` and use `repository_ctx.file` to author three inert
executable files at the exact template destinations before the pinned
source-authentic `lld` linker-discovery argv. This makes all six effects
explicit; assert stable result fields but do not require linker success
on hosts without lld. Root-Label
`repository_ctx.template` is currently unsupported, so this row tests
Core's ordered prefix shape/staging, not real template-source provenance.
The existing Loading template source/bytes oracle and authentic rules_cc
smoke cover those separate facts. Preserve the original fixture row
unchanged, including its expected result; verify that adding the local
override does not regress it. Add no copied upstream template asset or
nondiscriminating assertion, and regenerate/independently verify Bazel
9.2 expected results in fresh ignored-RC roots without reading
credentials. A separate fixture is a `REPLAN` fallback only if this
local override changes the original row.

Focused Core tests must cover old sole-file and two-link prefixes,
exact six-effect acceptance, order/name/type/mode/size/additional-effect
negatives, Homebrew presence defense, and inert adversarial wrapper
content under the guarded GCC argv (no script execution, output or host
write). Focused Loading tests must show the selected source-authenticated
Homebrew presence gate happens after the earlier two `-v` requests but
before first template retention; use a sentinel value and assert it is
absent from result/debug/event text. Cover absent→present→absent
restoration, template source/bytes A→B→A and prefix conflict/replay
without child launch. Reuse WP839 runner safety and WP840 `which`
evidence; add only demonstrated gaps. The authentic smoke must pass the
current prefix rejection and reach a source-attributed first new stop.
A different resource or earlier-owner result is diagnostic only, not
acceptance. Record host resources and avoid an identical retry.

Resolve the pinned toolchain and serialize Cargo. Compile Core and
Loading with JSON `--no-run`, preflight exact focused selectors, then
run them; rebuild `slug_cli_v2` before offline Slug and authentic
replay. Clean stale `slugd` before/after daemon-sensitive checks. Run
format, diff, plan-status, archive and fixture hygiene. Independent
DESIGN review must accept the Homebrew gate and inert-file policy before
production edits, then FINAL review the actual diff/evidence. Loading's
large owner file and Core's near-2,000-line runner are size-review
triggers; keep the change at their existing policy seams rather than
splitting semantic ownership during this packet. The new three-file
prefix adds at most 128 KiB of already-Arc-backed retained rendered
bytes per effect plan; no benchmark or memory-reduction claim is made.

Allowlist: this manifest and canonical Current;
`app/slug_loading_v2/src/{module_extension_repository_file_effect,
repository_rule_context}.rs` and colocated tests for the pre-template
gate; `app/slug_core_v2/src/runtime/repository_process.rs` and
colocated tests for the finite prefix guard;
`tests/v2_oracle/fixtures/repository-rule-execute-gcc-probes/{fixture.toml,
expected/oracle.json,workspace/MODULE.bazel,workspace/repo.bzl,
workspace/modules/prefix_source/{MODULE.bazel,BUILD.bazel,source.bzl}}`;
compact Stage 5/9 notes after acceptance. No Bzlmod data-shape, DICE key,
generic template semantics, materializer writer, linker argv grammar,
launcher/sandbox mount, action, cache transport, or other fixture change.

`REPLAN` if the Homebrew-present branch is required by the unchanged
authentic environment, if earlier `-v` probes cannot be preserved while
preventing secret-bearing template retention, if a template source must
become a typed provenance effect, if guarded GCC reads/executes a wrapper
file, if actual bounded outputs exceed the declared caps, or if a new
owner/key/process policy is required. Do not enlarge Core authorization
merely because the next stop is another unadmitted command or effect.

## Acceptance receipt

Design and corrected six-effect fixture contract received independent
ACCEPT. Pinned nightly toolchain resolved. Core JSON `--no-run` passed;
three exact nonignored focused tests passed after preflight, including
the 64-KiB/128-KiB/ordering negatives and inert adversarial wrappers in
the sandbox. Loading JSON `--no-run` passed; six exact nonignored selected
and protected tests passed after preflight, including two preserved `-v`
requests, pre-render Homebrew gate, redacted event text, and same-DICE
absent→present→absent restoration. `cargo build -p slug_cli_v2` passed
(13.68 s) after production edits. Two fresh ignored-RC Bazel 9.2 roots
generated and independently verified the two-row fixture (4.23/4.18 s);
the original expected result is byte-for-byte unchanged. Rebuilt Slug
passed both rows in a fresh run root. Authentic cquery exited 2 after
roughly 68 s at the new source-attributed unsupported `path.dirname`
access, `unix_cc_configure.bzl:500`; the former generated-prefix rejection
did not recur. No `slugd` was left running. Independent FINAL review
ACCEPT followed correction of the inert-wrapper discriminator to the
admitted `lld` argv and unconditional stderr marker; final Core JSON
compile, exact preflight and 3/3 execution passed after that correction.
Packet elapsed was roughly 30 minutes from predecessor acceptance through
FINAL review. No linker or toolchain closure is claimed.

## Immediate predecessor

WP840 at `00ab81869` accepted a source-bounded 832-candidate aggregate
`which` budget, keeping per-call PATH/DICE semantics. Loading 4/4,
fresh two-row Bazel/Slug fixture and independent FINAL review passed.
Authentic cquery cleared the old budget and stopped at Core's generated
GCC-prefix policy; the precise next owner/argv is inferred from pinned
source order, not captured in the generic top-level error. The full
WP840 contract and receipt remain in that commit's `current-packet.md`.
