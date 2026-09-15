# Current Slug V2 Work Packet

Packet: WP-4-7A-repository-generated-path-r1
Status: generated-path design accepted; implementation ready

## Observable result

The independently accepted repository-effect diagnostic at `fde43d5cf` exposed
the first unchanged CLI selector's typed terminal after 10.95 seconds:
toolchain registration row 8, canonical repository
`rules_java++toolchains+local_jdk`, repository invocation ordinal 6,
`repository_ctx.path argument must be a Label`. Admit the exact repository-local
string/path surface used by that authenticated local-JDK branch, then run that
one selector once to select the next owner. The remaining nine selectors and F3
stay stopped. The combined R2/group/computed-default/alias/initializer stack is
still unaccepted and may not merge or push.

Authenticated rules_java 9.1.0 source
`toolchains/local_java_repository.bzl`, SHA-256
`9213c5cdd42bc131ec32f5732ea78e1c839a137c146b6fe1cfea61adb4d9d380`,
uses an empty `java_home`, observes no `javac` in the selected host `PATH`, then
calls `repository_ctx.path("./nosystemjdk")`. It derives
`get_child("bin").get_child("java")`, reads `.exists`, and takes the absent-Java
branch that writes the existing error BUILD plan. No repository payload or
fixture byte is missing.

## Compatibility and ownership

Pinned Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a`,
`StarlarkBaseExternalContext.java:1544-1578`, admits string, Label or path. A
relative string is rooted in the repository working directory and a path input
is returned unchanged. `StarlarkPath.java:57-77,129-160,186-198` makes paths
immutable/hashable, joins string children lexically, exposes existence without
implicitly watching, and shares path bytes across equality, hash, string and
repr.

`repository_rule_context.rs` remains the sole evaluator/value owner. Extend its
existing `RepositoryStarlarkPath` with a private generated-root-relative value.
`repository_ctx.path` keeps the existing Label demand/retry behavior, returns an
existing path value unchanged, and converts an admitted relative string into a
generated value. Generated values retain the canonical repository name and a
normalized relative component sequence; `get_child` appends relative string
components and preserves that provenance. Equality and hash include the value
kind, canonical repository and normalized components.

One invocation-local `Arc<Mutex<...>>` generated-path state is shared by the
repository context and its generated path values. It starts with the repository
root present. A single context helper first pushes each valid `file` or
`template` effect into the existing plan builder, then records that authored
file and every generated-directory prefix as present. A failed push records
nothing. `.exists` reads only this attempt state: the root, `WORKSPACE` and its
parent are present after the authentic first write, while the disjoint
`nosystemjdk/bin/java` path remains absent. The state dies with the evaluator
attempt and is never copied into the returned plan.

Slug has no repository working directory until the immutable effect plan is
materialized. Generated-path `str`/`repr` therefore use the stable Slug-native
logical spelling `@@<canonical-repo>//<normalized-path>` (and
`@@<canonical-repo>//` for the root), never a guessed physical path. For the
selected generated Java path, `.exists` is exactly false and adds no watch or
observation. Existing Label and `which()` physical paths keep their byte-based
identity and do not gain `.exists`; their statically visible `get_child` method
returns a typed attempt error without changing Label/which provenance. Absolute
strings, root escape, physical generated-root prediction, filesystem methods,
`basename`, `dirname`, `is_dir`, `realpath`, `readdir`, reads of generated paths,
and existence or child projection for Label/which paths remain unsupported.

String normalization accepts `.`, repeated separators, `.` components and
interior `..` only when they do not escape the generated root. Reject empty,
NUL, backslash, absolute/drive-like, escaping and over-limit input before a path
value is allocated. These string restrictions are explicitly Slug-native;
Bazel's `workingDirectory.getRelative` accepts a broader host-path surface.
Bound a normalized path to 256 components and 4,096 bytes.
`get_child` accepts zero or more strings, applies the same normalization to the
combined path, returns the same value for zero arguments, and rejects nonstrings
and limit/escape failures.
The current `LabelPathArgument` carrier may remain private but its terminal text
must name the admitted Label/string/path shapes.

No generated path, evaluator value or physical root enters the retained effect,
plan, key, certificate, cache or DICE graph. No source, environment, host input,
effect ordering, print capture, retry, observation, publication or cancellation
boundary changes.

## Scope, caps and stops

Production may edit only
`app/slug_loading_v2/src/repository_rule_context.rs` and the terminal wording in
`app/slug_loading_v2/src/module_extension_repository_file_effect.rs`. Proof may
edit tests colocated in those two files. Scheduling/status sections in the
canonical plan, this manifest, Stage 4, bootstrap readiness and the configured
CLI ledger may change. No Bzlmod, materializer, source-preparation, fixture,
registry, CLI, analysis, query or execution file may change.

From `fde43d5cf`, allow 160 gross production and 200 gross proof Rust lines, 360
total, without deletion credit. Physical caps are 2,340 lines for
`repository_rule_context.rs` and 4,350 for the effect owner. The wider proof
allowance covers attempt-state publication ordering, prior effects, path-value
identity and provenance-conditional method behavior. Replan for another
production owner, physical-path/materializer coupling, retained-state or DICE
change, broader filesystem observation, absolute paths, cap breach or another
authentic prerequisite.

## Discriminating evidence and gates

Add and preflight exact selector
`repository_context_generated_relative_paths_are_lexical_absent_and_idempotent`.
It proves `.`, `./nosystemjdk`, repeated separators, safe parent normalization,
one/multiple-child joins, path idempotence, exact logical str/repr, equality,
hash/map lookup, and Rust `Value::ptr_eq` identity for a path input. It proves
root, authored `WORKSPACE` and authored directory-prefix presence alongside an
absent sibling and the selected Java path's absence after `WORKSPACE`, plus
unchanged Label-path behavior and typed physical-child rejection. It
rejects empty, absolute, drive-like, backslash, NUL, root escape, nonstring
children, 257 components and 4,097 bytes, while zero children are idempotent. A
focused effect-owner selector runs
the authenticated local-JDK path/get-child/exists fragment, proves the absent
branch writes only its expected WORKSPACE/BUILD plan and exact terminal-attempt
prints, and preserves invalid-type atomic failure. No fixture is added.

Compile within 60 seconds. Run both exact selectors and the inherited Label-path
selector under the 12-second/15-second limits, full loading unit and integration
harnesses, query compile coverage, format/diff/cap checks and final independent
review. After acceptance, commit the checkpoint, rebuild and preflight the
unchanged CLI harness, then run only
`configured_action_conflicts::one_shot_build_conflict_is_atomic_and_recovers`.
Record its newly exposed typed terminal and stop. Do not run the other nine CLI
selectors or F3 until a reviewed successor and all joint gates pass.
