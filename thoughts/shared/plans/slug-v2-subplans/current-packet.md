# WP1092 external syntax/load synopsis reuse

Packet: WP-7-304-m7a-external-syntax-synopsis-r1
Status: accepted — broader producer frontier trace follows
Baseline: 22fd34ebc187bff450dfbd8ae58dae71cca4f188
Owner: existing Loading external source/parse/evaluation DICE owners.
Compatibility: exact admitted Bazel9 syntax/encoding/load/error/evaluation behavior;
Rust-native incremental reuse Slug-native. Linux priority; Windows minimal/deferred.

Private ExternalBzlSyntaxKey owns the full original module identity plus mode.
Original current source completes first in the same immutable transaction under
its existing module guard; the private synopsis only revalidates that completed
source. No new detector identity, child routing or direct-root/source-wait surface.
Pure syntax metadata/errors are valid; Need/source/outer/cycle failures invalid.
Existing source observations remain current on every parent attempt.

The synopsis shares existing byteArc, moves filename and shares ordered immutable
load names. Retries borrow names; settled APIs receive required independent storage.
No retained AST/CodeMap/full sourceString/cache/global interner/lock. Metadata520B,
key312B versus original304B; filename/load capacities and extended DICE body/history
retention are real costs. Source live credits retire at finish, not with returned
Arc; current scope cumulative budgets and actual Weak graph release stay explicit.
One extra cold settled parse is traded for proven unresolved operation reuse;
CPU/speed improvement remains unproved. Existing scoped interning stays in use.

Accepted source scope: narrow bzl_module.rs, new private external_bzl_syntax.rs,
existing external_bzl_frontier_tests.rs and five-line strict observed dependency
expectation in host_package_load_tests.rs. Independent DESIGN/amendment/source/FINAL
and evidence review;18 current focused passes, direct Loading/Core/CLI production
check and original-profile native-feature Core preparation pass. Format/archive/
diff/plan checks pass. Failed/stale preparations and first fixture batch preserved.

ONE native7.289s, supervisor224.977MiB/self-reported226.035MiB of256, source252;
350 ordered held noninherited FD bindings and66 pins stable, group joined/empty,
collection non-atomic. Captured workspace-matching JAVA_HOME names-only environment
Need is outside current prepared-source admission; no Scope error/timeout latch.
Final root cumulative interval aborted_partial. This prefix proves resource fit
only; full producer/time/wholefit/Rustc/actions/certificates/bootstrap remain open.
All caps unchanged; no identical native retry or preparation workload credit.

[Compact receipt](wp1092-external-syntax-synopsis-2026-10-10.json) owns exact hashes,
gates/caps/reviews/qualifications. Accepted implementation contract and research
remain reachable at this packet's acceptance commit and target/wp1092. Next selection
traces the current normal producer refusal before defining a bounded owner change.
Goal ACTIVE. Preserve allocation discipline and prefer borrow/move/share/scoped
interning whenever owner lifetime and cleanup fit.
