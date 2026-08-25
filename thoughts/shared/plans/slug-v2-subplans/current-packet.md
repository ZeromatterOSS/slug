# Current Slug V2 Packet

Packet: `WP-6-7A-generated-package-selected-extension-owner-pure-representation-correction-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design base: accepted owner-first design `b97d6372`, nested-child correction
`1d71320f`, retained corrected Bzlmod Phase 1, repeated no-edit Phase-2 stops
and independent scratch-only retained-representation audit

Result: implement the accepted minimal owner-pure/final-certificate
representation without copying the workspace-global stage containers.

## Exact authority and frozen state

Write only the nine named Rust surfaces and their colocated proofs. Fixtures,
oracles, Cargo/BUILD, all other files and `../zabel` are read-only.

The first implementation attempt verified every frozen precondition, performed
no source edit and stopped because the 423-line `selected_repo_spec.rs`
headroom cannot contain the mandatory owner/demand/input surfaces. Conservative
Bzlmod production alone is >520 lines before proof: ~110 owner/value/error,
~230 for two Legacy/Observed key families/carriers/drivers and ~180 for typed
multi-module projection/mapping/admission. The existing workspace/root-only
aggregates cannot lawfully replace that work.

Use this pre-Phase-1 state only as the original semantic-accounting baseline:

| File | Lines | SHA-256 |
|---|---:|---|
| `app/slug_bzlmod_v2/src/selected_repo_spec.rs` | 13,397 | `25a0d0855ed83bc58942b02ec7daa1fcc78b50e604695a60b0e148b1edf24cad` |
| `app/slug_bzlmod_v2/src/lib.rs` | 430 | `1fa86c3c0f71e210adcd4aa618f238f032e445c36acdd2ad6aeb8ad31e81534c` |
| `app/slug_loading_v2/src/bzl_module.rs` | 9,120 | `20737363c9048fa5b5f81e6b8d4cdeb139e413ae0f053abc0cdaa1cc85cb9a58` |
| `app/slug_loading_v2/src/module_extension.rs` | 2,237 | `a7eec688b42258175704ad45558dd993a884891ad4f3bb3596ea5d8ac9f55480` |
| `app/slug_loading_v2/src/module_extension_repository_instantiation.rs` | 2,062 | `d3c35be63df4a05227f668319307d5b21ef3790d2cf940b89c0a946196849ae9` |
| `app/slug_loading_v2/src/module_extension_repository_validation.rs` | 1,822 | `8f8004ed00a9339b8418f6a0c57ea2b7d4f15d96ecfd68ec945e1c494362d1e5` |
| `app/slug_loading_v2/src/lib.rs` | 92 | `19b2b7179b1ea209fcb07a97d4d3114f46f11b9174b103abdbd6c396ae6ec08c` |
| `app/slug_core_v2/src/runtime/generated_repository_definition.rs` | 3,985 | `8166e0c83a0f86e50d251d25b649be18cfd37020434f163a1e06dde723ba27ad` |

The retained nested child
`app/slug_bzlmod_v2/src/selected_repo_spec/selected_extension_demand.rs`
is present and frozen. The seven retained bridge/input candidate
files outside this table and the private Host registry owner remain frozen at
the hashes recorded by `b97d6372`; the accepted fixture/evidence stay byte-
identical.

The corrected Phase 1 passes its four focused tests and
`cargo check -p slug_bzlmod_v2`, with no new warning and `git diff --check`
clean. Freeze the live values:

- parent 13,415 lines,
  `ad8c89c9d4613db408ae7429e51e3a59ae2e865a90f0daaf896dc2fe9624c333`;
- Bzlmod lib 460 lines,
  `c697e9341eb1990c6ef47c157d0967e6a9ea1d4c94e71ad2c6415f5c9c7674ab`;
- child 1,128 lines,
  `ad47ae4e308d95ed3d55100cdd77caf2a0e067a10fb5322fb9f49338f4a0f508`.

## Owner-pure implementation

Retain the nested child module at
`selected_repo_spec/selected_extension_demand.rs`. Rust child privacy lets it
consume the parent-private selected-mapping keys, fields and types without
changing them to `pub(crate)` or adding an adapter DICE key.

Write authority and responsibility are exact:

- `selected_repo_spec.rs`: only child declaration and parent reexports;
- Bzlmod `lib.rs`: only doc-hidden cross-crate reexports;
- the new child: compact owner, demand and owner-input types; exactly two
  Bzlmod key families with shared Legacy/Observed drivers/carriers/errors;
  complete multi-module projection/admission; and colocated Bzlmod proofs;
- the same six loading/core files from `b97d6372`, with exactly two loading
  owner keys and the accepted demand-first core activation.

Corrected Phase 1 consumes approximately +779 production/+397 proof/+1,176
aggregate. Repeated Phase-2 attempts made no source edit. Direct reuse is not
lawful because every existing loaded/prepared/pure/instantiated/validated value
structurally retains workspace-global/root-only predecessors. A detailed audit
puts the remaining two keys/core at >=580 production before proof.

The three prior gaps are closed: typed demand disposition, explicit normalized
versions/invalid terminal, and real owner-input projection/order/mapping/epoch/
Need/outer proof. Keep the three live Bzlmod files byte-frozen during the
remaining loading/core implementation.

Design exactly this smaller representation:

- private owner-pure success retains only `Arc<OwnerInputs>` plus the existing
  single request/calls receipt;
- loaded definitions, prepared module rows, Starlark heaps, traversal/event
  buffers and Host-Bzl reacquisition state are compute-local scratch;
- semantic errors retain only authenticated owner inputs plus the current
  request/partial calls required for the exact diagnostic;
- observed Host-Bzl outers retain owner inputs for the first load and owner
  inputs plus the first completed projection for reacquisition, never the
  failed child carrier or epoch;
- final validation retains `Arc<OwnerPureResult>` plus one request-level
  instantiated value in a distinct owner certificate; existing global
  certificate representation stays unchanged;
- extract pure `instantiate_request` and `validate_request` helpers, translating
  their errors back into unchanged global aggregate errors in existing loops;
- generalize `InvocationContext` to `Arc<[InvocationModule]>`; existing global
  evaluation supplies one row, selected-owner evaluation supplies all rows with
  one shared tag-owner token; and
- preserve owner-input -> first HostBzl -> reacquired HostBzl left-first epochs,
  child/local invocation event ownership and exactly two loading DICE keys.

No accepted semantic contract changes. Demand authenticates typed imports and
deduplicates repeated same-owner matches without parsing canonical spellings.
Owner inputs preserve every root/non-root use of the selected id, each module's
final mapping, distinct definition base/final mappings, all tags/imports/
overrides, root-use admission and post-load definition-factor rejection. Global
aggregates remain complete. Need/outer/prefix, left-first epochs, two-key loading
events, retained state and exact/Slug-native/unsupported classifications remain
exactly those in `b97d6372`.

Caps are:

| File | Physical cap |
|---|---:|
| `selected_repo_spec.rs` | 13,440 |
| Bzlmod `lib.rs` | 480 |
| new `selected_extension_demand.rs` | 1,150 |
| loading `bzl_module.rs` | 9,500 |
| loading `module_extension.rs` | 2,490 |
| loading instantiation | 2,200 |
| loading validation | 1,960 |
| loading `lib.rs` | 112 |
| core generated definition | 4,160 |

Semantic caps are <=1,500 production, <=800 proof and <=2,300
aggregate from the original frozen state. All nine physical ceilings remain
unchanged. Existing `CompactString`, canonical types, `Arc`, compact maps/sets
and `Allocative` remain sufficient, so no Buck2/V1 import or Stage 9 row is
authorized.

## Compatibility, proof, validation and stops

Requested imported generated-repository behavior remains **exact Bazel 9**;
private owner/key/carrier and nested-module organization are **Slug-native**;
directly selected unadmitted owner execution remains **unsupported/deferred**.
The Bazel 9.2 per-`ModuleExtensionId` oracle and explicit `../zabel`
selected-demand-seed/owner-index/execution guidance from `b97d6372` remain the
architectural basis; copy no Zig code or representation.

Proof remains the complete `b97d6372` matrix, including
same-owner multi-module inputs, unrelated unsupported/failing owner isolation,
direct unsupported, mapping polarity, output selection, Legacy/Observed epoch/
event/terminal/lifecycle behavior, full Bzlmod/loading/core baselines and the
rebuilt byte-identical fixture. Run focused Bzlmod child, loading owner and core
generated proofs first; then full Bzlmod/loading/core lib/runtime/build/cquery/
query baselines serially. Rebuild `slug_cli_v2` before invoking the accepted
fixture, clean stale `slugd` before/after daemon-sensitive runs, and finish with
formatting, diff/secret/archive/rustfmt-skip, frozen-scope, physical and semantic
cap checks plus independent implementation review.

Reach independent implementation ACCEPT or formal REPLAN. STOP a root-level
sibling, parent-private visibility widening, a third Bzlmod/
loading key, semantic/proof weakening, cap/authority widening, delimiter
parsing, global-carrier filtering, Java/JVM, milestone closure, M8/M7B and exact
identity-byte work. `REPLAN` before widening. M7 remains partial and
M7A -> M8 -> M7B remains.
