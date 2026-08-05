# Current Slug V2 Packet

Packet: `WP-6-m2-run-under-and-custom-flag-converter-implementation`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: private, source-equivalent valid-Unicode conversion for the two mixed
routes, with cache rendering and the full Java-String domain explicitly deferred.

## Goal

Implement the accepted conversion-only slice for `run_under` and
`experimental_propagate_custom_flag`. Keep their values private to the existing
native converter kernel and label context; do not expose a cache, renderer, or
command seam.

## Required design record

Preserve the existing 39/0 label classifier. Add two separate private mixed-route
functions (2/0): a valid-Unicode ShellUtils-equivalent RunUnder conversion and a
CustomFlag conversion. `RunUnderSuffix(Arc<[CompactString]>)` is the only
`Dupe` wrapper. The private `Allocative` `RunUnder` enum has
`Label { original: CompactString, suffix: RunUnderSuffix, label: ResolvedOptionLabel }`
and `Command { original: CompactString, suffix: RunUnderSuffix, command: CompactString }`
variants. CustomFlag's final value is `CompactString`.

The RunUnder tokenizer must retain raw original input, split only unquoted space
and tab, concatenate quote fragments, preserve quoted empty tokens, and implement
the exact single/double-quote and backslash state/error ordering. Map every
conversion failure to private `LabelConvertError::Invalid`; retain source error
texts only as evidence and defer user diagnostic projection. Classify only the
decoded first token beginning `//` or `@`, pass
that branch through the existing label-context helper, and keep the remaining
tokens as ordered suffix. The absent special-null default remains absent; an
explicit `null` is a command token. CustomFlag returns non-`//`/`@` input raw;
its label branch uses the `/...` escape/rewrite, including the
`//pkg:__subpackages__` collision: in main-repository contexts both
`//pkg/...` and `//pkg:__subpackages__` produce `@@//pkg/...`; corresponding
`@apparent//pkg/...` and `@apparent//pkg:__subpackages__` produce mapped/resolved
`@@repo//pkg/...`. PackageContext omits `@` for the current repository.

This is only the well-formed Unicode `&str` domain. Do not add a record renderer,
cache-key representation, UTF-16/WTF-8 representation, command activation,
runfiles/non-test trim, normalization, loader, checksum, wire, DICE, or a new
dependency. User-approved configured-target-cycle deferral remains explicit.

## Allowed paths

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `app/slug_configuration_v2/src/native/label_convert.rs`
- `app/slug_configuration_v2/src/native/tests.rs`

## Required tests and validation

Add focused exact tests for the shell state/error/suffix/classifier cases,
literal-versus-absent `null`, label contexts, raw CustomFlag defines, `/...`, and
the `:__subpackages__` collision. Run focused and crate tests/check, GNU-Windows
tests/check, formatting, archive, scope, cap, and `git diff --check` gates.

## Stop conditions

Stop with REPLAN on a renderer/cache need, a lone-surrogate/full-Java-String
need, new context/loader, reverse edge/cycle, or command ownership ambiguity.
Do not edit Cargo, fixtures, or add probes/artifacts.

## Diff budget

- Production Rust: at most 300 net lines.
- Test Rust: at most 500 net lines.
- Documentation: at most 100 net lines.
- Total: at most 900 net lines. No Cargo, fixture, generated, baseline, or
  unrelated changes.
