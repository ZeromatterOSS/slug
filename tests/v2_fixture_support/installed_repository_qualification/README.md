# Installed repository launcher qualification

Run from the checkout root as the unprivileged CLI user. `SLUG_BIN` points to
the protected, absolute path of the ordinary activation-candidate CLI. The
host needs a root-owned `/usr/bin/bwrap` admitted by that CLI and
`/usr/bin/python3`. The registry assembler uses the existing pinned offline
fixture. The socket control must succeed directly on the Host before running
the candidate.

```bash
set -euo pipefail
test "${SLUG_BIN#/}" != "$SLUG_BIN"
test -x "$SLUG_BIN"
test -x /usr/bin/bwrap
test ! -L /usr/bin/bwrap
test "$(stat -c %u /usr/bin/bwrap)" = 0
test "$(id -u)" -ne 0
/usr/bin/python3 -I -c 'import socket; socket.socket(socket.AF_UNIX, socket.SOCK_STREAM).close()'

fixture=tests/v2_fixture_support/installed_repository_qualification/workspace
qualification=$(mktemp -d)
printf 'Qualification directory %s\n' "$qualification"
python3 tools/v2_oracle/configured_cli_fixture.py assemble \
    --output "$qualification/prerequisites" | head -c 32769 > "$qualification/assembly.json"
test "$(wc -c < "$qualification/assembly.json")" -le 32768
cp -R "$fixture" "$qualification/workspace"
registry_uri=$(python3 -c 'import pathlib, sys; print(pathlib.Path(sys.argv[1]).as_uri())' \
    "$qualification/prerequisites/registry")
(
    cd "$qualification/workspace"
    timeout --kill-after=3s 32s "$SLUG_BIN" cquery //:installed_qualification \
        --registry="$registry_uri" --lockfile_mode=off --noshow_progress
) 2> >(tail -c 32768 > "$qualification/stderr") | head -c 8193 > "$qualification/stdout"
wait
test "$(wc -c < "$qualification/stdout")" -le 8192
test "$(wc -l < "$qualification/stdout")" -eq 1
grep -Ex '//:installed_qualification \(slugcfg-v2:[0-9a-f]{64}\)' "$qualification/stdout"
```

The export consumed by Cquery is created only after the repository sees the
confined nonzero Execute result, observes its file through the rooted owner,
applies the template and delete operations, and sees the continuation result.
The first payload checks retired credentials, no-new-privileges, seccomp,
closed setup descriptors, and `EPERM` for the same socket creation that
succeeded on the Host. A successful command goes through the production
owner’s authenticated bootstrap, lease progression, and reap gates. The
qualification directory keeps the bounded command output for inspection.
Repeat through a fresh, private `--output_base` dedicated to this workspace
before the public Rust comparison. Use a protected local parent. The daemon
assumes cooperation from processes running as the invoking user.
