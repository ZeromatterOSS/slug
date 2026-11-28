"""Qualify the ordinary installed CLI through one-shot and retained Cquery."""

import argparse
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
import tempfile

FIXTURE = Path(__file__).resolve().parent
ROOT = FIXTURE.parents[2]
sys.path.insert(0, str(ROOT))

from tools.v2_oracle.installed_daemon_witness import InstalledDaemon
from tools.v2_oracle_lib.bounded_process import capture

LABEL = re.compile(rb"//:installed_qualification \(slugcfg-v2:[0-9a-f]{64}\)\n")


def check_host():
    uid = os.geteuid()
    if uid == 0 or os.getresuid() != (uid, uid, uid):
        raise RuntimeError("qualification requires the unprivileged CLI user")
    launcher = Path("/usr/bin/bwrap").lstat()
    if launcher.st_uid != 0 or not stat.S_ISREG(launcher.st_mode):
        raise RuntimeError("qualification requires a root-owned /usr/bin/bwrap")
    capture(
        ["/usr/bin/python3", "-I", "-c",
         "import socket; socket.socket(socket.AF_UNIX, socket.SOCK_STREAM).close()"],
        cwd=ROOT, timeout=5, max_bytes=8192,
    ).check_returncode()


def qualify(binary, parent):
    check_host()
    evidence = Path(tempfile.mkdtemp(prefix="slug-qual-", dir=parent))
    print(f"Qualification directory {evidence}", flush=True)

    prerequisites = evidence / "prerequisites"
    assembled = capture(
        [sys.executable, "-B", str(ROOT / "tools/v2_oracle/configured_cli_fixture.py"),
         "assemble", "--output", str(prerequisites)],
        cwd=ROOT, timeout=20, max_bytes=32768,
    )
    (evidence / "assembly.json").write_bytes(assembled.stdout)
    (evidence / "assembly.stderr").write_bytes(assembled.stderr)
    assembled.check_returncode()

    args = ["cquery", "//:installed_qualification",
            f"--registry={(prerequisites / 'registry').as_uri()}",
            "--lockfile_mode=off", "--noshow_progress"]

    def cquery(name, workspace, *, base=None):
        startup = [] if base is None else [f"--output_base={base}"]
        result = capture(
            [str(binary), *startup, *args], cwd=workspace,
            timeout=32 if base is None else 43, max_bytes=32768, log_prefix=evidence / name,
        )
        result.check_returncode()
        if not LABEL.fullmatch(result.stdout):
            raise RuntimeError(f"{name} did not report the qualification target")
        if result.stderr.splitlines().count(b"confinement-ok") != 1:
            raise RuntimeError(f"{name} did not publish exactly one confinement witness")
        return result.stdout

    one_shot = shutil.copytree(FIXTURE / "workspace", evidence / "one-shot")
    cquery("one-shot", one_shot)

    workspace = shutil.copytree(FIXTURE / "workspace", evidence / "daemon-workspace")
    base = evidence / "daemon"
    with InstalledDaemon(base, binary) as daemon:
        first = cquery("daemon-first", workspace, base=base)
        pid = daemon.sample()
        second = cquery("daemon-second", workspace, base=base)
        daemon.sample()
        if second != first:
            raise RuntimeError("retained Cquery changed the qualification target projection")
    print(f"Passed one-shot and both commands on installed daemon {pid}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("slug", type=Path, help="protected activation-candidate CLI")
    parser.add_argument("--output-parent", type=Path, required=True,
                        help="short protected local directory for fresh evidence")
    args = parser.parse_args()
    try:
        binary = args.slug.resolve(strict=True)
        if not binary.is_file() or not os.access(binary, os.X_OK):
            raise RuntimeError("qualification requires an executable CLI")
        qualify(binary, args.output_parent.resolve(strict=True))
    except (OSError, RuntimeError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f"Installed launcher qualification failed: {error}\n")


if __name__ == "__main__":
    main()
