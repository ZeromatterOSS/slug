# Current Slug V2 Work Packet

Packet: WP-7-10-m7a-partial-inventory-accounting-r1
Status: corrected partial Cargo/static inventory ACCEPT; Bazel reachability and M7A remain open

## Result and boundary

Record only what the frozen Cargo closure and existing authenticated repository
artifacts prove after the live Bazel query and its recovery produced no row.
The result is a finite local package/path, feature, proc-macro, build-script,
generated-source and pin accounting against [Stage 10's accepted 33-package
inventory](./10-bazel-build-and-bootstrap.md#accepted-production-inventory).
It must leave Bazel declared/configured reachability and M7A behavior open.
Compatibility classification is unchanged: preserve previously accepted exact
and Slug-native slices; all unobserved Bazel coverage remains
unsupported/deferred. No semantic owner, runtime value, DICE key, request path,
cache, fixture or fallback changes in this packet.

Freeze main `44e58083d`, the sole Cargo receipt SHA-256
`b3886868867d6b2c6f8d1d49575fbc0f4322ab83216e1da456ae7e2f1e2c5e2`,
analysis SHA-256
`dad20c2240aa421f5d99fe30190cbe7fa9035b4e388ad64aa30e762afaf9cd10`,
and zero-row terminal Bazel recovery receipt SHA-256
`bc4c11d4a939afa8b143f1bbc484d2d70a212a993111f79946f83f703209b252`.
The prior F3 packet and its exact reviewed result remain at main `44e58083d`;
F3 is resource-blocked at the one-test 30-second ceiling with no retry or
extension.

## Evidence and decision

The locked/offline Linux normal/build Cargo metadata selected 345 packages and
957 edges: 35 first-party and 310 external. Direct set comparison with the
accepted inventory adds `slug_configuration_v2` and `slug_starlark_v2` and
removes none. Map each local Cargo manifest to its repository package path and
check only static BUILD ownership. All 35 mappings, the unchanged five local
proc macros, five build-script owners, enabled local feature snapshot, and
generated-source declarations are recorded in
[Stage 10](./10-bazel-build-and-bootstrap.md#partial-live-cargo-inventory-reconciliation-2026-09-16).

The decisive static gap is `app/slug_configuration_v2`: six selected local
Cargo packages depend on it normally, and `Cargo.Bazel.lock` names its path,
but there is no BUILD file or Rust target in that directory. By contrast,
`slug_starlark_v2` has a named BUILD target and static first-party references.
It is absent from `Cargo.Bazel.lock`'s `workspace_members`, and eight of the
310 selected external name/version keys are absent from that lock's `crates`
map. Stage 10 records each missing key. This is a second, independent static
graph gap; the lock must be synchronized from authoritative manifests and
Cargo resolution rather than edited by hand.
These checks do not replace the missing Bazel query. Treat BUILD files and the
lock as declarations only; neither proves selected/configured reachability,
features, actions or generated inputs. The nine frozen authority hashes still
match current files, but the earlier 33-package inventory did not preserve
feature vectors, so do not invent a feature or historical pin delta.

## Scope, validation and stop

Docs-only allowlist: this manifest, canonical plan status, Stage 10 inventory
and bootstrap readiness. Read the frozen `/tmp` receipts and tracked source;
run no Cargo/Bazel build, test, query, network call, oracle, payload acquisition
or F3 replay. Validate receipt hashes, selected set arithmetic, static path/
target mapping, `git diff --check` and `python3 scripts/v2_plan_status.py`.
Independent result review must confirm the local delta and the strict
separation between static declarations and Bazel reachability before this
partial accounting is accepted. A source/receipt mismatch, missing owner
misclassification or unsupported reachability claim returns `REPLAN`.

The first result review corrected a dev-only REAPI edge and accepted the
35-package/path/build-owner accounting, but subsequent full static lock-key
comparison exposed the missing local and eight external entries above. That
material finding reopens only the partial result review; the frozen Cargo and
zero-row Bazel receipts remain unchanged.
Independent correction rereview returned `ACCEPT` after confirming the 34/35
local and 302/310 selected external lock-key intersections and all nine
missing keys. This accepts static accounting only.

On acceptance, preserve the Cargo receipt and missing Bazel view. The next
implementation packet may correct the static `slug_configuration_v2` BUILD
owner and synchronize the Bazel lock under its own scoped validation, but it
cannot accept the M7A production closure without separately reviewed live
Bazel/configured evidence and the remaining readiness gates. No other
inventory query or F3 replay is selected here.
