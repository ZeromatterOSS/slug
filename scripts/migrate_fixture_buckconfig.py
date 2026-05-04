#!/usr/bin/env python3
"""
Migrate test fixture .buckconfig files to MODULE.bazel format.

For each .buckconfig in bucket B:
  - Translate [cells]/[repositories] X = path -> bazel_dep + local_path_override
  - Translate [cell_aliases]/[repository_aliases] X = Y -> repo_name on bazel_dep
  - Drop [external_cells] X = bundled (auto-registered)
  - Drop [project] ignore / package_boundary_exceptions (no MODULE.bazel equivalent)
  - Drop [build] execution_platforms (use --platforms CLI flag instead)
  - Keep [buildfile] section in .buckconfig (plan 35.6b will handle it)
  - Refuse any section outside the allowed set

Usage:
    python3 scripts/migrate_fixture_buckconfig.py path/to/.buckconfig [...]
    # or pipe paths via stdin:
    cat paths.txt | python3 scripts/migrate_fixture_buckconfig.py
"""

import sys
import os
import re
from pathlib import Path
from typing import Optional

# Sections that are allowed in bucket B fixtures.
# project + build (execution_platforms only) are allowed per classification notes.
ALLOWED_SECTIONS = {
    "cells",
    "repositories",
    "cell_aliases",
    "repository_aliases",
    "external_cells",
    "buildfile",
    "project",
    "build",
}

# [project] keys that are safe to drop (no MODULE.bazel equivalent)
PROJECT_DROP_KEYS = {"ignore", "package_boundary_exceptions"}

# [build] keys that are safe to drop (migrate to CLI flags later)
BUILD_DROP_KEYS = {"execution_platforms"}


class BuckconfigError(Exception):
    pass


def parse_buckconfig(path: Path) -> dict[str, dict[str, str]]:
    """
    Parse a .buckconfig file into a dict of section -> {key: value}.

    Handles:
      - Leading/trailing whitespace
      - '= ' and ':' separators
      - Continuation lines (lines starting with whitespace after a key line)
    """
    sections: dict[str, dict[str, str]] = {}
    current_section: Optional[str] = None
    current_key: Optional[str] = None

    with open(path) as f:
        for lineno, line in enumerate(f, 1):
            # Strip trailing whitespace/newline but keep leading for continuation detection
            rstripped = line.rstrip()
            stripped = rstripped.strip()

            if not stripped or stripped.startswith("#") or stripped.startswith(";"):
                current_key = None
                continue

            # Section header
            m = re.match(r"^\s*\[([^\]]+)\]\s*$", rstripped)
            if m:
                current_section = m.group(1).strip().lower()
                if current_section not in sections:
                    sections[current_section] = {}
                current_key = None
                continue

            if current_section is None:
                # Content before any section — skip or error
                raise BuckconfigError(
                    f"{path}:{lineno}: content before first section header: {line!r}"
                )

            # Try to parse as key = value first (even if line starts with whitespace)
            m = re.match(r"^\s*([^=:\s][^=:]*?)\s*[=:]\s*(.*?)\s*$", rstripped)
            if m:
                current_key = m.group(1).strip().lower()
                sections[current_section][current_key] = m.group(2).strip()
                continue

            # Continuation line (starts with whitespace AND no '=' separator AND we have a current key)
            if line[0:1] in (" ", "\t") and current_key is not None:
                sections[current_section][current_key] += " " + stripped
                continue

            raise BuckconfigError(
                f"{path}:{lineno}: cannot parse line: {line!r}"
            )

    return sections


def validate_sections(path: Path, sections: dict[str, dict[str, str]]) -> None:
    """Raise BuckconfigError if any section is outside the allowed set."""
    for section in sections:
        if section not in ALLOWED_SECTIONS:
            raise BuckconfigError(
                f"BUCKET-C: {path}: unexpected section [{section}] — "
                f"fixture belongs in bucket C, not B"
            )

    # Validate [project] keys
    if "project" in sections:
        for key in sections["project"]:
            if key not in PROJECT_DROP_KEYS:
                raise BuckconfigError(
                    f"BUCKET-C: {path}: unexpected [project] key '{key}' — "
                    f"fixture belongs in bucket C, not B"
                )

    # Validate [build] keys
    if "build" in sections:
        for key in sections["build"]:
            if key not in BUILD_DROP_KEYS:
                raise BuckconfigError(
                    f"BUCKET-C: {path}: unexpected [build] key '{key}' — "
                    f"fixture belongs in bucket C, not B"
                )


