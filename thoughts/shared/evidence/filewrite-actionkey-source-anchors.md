# FileWrite ActionKey projection — Bazel 9.2 source anchors and verified reconstruction

Packet: read-only oracle-evidence preparation for the just-in-time FileWrite
ActionKey projection (M2/M5 identity contract; Stage 6/8 scheduling note in the
canonical plan). No Slug Rust, fixture, oracle, or Cargo/BUILD surface changed.
Writes in this worktree: this document plus a throwaway scratch workspace under
`target/v2o-fpcheck/` (untracked, deletable) and `tools/fpcheck/FP.java`
(throwaway probe, superseded by the verified Python mirror recorded here).

Pinned Bazel revision: 9.2.0, commit `8220c6198837d5c13d53fea211cf3282aa12408a`
(local worktree `/run/media/system/Colossus/dev/bazel-9.2.0`). Live evidence
was generated with the local `bazel` launcher resolving `.bazelversion` 9.2.0.

## Result

The complete FileWrite ActionKey byte stream is reconstructed and verified
against five independent live Bazel 9.2 observations (three from the accepted
oracle fixture's states and two fresh scratch-workspace vectors), including
both regular and compressed variants and the exact 256-character compression
boundary. The stream is:

```text
Fingerprint (SHA-256 over protobuf CodedOutputStream framing):
  RegularFileWriteAction.computeKey:
    addString("332877c7-ca9f-4731-b387-54f620408522")   // GUID
    addBoolean(makeExecutable)
    addString(content)                                  // UTF-8, varint-len prefix
  CompressedFileWriteAction.computeKey:                 // chosen iff len > 256 chars
    addString("5bfba914-2251-11ee-be56-0242ac120002")
    addBoolean(makeExecutable)
    addBytes(full gzip stream: header incl. OS=255 byte, deflate body, CRC32+ISIZE trailer)
  // ActionKeyComputer.getKey tail:
  addBoolean(executionPlatform != null)
  if platform:
    addString(platform label canonical form)            // "//pkg:name" for main repo
    -- ConstraintCollection.addToFingerprint:
       addBoolean(parent != null); [parent recursive]
       addInt(local size)
       per value: addString(setting label canonical); addString(value label canonical)
    addNullableString(remoteExecutionProperties)        // platform() without the attr => "" (present!)
    addStringMap(platform exec properties)              // addInt(size) + key/value pairs
    addStrings(flags)
    addStrings(requiredSettings labels)
    addStrings(allowedToolchainTypes labels)
    addBoolean(checkToolchainTypes)                     // default false via builder
    addNullableString(missingToolchainErrorMessage)     // platform() default is PRESENT:
                                                        // "For more information on platforms or
                                                        //  toolchains see https://bazel.build/
                                                        //  concepts/platforms-intro."
  addStringMap(action exec properties)                  // AbstractFileWriteAction: empty => addInt(0)
  addInt(ACTION_KEY_UNIQUIFIER)                         // default 0
hex(SHA-256)
```

Primitive encodings (`util/Fingerprint.java`, protobuf `CodedOutputStream`):
`addString` = varint-u32 byte-length prefix + UTF-8 bytes; `addBoolean` = one
raw 0x00/0x01 byte; non-negative `addInt`/`addInt32NoTag` = plain varint;
`addBytes` = raw injection with no framing; maps/lists are `addInt(size)` then
ordered elements.

## Verified vectors (live local Bazel 9.2 runs)

| stream | observed key |
|---|---|
| content-A, //:p0 (constraint p0v), remote "", regular | `db52cb731f412b398cbe464d3125670d2ca3a341c4994ac5bc6a05bfc3aed4c8` |
| content-A, //:p1 (constraint p1v) | `d7f42e2a9ae23739f3804d83240a30408c92944801cdcf37694467f0ca13e5fb` |
| content-B, //:p0 | `da346b95ba903052a5be03dc26338fe2b90a6a569fdbc3f25cec33a208e53eb8` |
| empty-constraint platform //:p0, content-A | `f1efb0a3ba00b87fac9f4ad7f625e7fb4212ff646db190886cab8f505686755b` |
| 256×'x' regular boundary | `addde10eeedb3fe387d398373d6be45669dae2e40ddf634670039147735f5889` |
| 257×'y' compressed | `79c3e087449f292d92773df152bf436a812f15d2bcc0f46f77174ccdacac197c` |
| 300×'A' compressed | `8220013a50aa86cfe579622b59993d5131a827005bf79606e33d72602de4d0ac` |
| 300×'B' compressed | `3056e6fd2f15462f585b3778bc3ee717f5590167b04555a220346934a0172baf` |

The first three equal the accepted `action-query-identity-evidence` expected
values exactly, so the accepted fixture remains valid under this
reconstruction.

## Key discriminating findings (previously unrecorded)

1. **Compression threshold boundary confirmed live**: exactly 256 Java chars
   stays `RegularFileWriteAction`; 257 goes compressed — matching the strict
   `fileContents.length() > COMPRESS_CHARS_THRESHOLD` reading.
2. **Compressed keys hash the FULL gzip container** — header (including the
   OS byte), deflate body, and CRC32/ISIZE trailer — via raw `addBytes`. The
   retained bytes must reproduce `java.util.zip.GZIPOutputStream(stream, 8192)`
   byte-for-byte. All four compressed vectors match Python zlib level-6 raw
   deflate with header OS byte **255** and mtime 0. Caveat for the Rust port:
   JDK's `GZIPOutputStream` writes its own fixed header; the observed matches
   used zlib's OS=255 header. Either the JDK on this host emits 255 or the OS
   byte coincides across both contents; the implementation packet must pin the
   JDK-produced gzip bytes once (single `jshell`/JDK run) before freezing the
   compressor.
3. **`platform()` always populates `remoteExecutionProperties` as the empty
   string** when the attribute is unset (nullable-string present, not null),
   and populates `missingToolchainErrorMessage` with the
   `PlatformRule.DEFAULT_MISSING_TOOLCHAIN_ERROR` text. Treating either as null
   breaks every key. Anchor: `rules/platform/PlatformRule.java:35,146`.

## Source anchors (pinned revision)

- `analysis/actions/FileWriteAction.java`: GUIDs (~250, ~285),
  `COMPRESS_CHARS_THRESHOLD = 256` (~69), selection (~190–205), both
  `computeKey` bodies (~276, ~352).
- `actions/ActionKeyComputer.java`: whole file (tail order, uniquifier).
- `analysis/actions/AbstractFileWriteAction.java`: `getExecProperties()` =>
  `ImmutableMap.of()` (~125).
- `util/Fingerprint.java`: SHA-256 default (~81), primitive encodings.
- `analysis/platform/PlatformInfo.java`: `addTo` field order (~166).
- `analysis/platform/ConstraintCollection.java`: `addToFingerprint` (~275).
- `analysis/platform/ConstraintValueInfo.java` / `ConstraintSettingInfo.java`:
  `addTo` label ordering (~108 / ~97).
- `cmdline/Label.java` + `PackageIdentifier.java` + `RepositoryName.java`:
  canonical form = `["@@" + name] + "//" + pkg + ":" + target`.
- `rules/platform/PlatformRule.java`: default missing-toolchain message.
- `query2/aquery/ActionGraphTextOutputFormatterCallback.java` (~221): the
  printed ActionKey is `ActionExecutionMetadata.getKey(ctx, null)` — the same
  value the projection must reproduce.

## Cross-check against Zabel donor

The reviewed donor commit `c7298478…` (`src/analysis/file_write_action_key.zig`,
`action_key_fingerprint.zig`) implements the identical tail order, GUIDs,
empty exec-properties handling, and uniquifier zero, including the same
`DEFAULT_MISSING_TOOLCHAIN_ERROR` constant. Its test vectors are consistent
with every finding above; no donor divergence was found. This satisfies the
contract's cross-check requirement at source level; the implementation packet
still owes the runtime cross-check of the pinned Bazel revision.

## Residual risk / next packet obligations

- Pin the exact JDK gzip bytes (header OS byte question above) with one direct
  JDK observation before admitting the compressor.
- Non-ASCII content exercises the Latin-1→UTF-8 internal-string expansion
  (`StringEncoding`); not yet exercised by any vector here. Add one
  discriminator (e.g. content containing U+00E9) to the implementation packet.
- Multi-level constraint-collection parents are covered by the donor tests but
  not by a fresh local run; low risk, optionally add one fixture row.
