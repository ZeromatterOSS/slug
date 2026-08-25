# Current Slug V2 Packet

Packet: `WP-6-7A-generated-package-load-bridge-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design base: audit-2 accepted 2026-08-24 / Rust `b42b004c`

Result: docs-only design for one same-crate core bridge child that lets the
external exported-source build branch load packages from extension-generated
repositories. Linux under WSL is the only platform target.

## Frontier facts (audit-2, 2026-08-24)

The build branch (dice.rs:4476) and the query external package graph both
acquire routes through public `RootRepositoryRouteKey`, which admits only
local-path overrides. The doc-hidden Generated route constructor
(`for_generated_repo_spec`) exists in bzlmod; per-canonical `RepoSpec`s live in
loading's `HostValidatedGeneratedRepositorySpecs`; only core can see both.
`RepositoryPackageLoad(Observation)Key` accepts any nonroot route with a
matching canonical name and runs unchanged end to end for direct-local
externals (`51127df8`, `bd4fb8db`).

## Active design contract

Docs only; all Rust read-only. The design must:

- freeze exactly one bridge child key family (legacy + observed) whose natural
  producer resolves an apparent name through the accepted
  `HostCanonicalRepositoryApparentMapping` family and yields either a routed
  public `RootRepositoryRoute::for_generated_repo_spec` value or a typed
  semantic terminal;
- freeze fallback polarity inside the build branch: the public route key is
  tried first and its exact existing diagnostics are preserved verbatim for
  direct-local/builtin/unknown cases; the bridge child runs only on the two
  generated-route error kinds;
- preserve Need > compatible outer > semantic ordering and left-first epoch
  union; parents retain one local Result Arc plus compact epoch only;
- name the file allowlist (expected: dice.rs driver + test-only proof),
  production/proof/aggregate caps, serial WSL validation, and REPLAN stops;
- classify exact / Slug-native / unsupported-deferred; and
- defer explicitly: query-path activation, public bridge export,
  fixture/oracle growth unless a demonstrated gap appears, other platforms,
  exact identity bytes.

STOP any Rust edit now, second owner/key family, private-inner exposure,
semantic/event/equality drift, cap or format waiver, milestone closure, M8/M7B
or exact identity work. REPLAN if no cohesive single child exists, if fallback
polarity would alter existing diagnostics, or if the bridge needs loading's
retained certificate rather than its public view.

After ACCEPT schedule exactly one implementation successor. M7 remains partial
and M7A -> M8 -> M7B remains.
