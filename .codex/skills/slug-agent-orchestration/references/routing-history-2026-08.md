# Slug Agent Routing History: 2026-08

Archived terminal packet records displaced from the bounded live routing log.

| Date | Packet | Route | Wall time | Evidence | Review/rework | Result and next-use note |
|------|--------|-------|-----------|----------|---------------|--------------------------|
| 2026-08-04 | M1 external Starlark test-rule query design | One Terra-medium design/evidence writer, root owner audit, and one Sol-low reserved-boundary reviewer | not exposed | Fresh Bazel 9.2 probes prove an attribute-free external `test = True` rule has a successful ordinary dependency closure spanning `@bazel_tools`, platforms, rules_java, rules_shell, and remote coverage; live DICE inspection also proves the root-only Bzl key/cycle family cannot represent external source identity | The packet stopped before Rust or fixture edits. Review accepted the REPLAN, corrected the pinned source range through test coverage/run-under attributes, confirmed direct suite membership is not independently atomic, and bounded a future private route-keyed external Bzl key without authorizing test-rule projection | `REPLAN`; design only one dependency-free non-test external Starlark rule/loading slice, then treat test-base/tool-repository graph breadth separately |
