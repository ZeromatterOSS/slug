# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-ci-admission-design`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one explicit, security-reviewed CI admission contract.

## Goal and required evidence

Edit only the owner plan (120 changed lines), this manifest (50 total), and the
canonical plan (8 changed lines), at most 178 changed lines. Obtain explicit user
approval for provider/workflow path, trusted event matrix, Linux x86_64 runner,
permissions, concurrency/cost/timeouts, and environment-only secret injection.

## Stops and budget

Freeze two separate serialized trusted jobs/steps using only the canonical no-arg
CLIs: full cache once, then full RBE once, with no reconstruction, combination,
fallback, or retry. Untrusted/fork PRs receive no secrets and claim no remote
proof. Map each fixed public class faithfully to CI status; only its proof class
passes, while remote infrastructure and target failures remain distinguishable.

Bind CI to Bazel 9.2, exact RC/manifest hashes, `{build:1,test:43}`, Linux x86_64,
managed Linux/amd64, clean lifecycle, and the unchanged expected-red/core-unit
boundaries. Select an exact later workflow allowlist/cap and independent security
review. Missing provider/security/cost authority, secret exposure, raw retention,
driver/config/manifest changes, fallback/retry, or a Stage 10 completion claim is
`REPLAN`. No workflow/code/config/home/remote access or change in this design.
