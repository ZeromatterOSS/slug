//! Bounded, in-process tar.xz/tar.gz extraction for ordered repository effects.
//!
//! Generated repositories use ordered regular-file replacement and their own
//! shared decode limits, while BCR retains its stricter extraction policy.

use std::collections::HashSet;
use std::fs::File;
use std::fs::FileTimes;
use std::fs::OpenOptions;
use std::fs::{self};
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::UNIX_EPOCH;

use flate2::read::MultiGzDecoder;
use slug_bzlmod_v2::GeneratedRepositoryArchiveFormat;
use xz2::read::XzDecoder;
use xz2::stream::CONCATENATED;
use xz2::stream::Stream;

use super::repository_io::ArchiveMaterializationError;
use super::repository_io::source_service::SourceCounter;
use super::repository_io::source_service::SourceServiceScope;
use super::repository_io::source_service::{self};

const BLOCK: usize = 512;
const CALL_DECODED_LIMIT: u64 = 512 * 1024 * 1024;
const AGGREGATE_DECODED_LIMIT: u64 = 1024 * 1024 * 1024;
const HEADER_LIMIT: usize = 1024;
const PATH_LIMIT: usize = 256;
const COMPONENT_LIMIT: usize = 32;
// Shared decoder allocation ceiling, independent of archive identity.
const XZ_DECODER_MEMORY_LIMIT: u64 = 136 * 1024 * 1024;

#[derive(Clone, Copy)]
struct DecodeLimits {
    per_call: u64,
    aggregate: u64,
}

const GENERATED_DECODE_LIMITS: DecodeLimits = DecodeLimits {
    per_call: CALL_DECODED_LIMIT,
    aggregate: AGGREGATE_DECODED_LIMIT,
};

/// Apply one verified archive to an unpublished, caller-owned staging root.
///
/// The caller invokes this in source order, owns SHA-verified capture and root
/// publication, and discards the root on any error or cancellation. Decoded
/// accounting includes every physical header, payload, padding, end marker,
/// and trailing zero block; it is shared across the complete invocation.
pub(super) fn extract_selected_generated_archive(
    capture: &File,
    root: &Path,
    strip_prefix: &str,
    active: &dyn Fn() -> bool,
    aggregate_decoded: &mut u64,
    scope: Option<&Arc<SourceServiceScope>>,
) -> Result<(), ArchiveMaterializationError> {
    extract_generated_archive(
        capture,
        root,
        strip_prefix,
        GeneratedRepositoryArchiveFormat::TarXz,
        active,
        aggregate_decoded,
        scope,
    )
}

pub(super) fn extract_generated_archive(
    capture: &File,
    root: &Path,
    strip_prefix: &str,
    format: GeneratedRepositoryArchiveFormat,
    active: &dyn Fn() -> bool,
    aggregate_decoded: &mut u64,
    scope: Option<&Arc<SourceServiceScope>>,
) -> Result<(), ArchiveMaterializationError> {
    extract_with_format_and_limits(
        capture,
        root,
        strip_prefix,
        format,
        active,
        aggregate_decoded,
        GENERATED_DECODE_LIMITS,
        scope,
    )
}

#[cfg(test)]
fn extract_with_limits(
    capture: &File,
    root: &Path,
    strip_prefix: &str,
    active: &dyn Fn() -> bool,
    aggregate_decoded: &mut u64,
    limits: DecodeLimits,
    scope: Option<&Arc<SourceServiceScope>>,
) -> Result<(), ArchiveMaterializationError> {
    extract_with_format_and_limits(
        capture,
        root,
        strip_prefix,
        GeneratedRepositoryArchiveFormat::TarXz,
        active,
        aggregate_decoded,
        limits,
        scope,
    )
}

