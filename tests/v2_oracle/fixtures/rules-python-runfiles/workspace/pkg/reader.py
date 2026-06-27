import os
from pathlib import Path

from pkg.helper import message


def _runfile_path(relative):
    normalized = relative.replace("\\", "/")
    manifest = os.environ.get("RUNFILES_MANIFEST_FILE")
    if manifest:
        suffix = "/" + normalized
        for line in Path(manifest).read_text(encoding="utf-8").splitlines():
            if not line:
                continue
            key, _, value = line.partition(" ")
            if key.replace("\\", "/").endswith(suffix):
                return Path(value)

    runfiles_dir = os.environ.get("RUNFILES_DIR")
    if runfiles_dir:
        for prefix in ("_main", os.environ.get("TEST_WORKSPACE", "_main"), ""):
            candidate = Path(runfiles_dir) / prefix / relative
            if candidate.is_file():
                return candidate

    raise FileNotFoundError(relative)


if __name__ == "__main__":
    data = _runfile_path("pkg/data/message.txt").read_text(encoding="utf-8").strip()
    print(f"import={message()}")
    print(f"runfile-data={data}")