def build_module_bazel(path: Path, sections: dict[str, dict[str, str]]) -> Optional[str]:
    """
    Build MODULE.bazel content from parsed sections.

    Returns None if there are no cells/repositories to declare
    (i.e., the fixture is buildfile-only).
    """
    cells = {}
    # Merge [cells] and [repositories] (same semantics)
    for section_name in ("cells", "repositories"):
        if section_name in sections:
            for name, cell_path in sections[section_name].items():
                # Later entries with the same name overwrite earlier ones
                # (mirrors .buckconfig semantics)
                cells[name] = cell_path

    if not cells:
        return None  # buildfile-only, skip

    # Collect aliases: alias_name -> canonical_cell_name
    aliases: dict[str, str] = {}
    for section_name in ("cell_aliases", "repository_aliases"):
        if section_name in sections:
            for alias_name, canonical in sections[section_name].items():
                aliases[alias_name] = canonical.lower()

    # Collect external_cells = bundled (just skip them; we note which are bundled
    # so we don't emit a local_path_override for them)
    bundled_cells: set[str] = set()
    if "external_cells" in sections:
        for cell_name, source in sections["external_cells"].items():
            if source.strip().lower() == "bundled":
                bundled_cells.add(cell_name)

    # Determine self cell (root = .)
    self_cells = [name for name, p in cells.items() if p == "."]

    lines = [
        "# AUTO-GENERATED by scripts/migrate_fixture_buckconfig.py",
        "# Source: " + str(path),
        "",
    ]

    # Emit module() for the self cell
    if self_cells:
        # Use the first self-cell (usually "root")
        self_name = self_cells[0]
        lines.append(f'module(name = "{self_name}")')
        lines.append("")

    # Build reverse map: canonical_cell -> [alias_names]
    # so we can attach repo_name to the right bazel_dep
    reverse_aliases: dict[str, list[str]] = {}
    for alias_name, canonical in aliases.items():
        if alias_name == canonical:
            continue  # identity alias, skip
        reverse_aliases.setdefault(canonical, []).append(alias_name)

    # Track which cells we've emitted local_path_override for
    emitted_overrides: set[str] = set()

    # Emit bazel_dep + local_path_override for each non-self cell
    for cell_name, cell_path in cells.items():
        if cell_path == ".":
            continue  # self cell, handled above
        if cell_name in bundled_cells:
            continue  # bundled, auto-registered

        # Primary bazel_dep (no repo_name)
        lines.append(f'bazel_dep(name = "{cell_name}")')

        # Additional bazel_dep entries for each alias pointing at this cell
        for alias in reverse_aliases.get(cell_name, []):
            lines.append(f'bazel_dep(name = "{cell_name}", repo_name = "{alias}")')

        # local_path_override (emit once per cell name)
        if cell_name not in emitted_overrides:
            lines.append(
                f'local_path_override(\n'
                f'    module_name = "{cell_name}",\n'
                f'    path = "{cell_path}",\n'
                f')'
            )
            emitted_overrides.add(cell_name)
        lines.append("")

    # Also handle aliases that point to cells NOT declared in [cells]
    # (edge case: alias to self or bundled cell)
    for alias_name, canonical in aliases.items():
        if alias_name == canonical:
            continue
        if canonical in cells and cells[canonical] != ".":
            continue  # already handled above
        if canonical in bundled_cells:
            continue  # bundled
        # alias points to self or unknown — emit a note
        if canonical in self_cells or canonical in cells and cells[canonical] == ".":
            # alias pointing to root cell — emit bazel_dep with repo_name
            root_name = self_cells[0] if self_cells else canonical
            lines.append(f'bazel_dep(name = "{root_name}", repo_name = "{alias_name}")')
            lines.append("")

    content = "\n".join(lines)
    # Collapse multiple consecutive blank lines into one
    content = re.sub(r"\n{3,}", "\n\n", content)
    return content.strip() + "\n"


