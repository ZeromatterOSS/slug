# slug_build_event_stream

Bazel Build Event Protocol (BEP) and Build Event Service (BES) schemas +
translation layer for Slug. Exists so Slug can speak enough BEP that
third-party BES consumers (BuildBuddy, EngFlow, Trunk, custom collectors)
render a Slug invocation the same way they render a Bazel invocation.

## Layout

```
proto/
  src/main/…/build_event_stream.proto     ← vendored from Bazel 9.x
  src/main/protobuf/…                     ← transitive Bazel deps
  google/devtools/build/v1/…              ← BES core (googleapis de157ca3)
  google/api/…                            ← gRPC annotations
src/
  lib.rs                                  ← module re-exports for prost-generated types
  translate.rs                            ← BuckEvent → bep::BuildEvent mapping
  file_sink.rs                            ← --build_event_binary_file / _text_file sinks
examples/
  decode_bep.rs                           ← dump a BEP file's event sequence
  bep_diff.rs                             ← histogram diff of two BEP files
tests/
  roundtrip.rs, translate.rs, file_sink.rs
```

Upstream protos are copied verbatim. The only local modification is
`analysis_cache_service_metadata_status.proto`, converted from
`edition = "2023"` to `syntax = "proto3"` because `prost-build` 0.13 does not
yet support editions; the file contains only an enum so the conversion is
syntactic and wire-compatible.

## CLI surface

```
slug build --build_event_binary_file=events.pb   # length-delimited proto, Bazel-compatible
slug build --build_event_text_file=events.txt    # Debug-formatted, form-feed delimited
```

`--build_event_json_file` is not yet supported. Writing proto3-canonical JSON
(camelCase fields, ISO-8601 timestamps, string-name enums) requires
`pbjson-build` integration, tracked against Plan 18.8 conformance work.

## Conformance snapshot

As of 2026-04-24, building `examples/hello_world//:main` with both Bazel 9.1.0
and Slug produced the following event-type delta (`examples/bep_diff`):

| Event | Bazel | Slug | Notes |
|-------|-------|------|-------|
| Started | 1 | 1 | covered |
| Expanded (PatternExpanded) | 1 | 1 | covered |
| Configuration | 2 | 2 | covered |
| Configured (TargetConfigured) | 1 | 8 | Slug analyzes toolchain targets too; reduce by filtering in 18.2 |
| ActionExecuted | 0 | 5 | Bazel omits successful actions; Slug currently emits all |
| Finished (BuildFinished) | 1 | 1 | covered |
| BuildMetadata | 1 | 0 | add from invocation metadata (18.5) |
| UnstructuredCommandLine / StructuredCommandLine | 4 | 0 | add from argv (18.5) |
| OptionsParsed | 1 | 0 | add from config flags (18.5) |
| WorkspaceStatus / WorkspaceInfo | 2 | 0 | add workspace status command support (18.5) |
| Progress | 7 | 0 | requires stdout/stderr tee in subscriber (18.4 adjacent) |
| Fetch | 1 | 0 | emit on repo-rule fetches (out of scope for Plan 18) |
| NamedSetOfFiles + Completed | 2 | 0 | needed for target-output visibility on BuildBuddy |
| BuildToolLogs / BuildMetrics | 2 | 0 | nice-to-have dashboard content |
| ConvenienceSymlinksIdentified | 1 | 0 | wire from the `bazel-bin/` symlink code that already runs |

Reproduce:

```sh
# In examples/hello_world/:
bazel build //:main --build_event_binary_file=/tmp/bazel.pb
slug build //:main --build_event_binary_file=/tmp/slug.pb

cargo run -p slug_build_event_stream --example bep_diff -- /tmp/bazel.pb /tmp/slug.pb
```

The gap list is the working checklist for Plan 18.2-extensions and 18.5
metadata work.
