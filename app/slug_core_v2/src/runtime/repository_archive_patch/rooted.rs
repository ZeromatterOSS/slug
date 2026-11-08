//! The existing patch grammar, applied with two bounded streaming passes.
//! Source bodies never become a Vec/Arc. The private staged inode is covered
//! by the source root's full prepaid Archive backing and caller FD/scratch.
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;

use super::*;
use crate::runtime::repository_root_io::RootIo;
use crate::runtime::repository_root_io::WriteMode;

fn fail(e: impl std::fmt::Display) -> ArchiveMaterializationError {
    materialization(format!("rooted selected BCR patch: {e}"))
}
pub(super) fn apply(
    root: &RootIo<'_>,
    relative: &str,
    hunks: &[(HunkHeader, &[&str])],
    active: &dyn Fn() -> bool,
    scope: Option<&Arc<SourceServiceScope>>,
) -> Result<(), ArchiveMaterializationError> {
    let path = Path::new(relative);
    let before = root.lstat(path).map_err(fail)?;
    if before.st_size < 0 {
        return Err(fail("negative source size"));
    }
    root.admit_file_size(before.st_size as u64).map_err(fail)?;
    let _scratch = source_service::buffer(scope, 64 * 1024).map_err(fail)?;
    let input = root.open_read_regular(path).map_err(fail)?;
    stream(input, hunks, active, |_| Ok(()))?;
    if !crate::runtime::repository_root_io::unchanged(&before, &root.lstat(path).map_err(fail)?) {
        return Err(fail("source changed"));
    }
    let parent = path.parent().unwrap_or(Path::new(""));
    let mut staged = None;
    for index in 0..8192 {
        if !active() {
            return Err(fail("cancelled"));
        }
        let name = parent.join(format!(".slug-source-patch-{index:04x}"));
        match root.open_write_regular(&name, WriteMode::CreateNew) {
            Ok(file) => {
                staged = Some((name, file));
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(fail(e)),
        }
    }
    let (name, mut output) = staged.ok_or_else(|| fail("temporary name limit"))?;
    // Any error discards the entire private generation. No staged path escapes.
    let input = root.open_read_regular(path).map_err(fail)?;
    stream(input, hunks, active, |bytes| {
        source_service::write_counted(&mut output, bytes, active, scope).map_err(fail)
    })?;
    if !crate::runtime::repository_root_io::unchanged(&before, &root.lstat(path).map_err(fail)?) {
        return Err(fail("source changed"));
    }
    root.chmod_regular(&output, (before.st_mode & 0o7777) as u32)
        .map_err(fail)?;
    output.flush().map_err(fail)?;
    root.replace_regular(&name, path, &output).map_err(fail)
}
fn stream(
    file: File,
    hunks: &[(HunkHeader, &[&str])],
    active: &dyn Fn() -> bool,
    mut output: impl FnMut(&[u8]) -> Result<(), ArchiveMaterializationError>,
) -> Result<(), ArchiveMaterializationError> {
    let mut input = Lines {
        input: BufReader::with_capacity(64 * 1024, file),
        utf8: Utf8::default(),
        active,
    };
    let mut source_cursor = 0usize;
    let mut output_count = 0usize;
    let mut bytes = 0u64;
    let mut emit = |chunk: &[u8]| {
        bytes = bytes
            .checked_add(chunk.len() as u64)
            .filter(|n| *n <= 64 * 1024 * 1024)
            .ok_or_else(|| fail("file limit"))?;
        output(chunk)
    };
    for (header, lines) in hunks {
        let old = range_index(header.old_start, header.old_count)?;
        let new = range_index(header.new_start, header.new_count)?;
        let gap = old
            .checked_sub(source_cursor)
            .ok_or_else(|| fail("hunk ranges are inconsistent"))?;
        output_count = output_count
            .checked_add(gap)
            .ok_or_else(|| fail("hunk count overflow"))?;
        if new != output_count {
            return Err(fail("hunk ranges are inconsistent"));
        }
        for _ in 0..gap {
            if !input.line(None, &mut emit)? {
                return Err(fail("hunk exceeds source"));
            }
        }
        source_cursor = old;
        let mut old_seen = 0;
        let mut new_seen = 0;
        for line in *lines {
            let (kind, text) = line.split_at(1);
            match kind {
                " " => {
                    if !input.line(Some(text.as_bytes()), &mut emit)? {
                        return Err(fail("context exceeds source"));
                    }
                    source_cursor += 1;
                    old_seen += 1;
                    new_seen += 1;
                }
                "-" => {
                    if !input.line(Some(text.as_bytes()), &mut |_| Ok(()))? {
                        return Err(fail("deletion exceeds source"));
                    }
                    source_cursor += 1;
                    old_seen += 1;
                }
                "+" => {
                    emit(text.as_bytes())?;
                    new_seen += 1;
                }
                _ => unreachable!("parser validates hunk kinds"),
            }
        }
        if old_seen != header.old_count || new_seen != header.new_count {
            return Err(fail("hunk counts differ"));
        }
        output_count = output_count
            .checked_add(new_seen)
            .ok_or_else(|| fail("hunk count overflow"))?;
    }
    while input.line(None, &mut emit)? {}
    Ok(())
}
struct Lines<'a> {
    input: BufReader<File>,
    utf8: Utf8,
    active: &'a dyn Fn() -> bool,
}
impl Lines<'_> {
    fn line(
        &mut self,
        expected: Option<&[u8]>,
        emit: &mut impl FnMut(&[u8]) -> Result<(), ArchiveMaterializationError>,
    ) -> Result<bool, ArchiveMaterializationError> {
        let mut seen = 0;
        loop {
            if !(self.active)() {
                return Err(fail("cancelled"));
            }
            let buffer = self.input.fill_buf().map_err(fail)?;
            if buffer.is_empty() {
                return if seen == 0 {
                    Ok(false)
                } else {
                    Err(fail("source does not end in LF"))
                };
            }
            let end = buffer
                .iter()
                .position(|b| *b == b'\n')
                .map_or(buffer.len(), |n| n + 1);
            let bytes = &buffer[..end];
            self.utf8.consume(bytes)?;
            if let Some(expected) = expected {
                if expected.get(seen..seen + end) != Some(bytes) {
                    return Err(fail("context does not match source"));
                }
            }
            let complete = bytes.last() == Some(&b'\n');
            emit(bytes)?;
            seen += end;
            self.input.consume(end);
            if complete {
                if expected.is_some_and(|e| e.len() != seen) {
                    return Err(fail("context does not match source"));
                }
                return Ok(true);
            }
        }
    }
}
#[derive(Default)]
struct Utf8 {
    remaining: u8,
    min: u8,
    max: u8,
}
impl Utf8 {
    fn consume(&mut self, bytes: &[u8]) -> Result<(), ArchiveMaterializationError> {
        for &byte in bytes {
            if byte == 0 || byte == b'\r' {
                return Err(fail("source must be NUL-free UTF-8 LF text"));
            }
            if self.remaining != 0 {
                if byte < self.min || byte > self.max {
                    return Err(fail("source is not UTF-8"));
                }
                self.remaining -= 1;
                self.min = 0x80;
                self.max = 0xbf;
                continue;
            }
            match byte {
                0..=0x7f => {}
                0xc2..=0xdf => {
                    self.remaining = 1;
                    self.min = 0x80;
                    self.max = 0xbf
                }
                0xe0 => {
                    self.remaining = 2;
                    self.min = 0xa0;
                    self.max = 0xbf
                }
                0xe1..=0xec | 0xee..=0xef => {
                    self.remaining = 2;
                    self.min = 0x80;
                    self.max = 0xbf
                }
                0xed => {
                    self.remaining = 2;
                    self.min = 0x80;
                    self.max = 0x9f
                }
                0xf0 => {
                    self.remaining = 3;
                    self.min = 0x90;
                    self.max = 0xbf
                }
                0xf1..=0xf3 => {
                    self.remaining = 3;
                    self.min = 0x80;
                    self.max = 0xbf
                }
                0xf4 => {
                    self.remaining = 3;
                    self.min = 0x80;
                    self.max = 0x8f
                }
                _ => return Err(fail("source is not UTF-8")),
            }
        }
        Ok(())
    }
}
