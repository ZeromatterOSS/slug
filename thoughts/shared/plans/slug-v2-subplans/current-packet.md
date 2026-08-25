# Current Slug V2 Packet

Packet: `WP-6-7A-generated-repository-package-publication-frontier-audit`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design base: pending docs commit / Rust `846ef196`

Result: read-only audit choosing the smallest policy/lookup/source/load owner or
prerequisite that can eventually publish exact generated-repository packages
(extension chains such as `@rust_toolchains`) through the accepted
source-observation carrier. Linux under WSL is the only platform target;
Windows and macOS remain deferred.

## Active audit contract

Rust is read-only. The audit may change only scheduling/plan documents.

Accepted `846ef196` exposes the legacy semantic certificate/view/result/key and
observed key/carrier/opaque outer to a same-crate runtime sibling with zero
production callers. The consumer frontier stands: existing public
`RootRepositoryRoute` and its package source/load owners admit only builtin or
direct-local roots and cannot represent the extension-generated
`@rust_toolchains` chain; raw-source publication alone is not sufficient.

The audit must:

- name every prerequisite for exact generated BUILD loading: canonical
  deleted-package policy, `REPO.bazel` plus `.bazelignore`, and ordered
  `BUILD.bazel` then `BUILD` selection with complete epoch composition;
- choose the smallest next owner or prerequisite packet — do not assume that
  raw-source publication or the current narrow public route suffices;
- classify each surface exact / Slug-native / unsupported-deferred; and
- schedule exactly one successor with an observable result, allowlist, caps,
  validation and REPLAN stops per the plan-authoring guide.

STOP any Rust edit, public/crate-root exposure, package/public/bootstrap
activation, new key/carrier/adapter/caller, fixture/oracle growth, milestone
closure, M8/M7B or exact identity work. REPLAN before widening or if no bounded
smallest owner exists.

## Research and ownership

Reuse the accepted Bazel 9.2 source/Host-observation evidence and lifecycle
proof carried by `b3eba6df`/`846ef196`; this audit changes no user-visible
behavior and adds no fixture, oracle or upstream test. Existing
source-observation keys remain the natural DICE producers; the Result Arc plus
transaction-local epoch remains the sole retained value. There is no fallback.

## Authority

Docs-only write authority: this manifest plus the owner plan's scheduling tail.
Every other file is read-only. No Cargo/BUILD changes are permitted, so the
validation gate is documentation structure/diff hygiene plus consistency with
Live Status and the routing log.

After the audit ACCEPT, schedule its chosen successor as the current packet.
M7 remains partial and M7A -> M8 -> M7B remains.
