use std::sync::Arc;

use compact_str::CompactString;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModulePatchError(pub CompactString);

/// Apply the bounded unified-diff shape accepted by the source-preparation
/// fixture. The caller supplies one MODULE.bazel byte stream; patch paths are
/// validated only after Bazel's configured strip count is applied.
pub fn apply_unified_patch(
    original: Arc<[u8]>,
    patch: &[u8],
    strip: i32,
) -> Result<Arc<[u8]>, ModulePatchError> {
    let text = std::str::from_utf8(patch)
        .map_err(|_| ModulePatchError("patch is not UTF-8 unified diff text".into()))?;
    let mut lines = text.split_inclusive('\n').peekable();
    let Some(old) = lines.next().and_then(|line| line.strip_prefix("--- ")) else {
        return Err(ModulePatchError("patch is missing --- header".into()));
    };
    let Some(new) = lines.next().and_then(|line| line.strip_prefix("+++ ")) else {
        return Err(ModulePatchError("patch is missing +++ header".into()));
    };
    let old_path = patch_path(old.trim_end());
    let new_path = patch_path(new.trim_end());
    if strip == 0 && old_path == "a/MODULE.bazel" && new_path == "b/MODULE.bazel" {
        return Err(ModulePatchError(
            "patch path has an a/b prefix but strip is zero".into(),
        ));
    }
    let old_path = stripped_path(old_path, strip)?;
    let new_path = stripped_path(new_path, strip)?;
    let targets_module = old_path == "MODULE.bazel" && new_path == "MODULE.bazel";
    let source_lines = if targets_module {
        let source = std::str::from_utf8(&original).map_err(|_| {
            ModulePatchError("MODULE.bazel bytes are not UTF-8 for patching".into())
        })?;
        source.split_inclusive('\n').collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let mut output = Vec::with_capacity(original.len());
    let mut cursor = 0usize;
    let mut saw_hunk = false;
    while let Some(line) = lines.next() {
        if line.trim().is_empty() {
            continue;
        }
        let Some(header) = line.strip_prefix("@@ ") else {
            return Err(ModulePatchError(
                "patch contains content outside a hunk".into(),
            ));
        };
        let (start, old_count, new_count) = parse_ranges(header)?;
        if targets_module {
            if start == 0 || start - 1 < cursor || start - 1 > source_lines.len() {
                return Err(ModulePatchError(
                    "patch hunk has invalid source position".into(),
                ));
            }
            for source_line in &source_lines[cursor..start - 1] {
                output.extend_from_slice(source_line.as_bytes());
            }
            cursor = start - 1;
        }
        saw_hunk = true;
        let mut consumed_old = 0usize;
        let mut produced_new = 0usize;
        while let Some(next) = lines.peek().copied() {
            if next.starts_with("@@ ") {
                break;
            }
            let line = lines.next().unwrap();
            let (kind, body) = line.split_at(1);
            match kind {
                " " => {
                    if targets_module {
                        if source_lines.get(cursor).copied() != Some(body) {
                            return Err(ModulePatchError("patch context does not apply".into()));
                        }
                        output.extend_from_slice(body.as_bytes());
                        cursor += 1;
                    }
                    consumed_old += 1;
                    produced_new += 1;
                }
                "-" => {
                    if targets_module {
                        if source_lines.get(cursor).copied() != Some(body) {
                            return Err(ModulePatchError("patch removal does not apply".into()));
                        }
                        cursor += 1;
                    }
                    consumed_old += 1;
                }
                "+" => {
                    if targets_module {
                        output.extend_from_slice(body.as_bytes());
                    }
                    produced_new += 1;
                }
                "\\" if line.starts_with("\\ No newline") => {}
                _ => return Err(ModulePatchError("patch hunk line is malformed".into())),
            }
        }
        if consumed_old != old_count || produced_new != new_count {
            return Err(ModulePatchError(
                "patch hunk line counts do not match its header".into(),
            ));
        }
    }
    if !saw_hunk {
        return Err(ModulePatchError("patch has no hunk".into()));
    }
    if !targets_module {
        return Ok(original);
    }
    for source_line in &source_lines[cursor..] {
        output.extend_from_slice(source_line.as_bytes());
    }
    Ok(Arc::from(output))
}

fn stripped_path(path: &str, strip: i32) -> Result<String, ModulePatchError> {
    if path == "/dev/null" {
        return Ok(path.to_owned());
    }
    let parts = path
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let count = usize::try_from(strip.max(0)).unwrap();
    if parts.len() <= count {
        return Err(ModulePatchError("patch strip removes its file path".into()));
    }
    Ok(parts[count..].join("/"))
}

fn patch_path(header: &str) -> &str {
    header.split_whitespace().next().unwrap_or_default()
}

fn parse_ranges(header: &str) -> Result<(usize, usize, usize), ModulePatchError> {
    let (ranges, _) = header
        .split_once("@@")
        .ok_or_else(|| ModulePatchError("patch hunk is malformed".into()))?;
    if !ranges.chars().last().is_some_and(char::is_whitespace) {
        return Err(ModulePatchError("patch hunk is malformed".into()));
    }
    let mut fields = ranges.split_whitespace();
    let old = fields
        .next()
        .and_then(|value| value.strip_prefix('-'))
        .ok_or_else(|| ModulePatchError("patch hunk is malformed".into()))?;
    let new = fields
        .next()
        .ok_or_else(|| ModulePatchError("patch hunk is malformed".into()))?;
    if !new.starts_with('+') || fields.next().is_some() {
        return Err(ModulePatchError("patch hunk is malformed".into()));
    }
    let (start, old_count) = parse_range(old)?;
    let (_, new_count) = parse_range(&new[1..])?;
    Ok((start, old_count, new_count))
}

fn parse_range(value: &str) -> Result<(usize, usize), ModulePatchError> {
    let mut parts = value.split(',');
    let start = parts
        .next()
        .unwrap()
        .parse()
        .map_err(|_| ModulePatchError("patch hunk has invalid range".into()))?;
    let count = match parts.next() {
        Some(value) => value
            .parse()
            .map_err(|_| ModulePatchError("patch hunk has invalid range".into()))?,
        None => 1,
    };
    if parts.next().is_some() {
        return Err(ModulePatchError("patch hunk has invalid range".into()));
    }
    Ok((start, count))
}
