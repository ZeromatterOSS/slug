# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-unsafe

import json
import os
import re
import typing
from pathlib import Path

GOLDEN_DIRECTORY = "fixtures/"


def _prepend_header(content: str) -> str:
    return (
        f"# This file is {'@'}generated, "
        f"regenerate by re-running test with `-- --env BUCK2_UPDATE_GOLDEN=1` appended to the test command\n\n{content}"
    )


def _remove_ci_labels(content: str) -> str:
    # this label is only added for CI jobs, causing inconsistenty between local test and ci test.
    # Examples:
    #  "ci:overwrite",
    #  "ci:diff:linux:@fbcode//mode/dev-lg",
    #  "ci:continuous:linux:@fbcode//mode/dev-lg",
    new_content = []
    for line in content.splitlines():
        if "ci:" in line:
            continue
        new_content.append(line)
    return "\n".join(new_content)


def _normalize_newlines(content: str) -> str:
    """
    We use golden() with text data so in the interest of being a bit more
    platform independent we just normalize the newlines.
    """
    return "".join([line + "\n" for line in content.splitlines()])


def _test_repo_data_src() -> str:
    # `TEST_REPO_DATA_SRC` is set in the test runner
    dir = os.getenv("TEST_REPO_DATA_SRC")
    assert dir, "TEST_REPO_DATA_SRC must be set"
    return dir


def _unified_diff(
    *,
    left: str,
    right: str,
    file: str,
    context: int,
) -> str:
    import difflib

    return "".join(
        difflib.unified_diff(
            left.splitlines(keepends=True),
            right.splitlines(keepends=True),
            fromfile=file,
            tofile=file,
            n=context,
        ),
    )


def _is_update_invocation() -> bool:
    return os.getenv("BUCK2_UPDATE_GOLDEN") is not None


# Output is a map of `rel_path`-relative files to their expected values
def golden_dir(*, output: typing.Dict[str, str], rel_path: str) -> None:
    assert "golden" in rel_path, f"Golden path `{rel_path}` must contain `golden`"

    rel_path_path = Path(rel_path)

    for file, contents in output.items():
        golden(
            output=contents,
            rel_path=str(rel_path_path.joinpath(Path(file))),
        )

    # Check that there are no extra files
    path_in_src = Path(_test_repo_data_src()).joinpath(rel_path_path)

    for file in path_in_src.glob("**/*"):
        if file.is_dir():
            continue
        rel_file_path = str(file.relative_to(path_in_src))
        if rel_file_path not in output:
            if _is_update_invocation():
                file.unlink()
            else:
                raise AssertionError(
                    f"Extra golden file `{rel_file_path}` found, please remove it"
                )


def golden(*, output: str, rel_path: str) -> None:
    assert "golden" in rel_path, f"Golden path `{rel_path}` must contain `golden`"

    output = _prepend_header(output)
    output = _normalize_newlines(output)

    path_in_src = os.path.join(_test_repo_data_src(), rel_path)

    if _is_update_invocation():
        Path(path_in_src).parent.mkdir(parents=True, exist_ok=True)
        with open(path_in_src, "w") as f:
            f.write(output)
        return

    assert os.path.exists(path_in_src), f"Golden path `{path_in_src}` must exist"

    with open(path_in_src, "r") as f:
        expected = f.read()

    if _remove_ci_labels(expected) != _remove_ci_labels(output):
        unified_diff = _unified_diff(
            left=expected,
            right=output,
            file=path_in_src,
            context=3,
        )
        raise AssertionError(
            f"Expected golden file to match actual\n"
            f"\n\n{unified_diff}\n\n"
            "Re-run test with `-- --env BUCK2_UPDATE_GOLDEN=1` appended to the test command to regenerate the files"
        )


# Replace 128-bit configuration with placeholder.
def _replace_cfg_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


def golden_replace_cfg_hash(*, output: str, rel_path: str) -> None:
    golden(
        output=_replace_cfg_hash(output),
        rel_path=rel_path,
    )


def golden_replace_temp_path(*, output: str, rel_path: str, tmp_path: str) -> None:
    # Escaping backslashes are needed for windows paths
    tmp_path_escaped = tmp_path.replace("\\", "\\\\")
    golden(
        output=output.replace(tmp_path_escaped, "tmp-path").replace("\\\\", "/"),
        rel_path=rel_path,
    )



def sanitize_hashes(s: str) -> str:
    # Remote message hashes
    s = re.sub(r"\b[0-9]{16,}\b", "<STRING_HASH>", s)
    # Remove configuration hashes
    # This is so bad... we don't force these hashes to print as 16
    # characters... and that's hard to fix because we don't allow changes to
    # change action digests.
    s = re.sub(r"\b[0-9a-f]{12,16}\b", "<HASH>", s)
    # And action digests (SHA1: 40 chars, SHA256: 64 chars)
    s = re.sub(r"\b[0-9a-f]{40}:[0-9]{1,}\b", "<DIGEST>", s)
    return re.sub(r"\b[0-9a-f]{64}:[0-9]{1,}\b", "<DIGEST>", s)


