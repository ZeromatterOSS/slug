# Fixed rolling mount-lease diagnostic

This developer diagnostic tests quota-backed root lifetime across namespace
launches. It runs only a fixed trusted helper, with no repository rule or user
program. Review changes to the scripts and frozen plan before executing.
`trusted_helper.py` is byte-for-byte v5. `parent.py` changes only plan selection
(`--plan` required) and output confinement to the frozen plan's
`parent_only_output_root`. `prepare.py` freezes local inputs without launching
bwrap, importing the other scripts, compiling or running the experiment.

Requires Linux x86_64, 4096-byte pages, glibc Python 3.9+ and bubblewrap with the
used options. It uses fixed x86_64 syscall/ABI definitions; pinning installed
headers does not prove an installed bwrap binary matches upstream source.
Unavailable user namespaces/mount permissions fail; there is no fallback.

The only child is the fixed trusted helper, with CAP_SYS_ADMIN in its own
user namespace. Three sequential namespaces test the same backing, parent FD
reads after reap, rolling detached leases cloned from an ATTACHED root, and
128 KiB/128-inode to 64 KiB/64-inode same-superblock shrink. Data exact/+1 uses
page-aligned fixed files; metadata/kernel memory and inode capacity are separate.
This is **not** a production runner, confinement/security proof, arbitrary tool
execution, Starlark API proof, source-observer proof or workload memory baseline.

After review, use the same selected interpreter for preparation and parent:

```bash
probe_dir="$(pwd)/scripts/v2_repository_root_probe"
probe_python=/usr/bin/python3.12
probe_bwrap=/absolute/path/to/bwrap
probe_output_root="$(pwd)/target/repository-root-probe"
probe_plan="$probe_output_root/frozen-plan.json"
"$probe_python" -I -S "$probe_dir/prepare.py" \
  --bwrap "$probe_bwrap" --python "$probe_python" \
  --output-root "$probe_output_root" --plan "$probe_plan"
# The previous command prints the exact frozen plan path and SHA-256.
# Independently review that plan and these pinned scripts before the next step.
probe_plan_sha=PASTE_THE_PRINTED_SHA256
probe_run="$probe_output_root/reviewed-run-001"
"$probe_python" -I -S "$probe_dir/parent.py" \
  --plan "$probe_plan" --plan-sha256 "$probe_plan_sha" \
  --output "$probe_run" --ack-experiment-only
```

Use a fresh plan filename when preparing again; existing plans are never
overwritten. Use a fresh run directory for every attempt. Optional
`--include-root` selects the installed header tree (default `/usr/include`);
required paths are `linux/{mount,nsfs}.h`,
`x86_64-linux-gnu/asm/unistd_64.h` and `x86_64-linux-gnu/bits/statfs.h`.
Preparation pins bwrap, Python, parent, exact helper, preparer and these headers;
there are no repository baseline/current-packet pins or environment snapshots.

Parent checks explicit plan hash and every pin before launch and again before
success. Parent-only output is seven fixed files beneath the selected fresh run
directory: `receipt.json` plus each phase's stderr/protocol capture. Protocol is
capped at8192 bytes/phase, stderr at65536; each live phase at17 seconds, cleanup
at2.5, no new launch after55 and whole lifecycle at60 seconds. Received FDs and
owned launchers are cleaned before final receipt. The Host reservation is a
fresh `/tmp/slug-rootfd-probe-*/root` removed afterward. Child env is empty;
helper records only its empty-env runtime startup keys/LC_CTYPE and clears them.

Read `status`, actual quota/inode counters, FD closure/reap fields and raw failure
details. An inode early refusal remains UNPROVED; never reinterpret an operational
or permission failure as success. Keep compact accepted evidence with input pins;
failure receipts remain evidence and do not justify broadening this operation.
