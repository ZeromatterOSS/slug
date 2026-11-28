# Installed repository launcher qualification

Use the ordinary activation-candidate CLI as the unprivileged CLI user. The
host needs a protected CLI image, a root-owned `/usr/bin/bwrap` admitted by the
CLI, and `/usr/bin/python3`. Choose a short, protected local output parent for
the fresh private evidence directory and daemon socket.

```bash
python3 -B tests/v2_fixture_support/installed_repository_qualification/run.py \
    "$SLUG_BIN" --output-parent "$QUALIFICATION_PARENT"
```

The runner assembles the existing pinned offline registry and performs three
ordinary Cquery commands. One runs without an output base. Two run in a fresh
workspace through the same managed daemon. Both daemon commands must publish
the confinement marker from a fresh Execute and produce the same target label.
The runner checks the daemon's published directory and socket identities,
socket peer, executable and PID before accepting retention. Shutdown waits for
that exact process and requires the managed generation to retire.

The first payload checks retired capabilities, no-new-privileges, seccomp,
and closed setup descriptors. Its Unix socket attempt must return `EPERM`
after the same operation succeeds on the Host. The repository exports its
target after the first nonzero Execute, rooted read, template and delete, and
a second Execute that verifies the same root identity. The runner preserves
bounded command output in the evidence directory.

After qualification, compare the pinned Rust CrateInfo rows across both routes.
Use a fresh short output path under the same kind of protected parent.

```bash
python3 -B -m tools.v2_oracle.rust_toolchain_compare \
    --slug "$SLUG_BIN" --cache-root "$BAZEL_ARCHIVE_CAS" --output "$RUST_COMPARE_OUTPUT"
```

The comparison uses the existing Rust input inventory and Bazel 9.2.0
oracle. It checks exact stdout and empty Slug stderr for cold, warm, changed,
and restored editions in isolated workspaces.