def sanitize_stderr(s: str) -> str:
    # Remove tracing log lines (e.g., WARN/INFO/DEBUG lines from kuro's tracing output)
    # These look like: [TIMESTAMP] [2m2026-...Z[0m [33m WARN[0m [2mkuro_...[0m...
    # The inner ANSI timestamp also contains a timestamp; strip the whole line
    # Match lines that start with a timestamp bracket and contain WARN/INFO/DEBUG/ERROR
    s = re.sub(r"^\[.{29}\].*? (WARN|INFO|DEBUG|ERROR).+$", "", s, flags=re.MULTILINE)
    # Also handle after timestamp replacement
    s = re.sub(r"^\[<TIMESTAMP>\].*? (WARN|INFO|DEBUG|ERROR).+$", "", s, flags=re.MULTILINE)
    # Remove all timestamps
    s = re.sub(r"\[.{29}\]", "[<TIMESTAMP>]", s)
    # Remove all UUIDs
    s = re.sub(
        r"\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b", "<UUID>", s
    )
    # Remove "Commands" line
    s = re.sub(r"Commands: .+", "Commands: <COMMAND_STATS>", s)
    # Remove "Cache hits" percentage
    s = re.sub(r"Cache hits: .+", "Cache hits: <CACHE_STATS>", s)
    # Remove "Network" line
    s = re.sub(r"Network: .+", "Network: <NETWORK_STATS>", s)
    # Remove thread ID & path from "panicked at" line
    s = re.sub(r"\([0-9]+\) panicked at .+", "(<THREAD_ID>) panicked at <PATH>", s)
    # Strip Rust backtrace output (present when RUST_BACKTRACE=1 is set)
    s = re.sub(r"^[Ss]tack backtrace:.*$", "", s, flags=re.MULTILINE)
    s = re.sub(r"^\s{1,4}\d+: .+$", "", s, flags=re.MULTILINE)
    s = re.sub(r"^\s{8,}at .+$", "", s, flags=re.MULTILINE)
    s = re.sub(r"^note: run with `RUST_BACKTRACE=1` .+$", "", s, flags=re.MULTILINE)
    # Collapse multiple consecutive blank lines to a single blank line
    s = re.sub(r"\n{3,}", "\n\n", s)
    # Collapse multiple trailing newlines to a single newline
    s = re.sub(r"\n{2,}$", "\n", s)
    return sanitize_hashes(s)


def sanitize_stacktrace(s: str) -> str:
    s = sanitize_stderr(s)
    return "\n".join(
        filter(
            lambda x: re.match(r"\[<TIMESTAMP>\]((\s+\d+:)|(\s+at )).*", x) is None,
            s.splitlines(),
        )
    )


# Build report errors can change based on minor test changes such as
# 1. Adding a target in TARGETS.fixture
# 2. Line number changing due to code moving around
# Sanitize so that we only check the important bits of the error message
def sanitize_build_report_error(s: str) -> str:
    # Simplify analysis error message (Can change due to line number changes)
    s = re.sub(
        r"Error running analysis for.*\"", 'Error running analysis for <IRRELEVANT>"', s
    )
    # Simplify the Unknown target error (Can change due to number of targets in TARGETS.fixture)
    s = re.sub(
        r"Unknown target `.*` from package .*\"",
        'Unknown target `<TARGET>` from package <IRRELEVANT>"',
        s,
    )

    return sanitize_hashes(s)


def sanitize_build_report(report: dict) -> None:
    del report["trace_id"]
    del report["project_root"]

    # String cache keys can vary due to differences in platform hashes within the message
    if "strings" in report:
        # Sort by sanitized values
        strings = dict(
            sorted(
                report["strings"].items(),
                key=lambda item: sanitize_hashes(item[1]),
            )
        )
        # Create a new dict where the keys are 1 + a large number
        # in order for it to still be in the format of a string hash
        updated_strings = {}
        start = 10000000000000000
        for i, v in enumerate(strings.values()):
            updated_strings[i + start] = v

        report["strings"] = updated_strings


def sanitize_python(s: str, project_dir: Path) -> str:
    # Strip absolute project dir prefix
    s = s.replace(f"{project_dir}/", "")
    # Match python38 error formatting (can be removed when python38 is removed everywhere)
    s = re.sub(r" *\^+", "", s)
    s = s.replace("SyntaxError: invalid syntax", "IndentationError: unexpected indent")
    s = s.replace("[syntax] Syntax error!", "[indentation] Indentation error!")
    s = re.sub(r"(\\n)+", r"\\n", s)
    return s


def sanitize_json_strings(obj: typing.Any) -> typing.Any:
    """Recursively sanitize all string values in a JSON-like object using sanitize_stderr.

    This is useful for JSON reports where error message strings may contain
    Rust backtrace output (when RUST_BACKTRACE=1 is set).
    """
    if isinstance(obj, str):
        result = sanitize_stderr(obj)
        # Preserve original trailing newline behavior: sanitize_stderr may add
        # a trailing '\n' for backtrace cleanup, but JSON string values that
        # didn't originally end with '\n' should not get one added.
        if not obj.endswith("\n") and result.endswith("\n"):
            result = result.rstrip("\n")
        return result
    elif isinstance(obj, dict):
        return {k: sanitize_json_strings(v) for k, v in obj.items()}
    elif isinstance(obj, list):
        return [sanitize_json_strings(item) for item in obj]
    else:
        return obj


def strip_waiting_on(s: str) -> str:
    # Strip "Waiting on" lines
    return "\n".join(filter(lambda x: "Waiting on" not in x, s.splitlines()))
