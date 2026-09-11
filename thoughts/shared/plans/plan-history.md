# Plan evidence archive index

## Authority

The September 11, 2026 compaction removes completed chronological instructions
from active scheduling documents. It changes no historical acceptance result,
compatibility class, source/test evidence, or deferred runtime admission.
Original text remains in immutable Git commit `c5e7414d7`, an ancestor of main.
Historical words such as current, next, stop and authorized refer only to their
original packet. Canonical Live Status and the current manifest schedule work.

Retrieve a specific owner and range without loading the full archive:

```bash
git show c5e7414d7:thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md | sed -n '23695,23876p'
```

The line ranges below refer to the archived version, not current files. Current
owner docs retain the applicable invariants and point here for detailed source
anchors/receipts. Do not append new worker chronology to this index.

## Immutable source locations

| Path under thoughts/shared/plans | Baseline lines | Evidence |
|---|---|---|
| `2026-06-26-slug-v2-clean-restart.md` | 15–822 | Milestone evidence and September 10 current/prerequisite history |
| `2026-06-26-slug-v2-clean-restart.md` | 822–6640 | Accepted architecture and chronological M1–M7 checkpoints |
| `2026-06-26-slug-v2-clean-restart.md` | 6640–7080 | Direction resets, stage/milestone map and retained integration gates |
| `2026-06-26-slug-v2-clean-restart.md` | 7081–8106 | Historical query packet designs |
| `slug-v2-subplans/05-bzlmod-and-repository-graph.md` | 58–899 | Original M1 Bzlmod integration chain |
| `slug-v2-subplans/05-bzlmod-and-repository-graph.md` | 1160–4960 | Repository/canonical route, Host and built-in source ownership |
| `slug-v2-subplans/05-bzlmod-and-repository-graph.md` | 4961–5092 | Imported genrule and nodep pruning |
| `slug-v2-subplans/05-bzlmod-and-repository-graph.md` | 5093–5365 | Archive mode and verified local-file capture |
| `slug-v2-subplans/05-bzlmod-and-repository-graph.md` | 5366–5670 | Authentic metadata/payload demand and sentinel attempts |
| `slug-v2-subplans/05-bzlmod-and-repository-graph.md` | 5670–5975 | Typed registration diagnostics and observer design |
| `slug-v2-subplans/05-bzlmod-checkpoint-evidence.md` | 1–884 | Early module/discovery/MVS checkpoints |
| `slug-v2-subplans/05-bzlmod-checkpoint-evidence-2.md` | 1–1354 | Repository source/materialization design checkpoints |
| `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md` | 1–19280 | Repository/extension/source observation chronology |
| `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md` | 19281–19454 | Observer receipt and source diagnostic implementation |
| `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md` | 19455–19714 | Run registry and daemon launch/bind corrections |
| `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md` | 19715–19788 | Test-selector/Cargo launcher correction and registry acceptance |
| `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md` | 19789–19819 | Former authentic fixture policy decision stop |
| `slug-v2-subplans/06-analysis-toolchains-and-actions.md` | 664–23284 | Accepted configured analysis, providers, transitions, actions and loading handoffs |
| `slug-v2-subplans/06-analysis-toolchains-and-actions.md` | 23285–23421 | Execution-group runtime source matrix |
| `slug-v2-subplans/06-analysis-toolchains-and-actions.md` | 23422–23632 | Selected-toolchain request identity contract |
| `slug-v2-subplans/06-analysis-toolchains-and-actions.md` | 23633–23694 | Cross-owner conflict discovery |
| `slug-v2-subplans/06-analysis-toolchains-and-actions.md` | 23695–23876 | Validated requested-root closure and sharing contract |
| `slug-v2-subplans/06-analysis-toolchains-and-actions.md` | 23877–23981 | Combined R2 stop, candidate and baseline attribution |
| `slug-v2-subplans/08-ruleset-and-command-conformance.md` | 364–4826 | Query/cquery/aquery and ruleset acceptance chronology; finer index in Stage 8 |
| `slug-v2-subplans/10-bazel-build-and-bootstrap.md` | 76–2542 | Developer graph, fixture migration, BuildBuddy cache/RBE and no-CI evidence; finer index in Stage 10 |
| `slug-v2-subplans/10-bazel-build-and-bootstrap.md` | 2543–2644 | Original bootstrap and superseded comparison contract |

## Accepted boundaries retained by the compact status

M0 acceptance is `9897e940`. M1's full accepted producer inventory and its exit
audit are recorded in the archived canonical Live Status; do not infer that
compaction drops those accepted source/observed package/loading/query/build
frontiers. M2/M4 retain the shared configured graph; M3's 16 default functions
include `attr()` evidence at `4ea8f6c7`, retained descriptors at `83fe6037` and
activation at `ed38f82a`. M5/M6 retain only the stated FileWrite handoff.
Stage 8/10 current historical indexes preserve finer source and gate locations.

## Unaccepted candidate storage

The full output-conflict R2 candidate is reachable at `27e9e9c0c`, branch
`review/output-conflict-r2`, based on `97dffd5d4`. The branch snapshot has the
19 original code owners plus preservation-only `review-evidence/` files:
original candidate.patch, validation.txt and README. The patch digest is
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`.

```bash
git show 27e9e9c0c:review-evidence/validation.txt
git show 27e9e9c0c:review-evidence/candidate.patch
```

The worktree is `/home/wgray/slug-review-worktrees/output-conflict-r2` on this
host; use the branch/commit, not that path, as durable identity. Nothing is
pushed or accepted by preservation. Reconcile main's later prerequisites before
resuming evidence. Production integration omits preservation metadata and
requires all gates in the current Stage 5/6 fixture ledger.
