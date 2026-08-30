use std::io::Write;
use std::path::Path;

use super::repository_io::ArchiveMaterializationError;

const PATCH_FILE_LIMIT: usize = 256;
const PATCH_HUNK_LIMIT: usize = 4096;
const PATCH_LINE_LIMIT: usize = 1 << 20;

pub(super) fn apply_selected_bcr_patch(
    root: &Path,
    bytes: &[u8],
    strip: usize,
    active: &dyn Fn() -> bool,
) -> Result<(), ArchiveMaterializationError> {
    if bytes.contains(&b'\r') || bytes.contains(&0) || !bytes.ends_with(b"\n") {
        return Err(materialization(
            "selected BCR patch must be NUL-free UTF-8 LF text ending in LF",
        ));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| materialization("selected BCR patch is not valid UTF-8"))?;
    if text.lines().any(|line| line.len() > PATCH_LINE_LIMIT) {
        return Err(materialization(
            "selected BCR patch line exceeds size limit",
        ));
    }
    let lines = text.split_inclusive('\n').collect::<Vec<_>>();
    let mut cursor = 0usize;
    let mut files = 0usize;
    while cursor < lines.len() {
        while cursor < lines.len() && separator(lines[cursor]) {
            cursor += 1;
        }
        if cursor == lines.len() {
            break;
        }
        if !active() {
            return Err(materialization("repository session is no longer active"));
        }
        let old = header_path(lines[cursor], "--- ", strip)?;
        cursor += 1;
        let new = lines
            .get(cursor)
            .ok_or_else(|| materialization("selected BCR patch is missing +++ header"))
            .and_then(|line| header_path(line, "+++ ", strip))?;
        cursor += 1;
        if old != new {
            return Err(materialization(
                "selected BCR patch old and new paths must be equal after stripping",
            ));
        }
        files += 1;
        if files > PATCH_FILE_LIMIT {
            return Err(materialization("selected BCR patch exceeds file limit"));
        }
        let mut hunks = Vec::new();
        while cursor < lines.len() && lines[cursor].starts_with("@@ ") {
            let header = parse_hunk_header(lines[cursor])?;
            cursor += 1;
            let start = cursor;
            while cursor < lines.len()
                && !lines[cursor].starts_with("@@ ")
                && !lines[cursor].starts_with("--- ")
                && !separator(lines[cursor])
            {
                if !matches!(lines[cursor].as_bytes().first(), Some(b' ' | b'+' | b'-')) {
                    return Err(materialization(
                        "selected BCR patch has an unsupported hunk line",
                    ));
                }
                cursor += 1;
            }
            if hunks.len() >= PATCH_HUNK_LIMIT {
                return Err(materialization("selected BCR patch exceeds hunk limit"));
            }
            hunks.push((header, &lines[start..cursor]));
        }
        if hunks.is_empty() {
            return Err(materialization("selected BCR patch file has no hunks"));
        }
        apply_file(root, &old, &hunks, active)?;
    }
    if files == 0 {
        return Err(materialization(
            "selected BCR patch has no file modifications",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct HunkHeader {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
}

fn apply_file(
    root: &Path,
    relative: &str,
    hunks: &[(HunkHeader, &[&str])],
    active: &dyn Fn() -> bool,
) -> Result<(), ArchiveMaterializationError> {
    let target = root.join(relative);
    let metadata = std::fs::symlink_metadata(&target).map_err(|error| {
        materialization(format!(
            "opening selected BCR patch target {relative}: {error}"
        ))
    })?;
    if !metadata.file_type().is_file() {
        return Err(materialization(
            "selected BCR patch target must be an existing regular file",
        ));
    }
    let bytes = std::fs::read(&target).map_err(|error| {
        materialization(format!(
            "reading selected BCR patch target {relative}: {error}"
        ))
    })?;
    if bytes.contains(&b'\r')
        || bytes.contains(&0)
        || (!bytes.is_empty() && !bytes.ends_with(b"\n"))
    {
        return Err(materialization(
            "selected BCR patch target must be NUL-free UTF-8 LF text",
        ));
    }
    let source = std::str::from_utf8(&bytes)
        .map_err(|_| materialization("selected BCR patch target is not valid UTF-8"))?;
    let source = source.split_inclusive('\n').collect::<Vec<_>>();
    let mut output = Vec::with_capacity(source.len());
    let mut source_cursor = 0usize;
    for (header, lines) in hunks {
        if !active() {
            return Err(materialization("repository session is no longer active"));
        }
        let old_index = range_index(header.old_start, header.old_count)?;
        let new_index = range_index(header.new_start, header.new_count)?;
        if old_index < source_cursor
            || old_index > source.len()
            || new_index != output.len() + old_index - source_cursor
        {
            return Err(materialization(
                "selected BCR patch hunk ranges are inconsistent",
            ));
        }
        output.extend_from_slice(&source[source_cursor..old_index]);
        source_cursor = old_index;
        let mut old_seen = 0usize;
        let mut new_seen = 0usize;
        for line in *lines {
            let (kind, text) = line.split_at(1);
            match kind {
                " " => {
                    require_source_line(&source, source_cursor, text)?;
                    output.push(source[source_cursor]);
                    source_cursor += 1;
                    old_seen += 1;
                    new_seen += 1;
                }
                "-" => {
                    require_source_line(&source, source_cursor, text)?;
                    source_cursor += 1;
                    old_seen += 1;
                }
                "+" => {
                    output.push(text);
                    new_seen += 1;
                }
                _ => unreachable!("validated patch hunk line kind"),
            }
        }
        if old_seen != header.old_count || new_seen != header.new_count {
            return Err(materialization(
                "selected BCR patch hunk line counts do not match its header",
            ));
        }
    }
    output.extend_from_slice(&source[source_cursor..]);
    let parent = target
        .parent()
        .ok_or_else(|| materialization("selected BCR patch target has no parent"))?;
    let mut staged = tempfile::NamedTempFile::new_in(parent).map_err(|error| {
        materialization(format!(
            "staging selected BCR patched file {relative}: {error}"
        ))
    })?;
    for line in output {
        staged.write_all(line.as_bytes()).map_err(|error| {
            materialization(format!(
                "writing selected BCR patched file {relative}: {error}"
            ))
        })?;
    }
    staged
        .as_file()
        .set_permissions(metadata.permissions())
        .map_err(|error| {
            materialization(format!(
                "setting selected BCR patched mode {relative}: {error}"
            ))
        })?;
    staged.as_file_mut().flush().map_err(|error| {
        materialization(format!(
            "flushing selected BCR patched file {relative}: {error}"
        ))
    })?;
    std::fs::remove_file(&target).map_err(|error| {
        materialization(format!(
            "replacing selected BCR patch target {relative}: {error}"
        ))
    })?;
    staged.persist(&target).map_err(|error| {
        materialization(format!(
            "placing selected BCR patched file {relative}: {}",
            error.error
        ))
    })?;
    Ok(())
}

fn require_source_line(
    source: &[&str],
    index: usize,
    expected: &str,
) -> Result<(), ArchiveMaterializationError> {
    if source.get(index).copied() != Some(expected) {
        return Err(materialization(
            "selected BCR patch context does not match source",
        ));
    }
    Ok(())
}

fn header_path(
    line: &str,
    prefix: &str,
    strip: usize,
) -> Result<String, ArchiveMaterializationError> {
    let path = line
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix('\n'))
        .ok_or_else(|| materialization("selected BCR patch has a malformed file header"))?;
    if path == "/dev/null" || path.contains('\t') || path.contains('\\') || path.starts_with('/') {
        return Err(materialization(
            "selected BCR patch has an unsupported file path",
        ));
    }
    let components = path.split('/').collect::<Vec<_>>();
    if strip >= components.len() {
        return Err(materialization(
            "selected BCR patch strip removes the entire path",
        ));
    }
    let components = &components[strip..];
    if components.len() > 32
        || components
            .iter()
            .any(|component| component.is_empty() || matches!(*component, "." | ".."))
    {
        return Err(materialization(
            "selected BCR patch path is not safe and normalized",
        ));
    }
    let path = components.join("/");
    if path.len() > 256 {
        return Err(materialization(
            "selected BCR patch path exceeds byte limit",
        ));
    }
    Ok(path)
}

fn parse_hunk_header(line: &str) -> Result<HunkHeader, ArchiveMaterializationError> {
    let body = line
        .strip_prefix("@@ ")
        .and_then(|value| value.split_once(" @@"))
        .map(|(ranges, _)| ranges)
        .ok_or_else(|| materialization("selected BCR patch has a malformed hunk header"))?;
    let mut ranges = body.split_ascii_whitespace();
    let old = ranges
        .next()
        .ok_or_else(|| materialization("selected BCR patch hunk is missing old range"))?;
    let new = ranges
        .next()
        .ok_or_else(|| materialization("selected BCR patch hunk is missing new range"))?;
    if ranges.next().is_some() {
        return Err(materialization("selected BCR patch hunk has extra ranges"));
    }
    let (old_start, old_count) = parse_range(old, '-')?;
    let (new_start, new_count) = parse_range(new, '+')?;
    if old_count == 0 || new_count == 0 {
        return Err(materialization(
            "selected BCR patch creation and deletion hunks are unsupported",
        ));
    }
    Ok(HunkHeader {
        old_start,
        old_count,
        new_start,
        new_count,
    })
}

fn parse_range(value: &str, sign: char) -> Result<(usize, usize), ArchiveMaterializationError> {
    let value = value
        .strip_prefix(sign)
        .ok_or_else(|| materialization("selected BCR patch hunk range has the wrong sign"))?;
    let (start, count) = value.split_once(',').unwrap_or((value, "1"));
    let start = start
        .parse::<usize>()
        .map_err(|_| materialization("selected BCR patch hunk start is invalid"))?;
    let count = count
        .parse::<usize>()
        .map_err(|_| materialization("selected BCR patch hunk count is invalid"))?;
    Ok((start, count))
}

fn range_index(start: usize, count: usize) -> Result<usize, ArchiveMaterializationError> {
    if count == 0 {
        Ok(start)
    } else {
        start
            .checked_sub(1)
            .ok_or_else(|| materialization("selected BCR patch hunk starts before line one"))
    }
}

fn separator(line: &str) -> bool {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.len() >= 3 && line.bytes().all(|byte| byte == b'=')
}

fn materialization(message: impl Into<String>) -> ArchiveMaterializationError {
    ArchiveMaterializationError::materialization(message)
}

#[cfg(test)]
#[path = "tests/repository_archive_patch_tests.rs"]
mod tests;
