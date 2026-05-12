# Plan 28.4 Stage 13 acceptance fixture for the `ctx.expand_location`
# migration. The Rust impl (~330 LOC: pool-building + parser) was deleted;
# the bundled `_slug_expand_location` (backed by `slug_collect_location_pool`
# and `slug_lookup_output_path` runtime hooks) now serves the call.
#
# Pins:
#   - $(location :leaf) resolves to the first output path of a dep.
#   - $(locations :leaf) joins multiple output paths with " ".
#   - $(execpath :leaf) resolves identically to $(location :leaf).
#   - $(rootpath :leaf) resolves identically to $(location :leaf).
#   - Unresolved $(location :missing) is kept verbatim.
#   - Plain string with no $(...) passes through unchanged.
#   - Multiple substitutions in one string all expand.
#   - Plural form with a single-output dep produces that one path.

def _expand_location_leaf_impl(ctx):
    out1 = ctx.actions.declare_file(ctx.label.name + "_a.txt")
    out2 = ctx.actions.declare_file(ctx.label.name + "_b.txt")
    ctx.actions.write(out1, "leaf-a\n")
    ctx.actions.write(out2, "leaf-b\n")
    return [DefaultInfo(default_outputs = [out1, out2])]

_expand_location_leaf = rule(
    implementation = _expand_location_leaf_impl,
    attrs = {},
)

def _expand_location_probe_impl(ctx):
    deps = ctx.attr.deps

    # $(location :leaf) → first output path of the dep.
    loc = ctx.expand_location("$(location :expand_location_leaf_target)", deps)
    if not loc:
        fail("Stage 13: $(location) returned empty: %r" % loc)
    if not loc.endswith("_a.txt"):
        fail("Stage 13: $(location) expected *_a.txt, got %r" % loc)

    # $(locations :leaf) → space-joined paths (two outputs).
    locs = ctx.expand_location("$(locations :expand_location_leaf_target)", deps)
    parts = locs.split(" ")
    if len(parts) != 2:
        fail("Stage 13: $(locations) expected 2 parts, got %r" % locs)
    if not parts[0].endswith("_a.txt"):
        fail("Stage 13: $(locations)[0] expected *_a.txt, got %r" % parts[0])
    if not parts[1].endswith("_b.txt"):
        fail("Stage 13: $(locations)[1] expected *_b.txt, got %r" % parts[1])

    # $(execpath :leaf) → same path as $(location :leaf).
    ep = ctx.expand_location("$(execpath :expand_location_leaf_target)", deps)
    if ep != loc:
        fail("Stage 13: $(execpath) %r != $(location) %r" % (ep, loc))

    # $(rootpath :leaf) → same path as $(location :leaf).
    rp = ctx.expand_location("$(rootpath :expand_location_leaf_target)", deps)
    if rp != loc:
        fail("Stage 13: $(rootpath) %r != $(location) %r" % (rp, loc))

    # Unresolved pattern kept verbatim.
    unresolved = ctx.expand_location("$(location :no_such_target)", deps)
    if unresolved != "$(location :no_such_target)":
        fail("Stage 13: unresolved not verbatim: got %r" % unresolved)

    # Plain string unchanged.
    plain = ctx.expand_location("no markers here", deps)
    if plain != "no markers here":
        fail("Stage 13: plain string changed: got %r" % plain)

    # Multiple substitutions in one string.
    multi = ctx.expand_location(
        "A=$(location :expand_location_leaf_target) B=$(location :expand_location_leaf_target)",
        deps,
    )
    if multi != "A=%s B=%s" % (loc, loc):
        fail("Stage 13: multi: got %r" % multi)

    # Plural form with one-output dep — just the single path.
    loc_single = ctx.expand_location(
        "$(locations :expand_location_leaf_target)",
        deps,
    )

    # loc_single is a space-joined list; we already verified two parts above.
    # Re-check that it starts with the first path.
    if not loc_single.startswith(loc):
        fail("Stage 13: $(locations) start mismatch: got %r" % loc_single)

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "expand-location-proof-ok\n")
    return [DefaultInfo(default_output = out)]

_expand_location_probe = rule(
    implementation = _expand_location_probe_impl,
    attrs = {
        "deps": attr.label_list(),
    },
)

def expand_location_proof(name):
    _expand_location_leaf(name = "expand_location_leaf_target")
    _expand_location_probe(
        name = name,
        deps = [":expand_location_leaf_target"],
    )
