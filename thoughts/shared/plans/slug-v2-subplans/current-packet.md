# Current Slug V2 Packet

Packet: `WP-5-m1-direct-local-trusted-nonregistry-evaluator-adapter`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: private one-file trusted direct-nonregistry evaluator correction
Evidence: accepted support-gated direct-local preparation in `f2b626f2`;
pinned Bazel 9.2 `ModuleFileFunction`, `ModuleThreadContext`, and
`InterimModule` defaults/validation/include-map behavior; accepted empty-key
nonregistry identity evidence; and the existing private supplied-file evaluator
regressions. Add no oracle or fixture.

Edit exactly `app/slug_bzlmod_v2/src/module_eval.rs`. The formatted net addition
may not exceed **190 production lines, 430 test lines, or 620 total lines**.
Add one crate-private trusted direct-nonregistry adapter over the existing
private evaluator machinery:

- `DirectNonregistryIncludeFile<'a>` retaining raw label, logical file ID, and
  borrowed source bytes;
- `DirectNonregistryEvaluationError`, with distinct preparation, execution,
  finalization, declared-name mismatch, and declared-version mismatch variants;
  and
- `evaluate_direct_nonregistry_module_closure_with_events`, returning the
  evaluated module or typed error plus a marker-conditional local `EventBatch`.

Keep the expected `NonrootModuleKey` separate from initially empty declaration
state. Construct `NonrootModuleBuilder` with the expected key but empty declared
name, declared version, and repo name. The `module()` directive alone populates
those fields and retains its existing repo-name default from the declared name.
After successful execution and finalization, validate the declared name first.
Only if it matches and the expected version is nonempty may declared-version
validation run. An empty expected version skips that comparison and preserves
the normalized declared version in the successful output. Error carriers retain
the expected key and declared field needed for exact later diagnostics.

The adapter consumes only an already-supported, fully acquired closure. Insert
every ordered include occurrence into the execution map and prepare every
occurrence. Repeated raw labels are **last occurrence wins** in that map: pinned
Bazel compiles every horizon occurrence and unconditionally calls
`includeLabelToCompiledModuleFile.put(raw_label, compiled_file)`, so later
values replace earlier ones before execution. Every inline `include()` call
still executes the selected prepared program, including repeated calls. Do not
apply the old supplied-file duplicate or unreachable rejection seam to the
trusted closure. Preserve those checks unchanged for the existing strict test
adapter.

Parse and prepare the root plus every supplied occurrence before executing the
root. Any parser, restricted-syntax, scope, identifier, or prepare failure in
the full supplied closure therefore precedes every directive effect. Set the
prepared-program table once, execute the root, preserve nested logical source
locations and hidden evaluator roots, and finalize only after evaluation. Keep
the existing force-GC test coverage without making GC part of the production
adapter contract.

Nonregistry print is allowed. When event capture is requested, install the
existing recording print handler and return its ordered batch, including prints
emitted before an execution or post-execution identity failure. When capture is
not requested, leave the evaluator's direct/default print behavior installed.
Do not use `RejectPrint` for this adapter. The existing strict private test seam
may retain its rejection behavior; registry no-op print and DICE ownership are
outside this packet.

Tests must discriminate initially empty declarations and omitted `module()`;
name mismatch before simultaneous version mismatch; the retained nonempty-key
version mismatch seam; empty expected-version skip plus preserved declared
version; repo-name defaulting; parse/prepare of every occurrence before root
effects; duplicate raw-label last-wins selection while every occurrence is
prepared; repeated inline execution; nested locations; uncaptured print
success; ordered captured root/fragment prints; and print-before-failure batch
retention. Keep the existing strict duplicate, unreachable, `RejectPrint`, GC,
deferred-value, and directive regressions passing.

Stops: no second file, DICE key, source-preparation consumer, public export,
registry/MVS/contextual mapping, public unsupported-cycle publication, event
storage or replay, direct IO, fixture/oracle, dependency, or cap breach. Do not
weaken or remove the existing strict evaluator seam merely to fit the adapter.
`REPLAN` if exact behavior requires a second production file or a public type.
Run the focused nonroot evaluator tests, the owning library tests, formatting,
GNU-Windows no-run, archive/scope/cap/diff gates, and independent latest-diff
review; do not run Bazel.