fn extract_with_format_and_limits(
    capture: &File,
    root: &Path,
    strip_prefix: &str,
    format: GeneratedRepositoryArchiveFormat,
    active: &dyn Fn() -> bool,
    aggregate_decoded: &mut u64,
    limits: DecodeLimits,
    scope: Option<&Arc<SourceServiceScope>>,
) -> Result<(), ArchiveMaterializationError> {
    validate_strip_prefix(strip_prefix)?;
    let root_metadata = fs::symlink_metadata(root)
        .map_err(|error| materialization(format!("inspecting generated archive root: {error}")))?;
    if !root_metadata.file_type().is_dir() {
        return Err(materialization(
            "generated archive root is not a real directory",
        ));
    }
    if *aggregate_decoded > limits.aggregate {
        return Err(materialization(
            "generated archives exceed aggregate decoded limit",
        ));
    }
    if !active() {
        return Err(materialization("repository session is no longer active"));
    }

    let mut capture = capture
        .try_clone()
        .map_err(|error| materialization(format!("opening verified generated archive: {error}")))?;
    capture
        .seek(SeekFrom::Start(0))
        .map_err(|error| materialization(format!("seeking verified generated archive: {error}")))?;
    match format {
        GeneratedRepositoryArchiveFormat::TarXz => {
            // Bound XZ dictionary allocation and consume every concatenated stream.
            let _decoder_credit = source_service::decoder(scope, XZ_DECODER_MEMORY_LIMIT as usize)
                .map_err(materialization)?;
            let stream = Stream::new_stream_decoder(XZ_DECODER_MEMORY_LIMIT, CONCATENATED)
                .map_err(|error| {
                    materialization(format!("initializing bounded xz decoder: {error}"))
                })?;
            extract_tar(
                XzDecoder::new_stream(capture, stream),
                root,
                strip_prefix,
                active,
                aggregate_decoded,
                limits,
                scope,
            )
        }
        GeneratedRepositoryArchiveFormat::TarGz => {
            // flate2 bounds each optional extra/name/comment header field to
            // 65535 bytes. Reserve their storage before the constructor parses
            // the first header; later concatenated members reuse the decoder.
            let _gzip_header_credit =
                source_service::buffer(scope, 256 * 1024).map_err(materialization)?;
            extract_tar(
                MultiGzDecoder::new(capture),
                root,
                strip_prefix,
                active,
                aggregate_decoded,
                limits,
                scope,
            )
        }
    }
}

