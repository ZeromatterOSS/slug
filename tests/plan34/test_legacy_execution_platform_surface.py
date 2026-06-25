import subprocess
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
LEGACY_EXECUTION_PLATFORM_TOKENS = (
    "CommandExecutorConfig(",
    "ExecutionPlatformInfo(",
)
STARLARK_OR_BUILD_SUFFIXES = (".bzl", ".bxl", "BUILD.bazel")
ALLOWED_LEGACY_PREFIXES = ("tests/", "examples/")


def _tracked_files() -> list[Path]:
    result = subprocess.run(
        ["git", "ls-files"],
        cwd=REPO_ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True,
    )
    return [REPO_ROOT / line for line in result.stdout.splitlines()]


def test_legacy_execution_platform_surface_is_test_or_example_only() -> None:
    violations: list[str] = []
    for path in _tracked_files():
        repo_path = path.relative_to(REPO_ROOT).as_posix()
        if repo_path.startswith(ALLOWED_LEGACY_PREFIXES):
            continue
        if not repo_path.endswith(STARLARK_OR_BUILD_SUFFIXES):
            continue
        text = path.read_text(encoding="utf-8")
        if any(token in text for token in LEGACY_EXECUTION_PLATFORM_TOKENS):
            violations.append(repo_path)

    assert violations == [], (
        "Legacy ExecutionPlatformInfo/CommandExecutorConfig is not a Bazel 9 "
        "execution-platform surface. Use native platform(exec_properties=...) "
        f"outside tests/examples. Violations: {violations}"
    )
