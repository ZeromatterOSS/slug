# WP899 bounded child-accounting attribution

Status: DESIGN checkpoint; full WP899 unaccepted.
Baseline: 4a324c4fc, following preserved single-payload component606eb7d29.
Compatibility: Slug-native Linux diagnostic supervisor; no Bazel semantic change.
Inherit [global9 terminal](wp899-child-accounting-frontier-2026-10-03.json),
current packet and orchestration validation/resource rules.

Observable result: an accounting-loss receipt distinguishes the selected child
census operation, caller context and owner class using fixed scalar detail.
Global9 emitted zero Slug records and cannot retrospectively establish its exact
operation/errno. Source audit shows `children()` catches OSError/ValueError from
all-thread traversal and discards any partial result. Existing persistent-thread,
FD churn, descendant/adoption and cleanup tests do not discriminate thread exit
between enumeration and children-file read.

## Decision and ownership

Root implements only attribution in scripts/v2_source_admission_supervisor.py and
its existing tests. children() remains the producer; the first refusal latch owns
its matching detail through terminal receipt. Add finite ChildSite codes for task
open, iteration-next, children read, PID parse and iterator close; ChildOrigin for
OS versus invalid metadata, optional signed32 errno; ChildContext for unknown,
isolation preflight, active discovery, confirmed-stopped discovery, terminal
discovery and final join; ChildOwner for unknown, supervisor, root and descendant.
No PID/TID/path/list contents/exception text/environment is retained or emitted.
Caller/owner codes name call sites and owned-process roles, not causal inference.

AccountingError accepts either existing DiskFailure or new ChildFailure, with
exact type validation. First refuse stores only matching scalar detail, separately
projected to existing disk fields and new child_failure_* fields; unrelated or
later failures cannot replace or manufacture first detail. Keep disk byte/enum
meanings unchanged; absent child detail emits zero codes/None errno. Setup CLI
still emits its existing generic refusal, with no expanded setup reporting.

Wrap only originally caught OSError/ValueError; iterator StopIteration terminates
normally, and unrelated parser exception domains remain unchanged. Helpers raise
outside exception handlers so native exception/cause/context is not retained.
Open iterator with its existing context manager, exhaust all rows, read/parse every
listed thread, select caught body errors as scalar detail without raising, then
let context exit close. Mark close in an inner finally; select any caught close
error at the outer boundary. Raise selected AccountingError only after all handlers
and iterator context have exited. Unrelated body exceptions propagate unchanged
when close succeeds; caught close failure retains its original superseding precedence.
Thus both caught body and close failures carry no native cause/context.
all errors remain fatal, including close error. An error during close may supersede
an earlier operation just as the original with-context does; detail must identify
the actually selected close failure. No partial census, skipping ENOENT, leader-only
scan, retry, observation cache, extra system scan or changed signal/clock is allowed.

The detail is a small nonsemantic supervisor-owned Python value. Existing result
set memory and DICE/source/runtime ownership remain unchanged. No Rust/donor or
Buck2 representation change. Native exception traceback locals are not a public
receipt and are not persisted. No Windows work selected.

## Validation and scope

Allowlist: the two existing Python files, this contract, a compact acceptance
receipt, and current/canonical scheduling status. No actual workload, Cargo,
network, source semantics or adjacent supervisor repair in this component.
Estimate net90–140 production/180–260 test lines. Existing supervise() exceeds150
lines, but narrow caller tags/first-detail projection stay at its natural owner;
do not refactor lifecycle while adding evidence. Existing file below2000 lines.

Root owns serial local Linux tests, capped30s TERM/KILL3; no concurrency with
Rust native fixtures. No Cargo compile needed for Python-only change. Use a fresh
Python process matching actual supervisor interpreter/usr/bin/python3.12. Before
execution check no overlapping supervised fixture/Slug/Cargo process. Record exact
source hashes, names/pass count, elapsed, failures. Existing25 protected supervisor
controls are reused when unchanged; run whole current small supervisor suite once
because AccountingError/refusal projection and discovery caller tags cross them.

New discriminators:
- task-open OS failure; iterator-next failure after a valid row; malformed PID data;
  all discard partial result and carry correct site/context/owner/origin/errno;
- iterator close after successful traversal and superseding a caught read failure;
  absent native cause/context in both; unrelated body exception with successful
  close remains unchanged, and caught close failure supersedes it as before;
- exact successful union across multiple task rows; normal iterator exhaustion,
  strict original int parser behavior; invalid diagnostic scalar types rejected;
- redacted exception/path/contents: only fixed reason and validated scalar detail,
  no native cause/context or diagnostic exception text;
- one ordinary local cooperating thread: enumerate its real task entry, trigger
  retirement and actual worker join with bounded pipe handshake, confirm task path
  disappeared, then perform real children read. Patch only test-boundary reader,
  no production hook. Keep actual child join on every assertion path. This proves
  controlled children-read ENOENT, never global9 attribution;
- inject first child accounting detail into actual normal supervisor discovery,
  then later disk cleanup detail; first survives TERM/join/stage cleanup.
  Disk-first/child-later proves only disk projection survives.
  Inject unrelated first refusal and later child detail to prove absent projection.
  Preserve existing disk first-detail gates and descendant/adoption controls.

Independent DESIGN before source edits; three reviewers cover operation/exception
boundary, caller/latch/lifecycle and evidence/discriminator scope. Independent FINAL
checks actual diff and executed evidence. Resource/signal policy and all existing
caps remain unchanged: hard4GiBAS, verified parent-death guard, sampled4GiBRSS/any
swap/descendant/accountingloss,6GiB disk,1MiB log,27+3+3s,20ms poll,100ms census.
REPLAN only if attribution requires a policy/lifecycle change or tests show a
contradictory contract. A later actual run needs a separate reviewed freeze after
this component; no identical retry, source-fit/M7A/fullWP899/40GB claim follows.