fn extract_tar(
    decoder: impl Read,
    root: &Path,
    strip_prefix: &str,
    active: &dyn Fn() -> bool,
    aggregate_decoded: &mut u64,
    limits: DecodeLimits,
    scope: Option<&Arc<SourceServiceScope>>,
) -> Result<(), ArchiveMaterializationError> {
    let _decoder_buffer_credit =
        source_service::buffer(scope, 64 * 1024 + 1024).map_err(materialization)?;
    let mut tar = BoundedTarReader::new(decoder, active, aggregate_decoded, limits, scope);
    let mut physical_headers = 0usize;
    let mut pending_longname: Option<String> = None;
    let mut prefix_found = strip_prefix.is_empty();
    let mut seen_stripped_paths = HashSet::<String>::new();

    loop {
        let block = tar.block("reading generated tar header")?;
        if is_zero(&block) {
            if !is_zero(&tar.block("reading generated tar end block")?) {
                return Err(materialization("generated tar has only one zero end block"));
            }
            if pending_longname.is_some() {
                return Err(materialization("generated tar has an orphan GNU long name"));
            }
            tar.finish_zero_padding()?;
            break;
        }
        physical_headers = physical_headers
            .checked_add(1)
            .filter(|count| *count <= HEADER_LIMIT)
            .ok_or_else(|| materialization("generated tar exceeds physical header limit"))?;
        validate_checksum(&block)?;
        let header = tar::Header::from_byte_slice(&block);
        let entry_type = header.entry_type();
        let size = header
            .size()
            .map_err(|error| materialization(format!("reading generated tar size: {error}")))?;

        if entry_type.is_gnu_longname() {
            if pending_longname.is_some() {
                return Err(materialization("generated tar has doubled GNU long names"));
            }
            // The payload may include one terminating NUL after a 256-byte path.
            if size == 0 || size > PATH_LIMIT as u64 + 1 {
                return Err(materialization(
                    "generated GNU long name exceeds path limit",
                ));
            }
            let mut bytes = [0u8; PATH_LIMIT + 1];
            tar.read_exact(&mut bytes[..size as usize], "reading GNU long name")?;
            tar.padding(size)?;
            let name = bytes[..size as usize]
                .strip_suffix(&[0])
                .unwrap_or(&bytes[..size as usize]);
            pending_longname = Some(validate_path_text(name)?);
            continue;
        }
        let kind = if entry_type.is_file() {
            EntryKind::Regular
        } else if entry_type.is_dir() {
            EntryKind::Directory
        } else {
            // Includes PAX, GNU longlink/sparse, symlinks, hardlinks and nodes.
            return Err(materialization(
                "generated tar contains an unsupported entry type",
            ));
        };
        let mode = header
            .mode()
            .map_err(|error| materialization(format!("reading generated tar mode: {error}")))?;
        if (kind == EntryKind::Regular && !matches!(mode, 0o644 | 0o755))
            || (kind == EntryKind::Directory && mode != 0o755)
        {
            return Err(materialization(
                "generated tar has an unsupported entry mode",
            ));
        }
        if kind == EntryKind::Directory && size != 0 {
            return Err(materialization("generated tar directory has a payload"));
        }
        if kind == EntryKind::Regular {
            tar.admit_payload(size)?;
        }
        let raw_path = match pending_longname.take() {
            Some(path) => path,
            None => validate_path_text(&header.path_bytes())?,
        };
        let Some(path) = normalize_path(&raw_path, kind)? else {
            if kind != EntryKind::Directory {
                return Err(materialization("generated tar has an invalid root entry"));
            }
            tar.padding(size)?;
            continue;
        };
        let stripped = if strip_prefix.is_empty() {
            Some(path.as_str())
        } else if path == strip_prefix {
            if kind != EntryKind::Directory {
                return Err(materialization(
                    "generated tar strip prefix root is not a directory",
                ));
            }
            prefix_found = true;
            if !seen_stripped_paths.insert(String::new()) {
                return Err(materialization(
                    "generated tar repeats a stripped destination",
                ));
            }
            None
        } else if let Some(suffix) = path
            .strip_prefix(strip_prefix)
            .and_then(|suffix| suffix.strip_prefix('/'))
        {
            prefix_found = true;
            Some(suffix)
        } else {
            None
        };
        if let Some(path) = stripped {
            if !seen_stripped_paths.insert(path.to_owned()) {
                return Err(materialization(
                    "generated tar repeats a stripped destination",
                ));
            }
            let destination = ensure_parent(root, path)?;
            let destination = destination.join(path.rsplit('/').next().unwrap_or(path));
            match kind {
                EntryKind::Directory => {
                    ensure_directory(&destination, path)?;
                }
                EntryKind::Regular => {
                    admit_regular_target(&destination, path)?;
                    let mut output = OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(&destination)
                        .map_err(|error| {
                            materialization(format!("opening generated file {path}: {error}"))
                        })?;
                    tar.copy_exact(size, &mut output, path)?;
                    output.flush().map_err(|error| {
                        materialization(format!("flushing generated file {path}: {error}"))
                    })?;
                    set_mode(&destination, mode | 0o400)?;
                    set_file_mtime(&output, &header, path)?;
                }
            }
        } else if kind == EntryKind::Regular {
            tar.copy_exact(size, &mut std::io::sink(), "discarded prefix entry")?;
        }
        tar.padding(size)?;
    }
    if !prefix_found {
        return Err(materialization("generated tar strip prefix was not found"));
    }
    if !active() {
        return Err(materialization("repository session is no longer active"));
    }
    Ok(())
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum EntryKind {
    Regular,
    Directory,
}

fn validate_strip_prefix(prefix: &str) -> Result<(), ArchiveMaterializationError> {
    if prefix.is_empty() {
        return Ok(());
    }
    if prefix.len() > PATH_LIMIT
        || prefix.starts_with('/')
        || prefix.ends_with('/')
        || prefix.contains('\\')
        || prefix.as_bytes().contains(&0)
    {
        return Err(materialization("generated tar has an invalid strip prefix"));
    }
    let components: Vec<_> = prefix.split('/').collect();
    if components.len() > COMPONENT_LIMIT
        || components
            .iter()
            .any(|component| component.is_empty() || matches!(*component, "." | ".."))
    {
        return Err(materialization("generated tar has an invalid strip prefix"));
    }
    Ok(())
}

fn validate_path_text(bytes: &[u8]) -> Result<String, ArchiveMaterializationError> {
    if bytes.is_empty() || bytes.contains(&0) || bytes.contains(&b'\\') || bytes.len() > PATH_LIMIT
    {
        return Err(materialization("generated tar has an invalid path"));
    }
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| materialization("generated tar path is not valid Unicode"))
}

fn normalize_path(
    path: &str,
    kind: EntryKind,
) -> Result<Option<String>, ArchiveMaterializationError> {
    let mut path = path;
    while let Some(rest) = path.strip_prefix("./") {
        path = rest;
    }
    if kind == EntryKind::Directory {
        path = path.strip_suffix('/').unwrap_or(path);
    } else if path.ends_with('/') {
        return Err(materialization(
            "generated regular path has a trailing slash",
        ));
    }
    if path.is_empty() {
        return Ok(None);
    }
    if path.starts_with('/') {
        return Err(materialization("generated tar path is absolute"));
    }
    let mut components = 0usize;
    for component in path.split('/') {
        if component.is_empty() || matches!(component, "." | "..") {
            return Err(materialization("generated tar path is not normalized"));
        }
        components += 1;
    }
    if components > COMPONENT_LIMIT {
        return Err(materialization(
            "generated tar path exceeds component limit",
        ));
    }
    Ok(Some(path.to_owned()))
}