def rewrite_buckconfig_buildfile_only(
    path: Path, sections: dict[str, dict[str, str]]
) -> Optional[str]:
    """
    Return the new .buckconfig content with only the [buildfile] section,
    or None if the file can be deleted entirely.
    """
    if "buildfile" not in sections:
        return None  # delete the file

    lines = ["[buildfile]"]
    for key, value in sections["buildfile"].items():
        lines.append(f"    {key} = {value}")
    lines.append("")
    return "\n".join(lines)


def is_already_migrated(buckconfig_path: Path) -> bool:
    """
    Returns True if MODULE.bazel already exists next to the .buckconfig
    AND the .buckconfig either doesn't exist or is already buildfile-only.
    """
    module_path = buckconfig_path.parent / "MODULE.bazel"
    if not module_path.exists():
        return False
    if not buckconfig_path.exists():
        return True
    # Check if .buckconfig is already buildfile-only (no cells section)
    try:
        sections = parse_buckconfig(buckconfig_path)
    except BuckconfigError:
        return False
    cell_sections = {"cells", "repositories", "cell_aliases", "repository_aliases", "external_cells"}
    return not any(s in sections for s in cell_sections)


def migrate(buckconfig_path: Path, dry_run: bool = False) -> str:
    """
    Migrate a single .buckconfig. Returns a status string.
    """
    if not buckconfig_path.exists():
        return f"SKIP (not found): {buckconfig_path}"

    if is_already_migrated(buckconfig_path):
        return f"SKIP (already migrated): {buckconfig_path}"

    try:
        sections = parse_buckconfig(buckconfig_path)
    except BuckconfigError as e:
        return f"ERROR (parse): {e}"

    try:
        validate_sections(buckconfig_path, sections)
    except BuckconfigError as e:
        return f"ERROR (validate): {e}"

    module_content = build_module_bazel(buckconfig_path, sections)
    new_buckconfig = rewrite_buckconfig_buildfile_only(buckconfig_path, sections)

    module_path = buckconfig_path.parent / "MODULE.bazel"

    if module_content is None:
        # No cells to migrate — the file is buildfile-only (or empty).
        # Leave it alone: nothing to do.
        return f"SKIP (buildfile-only, no cells to migrate): {buckconfig_path}"

    if dry_run:
        result = f"DRY-RUN: {buckconfig_path}"
        if module_content:
            result += f"\n  -> would write {module_path}"
        if new_buckconfig is None:
            result += f"\n  -> would delete {buckconfig_path}"
        else:
            result += f"\n  -> would rewrite .buckconfig to [buildfile]-only"
        return result

    # Write MODULE.bazel
    if module_content:
        with open(module_path, "w") as f:
            f.write(module_content)

    # Rewrite or delete .buckconfig
    if new_buckconfig is None:
        buckconfig_path.unlink()
        action = "deleted .buckconfig"
    else:
        with open(buckconfig_path, "w") as f:
            f.write(new_buckconfig)
        action = "rewrote .buckconfig to [buildfile]-only"

    if module_content:
        return f"MIGRATED: {buckconfig_path} -> {module_path} ({action})"
    else:
        return f"MIGRATED (buildfile-only, no MODULE.bazel): {buckconfig_path} ({action})"


def main() -> None:
    import argparse

    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument(
        "paths",
        nargs="*",
        help=".buckconfig paths to migrate (read from stdin if not given)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show what would be done without writing files",
    )
    args = parser.parse_args()

    if args.paths:
        paths = [Path(p) for p in args.paths]
    else:
        paths = [Path(line.strip()) for line in sys.stdin if line.strip()]

    migrated = 0
    skipped = 0
    errors = 0

    for path in paths:
        # Resolve relative paths from cwd
        if not path.is_absolute():
            path = Path(os.getcwd()) / path

        status = migrate(path, dry_run=args.dry_run)
        print(status)

        if status.startswith("MIGRATED"):
            migrated += 1
        elif status.startswith("SKIP"):
            skipped += 1
        elif status.startswith("ERROR") or status.startswith("DRY-RUN"):
            if status.startswith("ERROR"):
                errors += 1

    print(f"\nSummary: {migrated} migrated, {skipped} skipped, {errors} errors", file=sys.stderr)
    if errors:
        sys.exit(1)


if __name__ == "__main__":
    main()
