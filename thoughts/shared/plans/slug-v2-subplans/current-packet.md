# Current Slug V2 Packet

Packet: `WP-5-m1-loading-native-windows-host-glob-ordering-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: read-only feasibility and evidence design
Evidence: accepted typed OS-native directory-entry representation and Windows
classifier seam, but no native-Windows semantic evidence; current private byte
segment/traversal computation is Unix-only and non-Unix returns
`UnsupportedHost`.

Do not edit Rust, fixtures, harnesses, tools, or generated evidence. Read the
live OS-native directory representation, Windows classifier seam, Unix-only
glob segment/traversal values, and dormant loading adapter. Pin the exact Bazel
9.2 source and native observation needed to decide whether the current
representations could preserve Windows name identity and ordering end to end.

Freeze one minimal native-Windows evidence contract for directory listing,
segment candidates, traversal results, and final BUILD `glob()` ordering. It
must cover ASCII, BMP, non-BMP, and a Win32-created unpaired UTF-16 surrogate;
ordinary files/directories; the native symlink/reparse behavior already
required by traversal; repeated execution; and mutation/restoration. A
retained-server lifecycle is required only after a later implementation
activates the path. If retained oracle evidence is selected, specify the
smallest Windows-only harness/fixture boundary before any edit. GNU-Windows
compilation is supplemental only.

The design must state whether the typed directory entries and current private
`Arc<[u8]>` glob pattern/result values can be lossless on Windows. Treat the
Unix-only implementation and `UnsupportedHost` branches as an expected
possible `REPLAN` result, not as an already-existing Windows carrier. It may
schedule a later implementation only if native evidence and the current owner
graph suffice.

Stop with **REPLAN** if native Windows evidence is unavailable; any conversion
is lossy; lone-surrogate semantics would be inferred from Rust types, Java
source, or GNU-Windows linkage; or a new DICE key/owner, public/general string
identity, path-observation redesign/operation, or direct filesystem bypass is
needed. Registry JVM transport, discovery/MVS, external repositories, parser/
evaluator/callable activation, and broader glob matching/composition remain
out of scope.