fn ensure_parent(root: &Path, path: &str) -> Result<PathBuf, ArchiveMaterializationError> {
    let mut parent = root.to_path_buf();
    if let Some((parents, _)) = path.rsplit_once('/') {
        for component in parents.split('/') {
            parent.push(component);
            ensure_directory(&parent, path)?;
        }
    }
    Ok(parent)
}

fn ensure_directory(path: &Path, subject: &str) -> Result<(), ArchiveMaterializationError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(()),
        Ok(_) => Err(materialization(format!(
            "generated tar file/directory collision at {subject}"
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path).map_err(|error| {
                materialization(format!("creating generated directory {subject}: {error}"))
            })
        }
        Err(error) => Err(materialization(format!(
            "inspecting generated directory {subject}: {error}"
        ))),
    }
}

fn admit_regular_target(path: &Path, subject: &str) -> Result<(), ArchiveMaterializationError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(()),
        Ok(_) => Err(materialization(format!(
            "generated tar file/directory collision at {subject}"
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(materialization(format!(
            "inspecting generated file {subject}: {error}"
        ))),
    }
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<(), ArchiveMaterializationError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|error| {
        materialization(format!(
            "setting generated archive mode at {}: {error}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<(), ArchiveMaterializationError> {
    Err(materialization(
        "generated archive executable modes are unsupported on this platform",
    ))
}

fn set_file_mtime(
    output: &File,
    header: &tar::Header,
    subject: &str,
) -> Result<(), ArchiveMaterializationError> {
    let seconds = header
        .mtime()
        .map_err(|error| materialization(format!("reading generated tar mtime: {error}")))?;
    let modified = UNIX_EPOCH
        .checked_add(Duration::from_secs(seconds))
        .ok_or_else(|| materialization("generated tar mtime is out of range"))?;
    output
        .set_times(FileTimes::new().set_modified(modified))
        .map_err(|error| materialization(format!("setting generated mtime for {subject}: {error}")))
}

fn validate_checksum(block: &[u8; BLOCK]) -> Result<(), ArchiveMaterializationError> {
    let header = tar::Header::from_byte_slice(block);
    let expected = header
        .cksum()
        .map_err(|error| materialization(format!("reading generated tar checksum: {error}")))?;
    let actual = block[..148]
        .iter()
        .chain([b' '; 8].iter())
        .chain(block[156..].iter())
        .map(|byte| u32::from(*byte))
        .sum::<u32>();
    if expected != actual {
        return Err(materialization("generated tar header checksum mismatch"));
    }
    Ok(())
}

fn is_zero(block: &[u8; BLOCK]) -> bool {
    block.iter().all(|byte| *byte == 0)
}

struct BoundedTarReader<'a, R> {
    reader: R,
    active: &'a dyn Fn() -> bool,
    scope: Option<&'a Arc<SourceServiceScope>>,
    call_decoded: u64,
    aggregate_decoded: &'a mut u64,
    limits: DecodeLimits,
}

impl<'a, R: Read> BoundedTarReader<'a, R> {
    fn new(
        reader: R,
        active: &'a dyn Fn() -> bool,
        aggregate_decoded: &'a mut u64,
        limits: DecodeLimits,
        scope: Option<&'a Arc<SourceServiceScope>>,
    ) -> Self {
        Self {
            reader,
            active,
            scope,
            call_decoded: 0,
            aggregate_decoded,
            limits,
        }
    }

    fn block(&mut self, operation: &str) -> Result<[u8; BLOCK], ArchiveMaterializationError> {
        let mut block = [0; BLOCK];
        self.read_exact(&mut block, operation)?;
        Ok(block)
    }

    fn read_exact(
        &mut self,
        mut bytes: &mut [u8],
        operation: &str,
    ) -> Result<(), ArchiveMaterializationError> {
        while !bytes.is_empty() {
            let allowed = self.allowed(bytes.len().min(64 * 1024))?;
            if allowed == 0 {
                return Err(self.limit_error());
            }
            let count = source_service::read_counted(
                &mut self.reader,
                &mut bytes[..allowed],
                self.active,
                self.scope,
                SourceCounter::DecodedBytes,
            )
            .map_err(|error| materialization(format!("{operation}: {error}")))?;
            if count == 0 {
                return Err(materialization(format!(
                    "{operation}: unexpected end of stream"
                )));
            }
            self.account(count as u64);
            bytes = &mut bytes[count..];
        }
        Ok(())
    }

    fn copy_exact(
        &mut self,
        mut remaining: u64,
        output: &mut impl Write,
        subject: &str,
    ) -> Result<(), ArchiveMaterializationError> {
        let mut buffer = [0u8; 64 * 1024];
        while remaining != 0 {
            let length = buffer.len().min(remaining as usize);
            self.read_exact(&mut buffer[..length], "reading generated tar payload")?;
            source_service::write_counted(output, &buffer[..length], self.active, self.scope)
                .map_err(|error| {
                    materialization(format!("writing generated file {subject}: {error}"))
                })?;
            remaining -= length as u64;
        }
        Ok(())
    }

    fn padding(&mut self, size: u64) -> Result<(), ArchiveMaterializationError> {
        let padding = (BLOCK as u64 - size % BLOCK as u64) % BLOCK as u64;
        let mut bytes = [0u8; BLOCK];
        self.read_exact(
            &mut bytes[..padding as usize],
            "reading generated tar padding",
        )
    }

    fn admit_payload(&self, size: u64) -> Result<(), ArchiveMaterializationError> {
        let padding = (BLOCK as u64 - size % BLOCK as u64) % BLOCK as u64;
        let payload = size.checked_add(padding).ok_or_else(|| {
            materialization("generated tar payload size overflows decoded accounting")
        })?;
        if payload > self.limits.per_call.saturating_sub(self.call_decoded) {
            return Err(materialization(
                "generated tar exceeds per-call decoded limit",
            ));
        }
        if payload
            > self
                .limits
                .aggregate
                .saturating_sub(*self.aggregate_decoded)
        {
            return Err(materialization(
                "generated archives exceed aggregate decoded limit",
            ));
        }
        Ok(())
    }

    fn finish_zero_padding(&mut self) -> Result<(), ArchiveMaterializationError> {
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let allowed = self.allowed(buffer.len())?;
            if allowed == 0 {
                // At the exact cap, distinguish a valid EOF from one extra
                // decoded byte. The probe itself is never retained or written.
                let mut extra = [0u8; 1];
                return match source_service::read_counted(
                    &mut self.reader,
                    &mut extra,
                    self.active,
                    self.scope,
                    SourceCounter::DecodedBytes,
                ) {
                    Ok(0) => Ok(()),
                    Ok(_) => Err(self.limit_error()),
                    Err(error) => Err(materialization(format!(
                        "finishing generated compressed stream: {error}"
                    ))),
                };
            }
            let count = source_service::read_counted(
                &mut self.reader,
                &mut buffer[..allowed],
                self.active,
                self.scope,
                SourceCounter::DecodedBytes,
            )
            .map_err(|error| {
                materialization(format!("finishing generated compressed stream: {error}"))
            })?;
            if count == 0 {
                return Ok(());
            }
            self.account(count as u64);
            if buffer[..count].iter().any(|byte| *byte != 0) {
                return Err(materialization("generated tar has nonzero trailing data"));
            }
        }
    }

    fn allowed(&self, desired: usize) -> Result<usize, ArchiveMaterializationError> {
        if !(self.active)() {
            return Err(materialization("repository session is no longer active"));
        }
        let call_remaining = self.limits.per_call.saturating_sub(self.call_decoded);
        let aggregate_remaining = self
            .limits
            .aggregate
            .saturating_sub(*self.aggregate_decoded);
        Ok(desired
            .min(call_remaining as usize)
            .min(aggregate_remaining as usize))
    }

    fn account(&mut self, count: u64) {
        self.call_decoded += count;
        *self.aggregate_decoded += count;
    }

    fn limit_error(&self) -> ArchiveMaterializationError {
        if self.call_decoded >= self.limits.per_call {
            materialization("generated tar exceeds per-call decoded limit")
        } else {
            materialization("generated archives exceed aggregate decoded limit")
        }
    }
}

fn materialization(message: impl Into<String>) -> ArchiveMaterializationError {
    ArchiveMaterializationError::materialization(message)
}

#[cfg(test)]
#[path = "tests/repository_generated_archive_tests.rs"]
mod tests;
