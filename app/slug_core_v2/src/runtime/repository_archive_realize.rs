use std::collections::HashMap;
use std::fs::File;
use std::fs::FileTimes;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use std::time::UNIX_EPOCH;

use flate2::read::MultiGzDecoder;
use sha2::Digest;
use sha2::Sha256;

use super::repository_archive::SelectedBcrArchive;
use super::repository_archive::SelectedBcrArchiveFormat;
use super::repository_io::ArchiveMaterializationError;
use super::repository_io::Materialized;

const BLOCK: usize = 512;
const DECOMPRESSED_LIMIT: u64 = 256 * 1024 * 1024;
const PAYLOAD_LIMIT: u64 = 256 * 1024 * 1024;
const ENTRY_LIMIT: u64 = 64 * 1024 * 1024;
const HEADER_LIMIT: usize = 8192;
const LOGICAL_LIMIT: usize = 8192;
const PATH_LIMIT: usize = 256;
const COMPONENT_LIMIT: usize = 32;
const MODULE_LIMIT: u64 = 1024 * 1024;
const PAX_PAYLOAD_LIMIT: u64 = 64 * 1024;

pub(super) fn realize_selected_bcr(
    plan: &SelectedBcrArchive,
    archive: tempfile::NamedTempFile,
    overlays: Vec<tempfile::NamedTempFile>,
    patches: Vec<tempfile::NamedTempFile>,
    module: tempfile::NamedTempFile,
    active: &dyn Fn() -> bool,
) -> Result<Materialized, ArchiveMaterializationError> {
    if overlays.len() != plan.overlays.len() || patches.len() != plan.patches.len() {
        return Err(materialization(
            "selected BCR transform capture count does not match plan",
        ));
    }
    let root = tempfile::tempdir()
        .map_err(|error| materialization(format!("creating selected BCR root: {error}")))?;
    extract(
        archive.as_file(),
        root.path(),
        plan.strip_prefix.as_deref(),
        active,
    )?;
    archive
        .close()
        .map_err(|error| materialization(format!("deleting verified archive capture: {error}")))?;
    for (overlay, capture) in plan.overlays.iter().zip(overlays) {
        place_overlay(capture, root.path(), &overlay.destination, active)?;
    }
    for capture in patches {
        let bytes = read_capture(capture, "patch", 8 * 1024 * 1024, active)?;
        super::repository_archive_patch::apply_selected_bcr_patch(
            root.path(),
            &bytes,
            plan.patch_strip,
            active,
        )?;
    }
    place_module(module, root.path(), active)?;
    if !active() {
        return Err(materialization("repository session is no longer active"));
    }
    Ok(Materialized::AssociatedImmutable {
        source_identity: selected_bcr_source_association(plan),
        root,
    })
}

fn selected_bcr_source_association(plan: &SelectedBcrArchive) -> Arc<str> {
    let mut digest = Sha256::new();
    digest.update(b"slug.selected-bcr-root.v2\0");
    framed(
        &mut digest,
        match plan.format {
            SelectedBcrArchiveFormat::TarGz => b"tar-gzip",
        },
    );
    framed(
        &mut digest,
        plan.strip_prefix.as_deref().unwrap_or("").as_bytes(),
    );
    framed(&mut digest, &plan.integrity);
    framed_usize(&mut digest, plan.patches.len());
    for patch in &plan.patches {
        framed(&mut digest, &patch.integrity);
    }
    framed_usize(&mut digest, plan.patch_strip);
    framed_usize(&mut digest, plan.overlays.len());
    for overlay in &plan.overlays {
        framed(&mut digest, overlay.destination.as_bytes());
        framed(&mut digest, &overlay.integrity);
    }
    framed(&mut digest, &plan.module_integrity);
    Arc::from(hex::encode(digest.finalize()))
}

fn framed(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}

fn framed_usize(digest: &mut Sha256, value: usize) {
    digest.update((value as u64).to_be_bytes());
}

fn extract(
    capture: &File,
    root: &Path,
    strip_prefix: Option<&str>,
    active: &dyn Fn() -> bool,
) -> Result<(), ArchiveMaterializationError> {
    let mut capture = capture
        .try_clone()
        .map_err(|error| materialization(format!("opening verified archive capture: {error}")))?;
    capture
        .seek(SeekFrom::Start(0))
        .map_err(|error| materialization(format!("seeking verified archive capture: {error}")))?;
    let decoder = MultiGzDecoder::new(capture);
    let mut tar = BoundedTarReader::new(decoder, active);
    let mut namespace = HashMap::<String, EntryKind>::new();
    let mut pending_longname: Option<String> = None;
    let mut headers = 0usize;
    let mut logical = 0usize;
    let mut payload = 0u64;
    let mut prefix_found = strip_prefix.is_none();

    loop {
        let block = tar.block("reading tar header")?;
        if is_zero(&block) {
            let second = tar.block("reading second tar end block")?;
            if !is_zero(&second) {
                return Err(materialization("selected BCR tar has one zero end block"));
            }
            if pending_longname.is_some() {
                return Err(materialization("selected BCR has an orphan GNU long name"));
            }
            tar.finish_zero_padding()?;
            break;
        }
        headers = headers
            .checked_add(1)
            .filter(|count| *count <= HEADER_LIMIT)
            .ok_or_else(|| materialization("selected BCR tar exceeds physical header limit"))?;
        validate_checksum(&block)?;
        let header = tar::Header::from_byte_slice(&block);
        let entry_type = header.entry_type();
        if entry_type.is_pax_local_extensions() {
            return Err(materialization(
                "selected BCR tar contains unsupported local PAX metadata",
            ));
        }
        if entry_type.is_gnu_longlink() {
            return Err(materialization("selected BCR tar contains a GNU long link"));
        }
        if entry_type.is_gnu_sparse() {
            return Err(materialization("selected BCR tar contains a sparse entry"));
        }
        let size = header
            .size()
            .map_err(|error| materialization(format!("reading tar entry size: {error}")))?;
        if entry_type.is_pax_global_extensions() {
            if size > PAX_PAYLOAD_LIMIT {
                return Err(materialization(
                    "selected BCR global PAX payload exceeds size limit",
                ));
            }
            payload = checked_payload(payload, size)?;
            let mut bytes = vec![0; size as usize];
            tar.read_exact(&mut bytes, "reading global PAX metadata")?;
            validate_global_pax_comment(&bytes)?;
            tar.padding(size)?;
            continue;
        }
        if entry_type.is_gnu_longname() {
            if pending_longname.is_some() {
                return Err(materialization(
                    "selected BCR tar has doubled GNU long names",
                ));
            }
            if size == 0 || size > PATH_LIMIT as u64 {
                return Err(materialization(
                    "selected BCR GNU long name exceeds path limit",
                ));
            }
            let mut bytes = [0u8; PATH_LIMIT];
            tar.read_exact(&mut bytes[..size as usize], "reading GNU long name")?;
            tar.padding(size)?;
            let bytes = &bytes[..size as usize];
            let bytes = bytes.strip_suffix(&[0]).unwrap_or(bytes);
            pending_longname = Some(validate_path_text(bytes)?);
            continue;
        }
        let kind = if entry_type.is_file() {
            EntryKind::Regular
        } else if entry_type.is_dir() {
            EntryKind::Directory
        } else {
            return Err(materialization(
                "selected BCR tar contains an unsupported entry type",
            ));
        };
        let mode = header
            .mode()
            .map_err(|error| materialization(format!("reading tar mode: {error}")))?;
        if (kind == EntryKind::Directory && !matches!(mode, 0o755 | 0o775))
            || (kind == EntryKind::Regular && !matches!(mode, 0o644 | 0o664 | 0o755 | 0o775))
        {
            return Err(materialization("selected BCR unsupported entry mode"));
        }
        logical = logical
            .checked_add(1)
            .filter(|count| *count <= LOGICAL_LIMIT)
            .ok_or_else(|| materialization("selected BCR tar exceeds logical entry limit"))?;
        let raw_path = match pending_longname.take() {
            Some(path) => path,
            None => validate_path_text(&header.path_bytes())?,
        };
        let Some(path) = normalize_path(&raw_path, kind)? else {
            if kind != EntryKind::Directory || size != 0 {
                return Err(materialization(
                    "selected BCR tar has an invalid root entry",
                ));
            }
            continue;
        };
        let path = match strip_prefix {
            None => Some(path),
            Some(prefix) if path == prefix => {
                if kind != EntryKind::Directory {
                    return Err(materialization(
                        "selected BCR strip_prefix root is not a directory",
                    ));
                }
                prefix_found = true;
                None
            }
            Some(prefix) => path
                .strip_prefix(prefix)
                .and_then(|suffix| suffix.strip_prefix('/'))
                .map(|suffix| {
                    prefix_found = true;
                    suffix.to_owned()
                }),
        };
        let Some(path) = path else {
            if kind == EntryKind::Directory {
                if size != 0 {
                    return Err(materialization("selected BCR directory has a payload"));
                }
            } else {
                if size > ENTRY_LIMIT {
                    return Err(materialization("selected BCR entry exceeds size limit"));
                }
                payload = checked_payload(payload, size)?;
                tar.copy_exact(size, &mut std::io::sink(), "discarded prefix entry")?;
            }
            tar.padding(size)?;
            continue;
        };
        admit_namespace(&namespace, &path, kind)?;
        namespace.insert(path.clone(), kind);
        let destination = root.join(&path);
        match kind {
            EntryKind::Directory => {
                if size != 0 {
                    return Err(materialization("selected BCR directory has a payload"));
                }
                std::fs::create_dir_all(&destination).map_err(|error| {
                    materialization(format!("creating selected BCR directory {path}: {error}"))
                })?;
                set_mode(&destination, 0o755)?;
            }
            EntryKind::Regular => {
                if size > ENTRY_LIMIT {
                    return Err(materialization("selected BCR entry exceeds size limit"));
                }
                payload = checked_payload(payload, size)?;
                if let Some(parent) = destination.parent() {
                    std::fs::create_dir_all(parent).map_err(|error| {
                        materialization(format!("creating selected BCR parent for {path}: {error}"))
                    })?;
                }
                let mut output = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&destination)
                    .map_err(|error| {
                        materialization(format!("creating selected BCR file {path}: {error}"))
                    })?;
                tar.copy_exact(size, &mut output, &path)?;
                output.flush().map_err(|error| {
                    materialization(format!("flushing selected BCR file {path}: {error}"))
                })?;
                set_mode(&destination, mode | 0o400)?;
                let mtime = header
                    .mtime()
                    .map_err(|error| materialization(format!("reading tar mtime: {error}")))?;
                let modified = UNIX_EPOCH
                    .checked_add(Duration::from_secs(mtime))
                    .ok_or_else(|| materialization("selected BCR mtime is out of range"))?;
                output
                    .set_times(FileTimes::new().set_modified(modified))
                    .map_err(|error| {
                        materialization(format!("setting selected BCR mtime for {path}: {error}"))
                    })?;
            }
        }
        tar.padding(size)?;
    }
    if !prefix_found {
        return Err(materialization("selected BCR strip_prefix was not found"));
    }
    Ok(())
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum EntryKind {
    Regular,
    Directory,
}

fn validate_path_text(bytes: &[u8]) -> Result<String, ArchiveMaterializationError> {
    if bytes.is_empty() || bytes.contains(&0) || bytes.contains(&b'\\') {
        return Err(materialization("selected BCR tar has an invalid path"));
    }
    let path = std::str::from_utf8(bytes)
        .map_err(|_| materialization("selected BCR tar path is not valid Unicode"))?;
    if path.len() > PATH_LIMIT {
        return Err(materialization("selected BCR tar path exceeds byte limit"));
    }
    Ok(path.to_owned())
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
            "selected BCR regular path has a trailing slash",
        ));
    }
    if path.is_empty() {
        return Ok(None);
    }
    if path.starts_with('/') {
        return Err(materialization("selected BCR tar path is absolute"));
    }
    let mut components = 0usize;
    for component in path.split('/') {
        if component.is_empty() || matches!(component, "." | "..") {
            return Err(materialization("selected BCR tar path is not normalized"));
        }
        components += 1;
    }
    if components > COMPONENT_LIMIT {
        return Err(materialization(
            "selected BCR tar path exceeds component limit",
        ));
    }
    Ok(Some(path.to_owned()))
}

fn admit_namespace(
    namespace: &HashMap<String, EntryKind>,
    path: &str,
    kind: EntryKind,
) -> Result<(), ArchiveMaterializationError> {
    if namespace.contains_key(path) {
        return Err(materialization("selected BCR tar has a duplicate path"));
    }
    let mut ancestor = path;
    while let Some((parent, _)) = ancestor.rsplit_once('/') {
        if namespace.get(parent) == Some(&EntryKind::Regular) {
            return Err(materialization(
                "selected BCR tar has a regular-file ancestor collision",
            ));
        }
        ancestor = parent;
    }
    if kind == EntryKind::Regular
        && namespace.keys().any(|entry| {
            entry
                .strip_prefix(path)
                .is_some_and(|suffix| suffix.starts_with('/'))
        })
    {
        return Err(materialization(
            "selected BCR tar has a regular-file descendant collision",
        ));
    }
    Ok(())
}

fn checked_payload(current: u64, size: u64) -> Result<u64, ArchiveMaterializationError> {
    current
        .checked_add(size)
        .filter(|total| *total <= PAYLOAD_LIMIT)
        .ok_or_else(|| materialization("selected BCR payload exceeds total limit"))
}

fn validate_global_pax_comment(bytes: &[u8]) -> Result<(), ArchiveMaterializationError> {
    if bytes.is_empty() || bytes.contains(&0) || std::str::from_utf8(bytes).is_err() {
        return Err(materialization(
            "selected BCR global PAX metadata must be nonempty NUL-free UTF-8",
        ));
    }
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        let space = bytes[cursor..]
            .iter()
            .position(|byte| *byte == b' ')
            .map(|offset| cursor + offset)
            .ok_or_else(|| materialization("selected BCR global PAX record has no length"))?;
        let digits = &bytes[cursor..space];
        if digits.is_empty()
            || !digits.iter().all(u8::is_ascii_digit)
            || (digits.len() > 1 && digits[0] == b'0')
        {
            return Err(materialization(
                "selected BCR global PAX record length is malformed",
            ));
        }
        let length = std::str::from_utf8(digits)
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .ok_or_else(|| materialization("selected BCR global PAX record length is invalid"))?;
        let end = cursor
            .checked_add(length)
            .filter(|end| *end <= bytes.len() && *end > space + 2)
            .ok_or_else(|| materialization("selected BCR global PAX record length is invalid"))?;
        let record = &bytes[space + 1..end];
        let body = record
            .strip_suffix(b"\n")
            .ok_or_else(|| materialization("selected BCR global PAX record lacks final LF"))?;
        let equals = body.iter().position(|byte| *byte == b'=').ok_or_else(|| {
            materialization("selected BCR global PAX record has no key separator")
        })?;
        if &body[..equals] != b"comment" {
            return Err(materialization(
                "selected BCR global PAX key is unsupported",
            ));
        }
        cursor = end;
    }
    Ok(())
}

fn read_capture(
    mut capture: tempfile::NamedTempFile,
    subject: &str,
    limit: u64,
    active: &dyn Fn() -> bool,
) -> Result<Vec<u8>, ArchiveMaterializationError> {
    let size = capture
        .as_file()
        .metadata()
        .map_err(|error| materialization(format!("sizing verified {subject} capture: {error}")))?
        .len();
    if size > limit {
        return Err(materialization(format!(
            "verified {subject} capture exceeds size limit"
        )));
    }
    if !active() {
        return Err(materialization("repository session is no longer active"));
    }
    capture
        .as_file_mut()
        .seek(SeekFrom::Start(0))
        .map_err(|error| materialization(format!("seeking verified {subject} capture: {error}")))?;
    let mut bytes = Vec::with_capacity(size as usize);
    capture
        .as_file_mut()
        .read_to_end(&mut bytes)
        .map_err(|error| materialization(format!("reading verified {subject} capture: {error}")))?;
    capture.close().map_err(|error| {
        materialization(format!("deleting verified {subject} capture: {error}"))
    })?;
    Ok(bytes)
}

fn place_overlay(
    capture: tempfile::NamedTempFile,
    root: &Path,
    destination: &str,
    active: &dyn Fn() -> bool,
) -> Result<(), ArchiveMaterializationError> {
    let bytes = read_capture(capture, "overlay", 64 * 1024 * 1024, active)?;
    let target = root.join(destination);
    let parent = target
        .parent()
        .ok_or_else(|| materialization("selected BCR overlay target has no parent"))?;
    std::fs::create_dir_all(parent).map_err(|error| {
        materialization(format!(
            "creating selected BCR overlay parent {destination}: {error}"
        ))
    })?;
    let mut staged = tempfile::NamedTempFile::new_in(parent).map_err(|error| {
        materialization(format!(
            "staging selected BCR overlay {destination}: {error}"
        ))
    })?;
    staged.as_file_mut().write_all(&bytes).map_err(|error| {
        materialization(format!(
            "writing selected BCR overlay {destination}: {error}"
        ))
    })?;
    staged.as_file_mut().flush().map_err(|error| {
        materialization(format!(
            "flushing selected BCR overlay {destination}: {error}"
        ))
    })?;
    set_mode(staged.path(), 0o755)?;
    match std::fs::symlink_metadata(&target) {
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err(materialization(
                "selected BCR overlay target is not a regular file",
            ));
        }
        Ok(_) => std::fs::remove_file(&target).map_err(|error| {
            materialization(format!(
                "removing prior overlay target {destination}: {error}"
            ))
        })?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(materialization(format!(
                "inspecting selected BCR overlay target {destination}: {error}"
            )));
        }
    }
    staged.persist(&target).map_err(|error| {
        materialization(format!(
            "placing selected BCR overlay {destination}: {}",
            error.error
        ))
    })?;
    Ok(())
}

fn place_module(
    mut capture: tempfile::NamedTempFile,
    root: &Path,
    active: &dyn Fn() -> bool,
) -> Result<(), ArchiveMaterializationError> {
    capture
        .as_file_mut()
        .seek(SeekFrom::Start(0))
        .map_err(|error| materialization(format!("seeking verified MODULE capture: {error}")))?;
    let mut staged = tempfile::NamedTempFile::new_in(root)
        .map_err(|error| materialization(format!("creating staged registry MODULE: {error}")))?;
    let size = capture
        .as_file()
        .metadata()
        .map_err(|error| materialization(format!("sizing verified registry MODULE: {error}")))?
        .len();
    if size > MODULE_LIMIT {
        return Err(materialization("registry MODULE exceeds size limit"));
    }
    if !active() {
        return Err(materialization("repository session is no longer active"));
    }
    std::io::copy(capture.as_file_mut(), staged.as_file_mut())
        .map_err(|error| materialization(format!("copying verified registry MODULE: {error}")))?;
    set_mode(staged.path(), 0o644)?;
    capture
        .close()
        .map_err(|error| materialization(format!("deleting verified MODULE capture: {error}")))?;
    let target = root.join("MODULE.bazel");
    match std::fs::symlink_metadata(&target) {
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err(materialization(
                "selected BCR MODULE target is not a regular file",
            ));
        }
        Ok(_) => std::fs::remove_file(&target)
            .map_err(|error| materialization(format!("removing archive MODULE.bazel: {error}")))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(materialization(format!(
                "inspecting archive MODULE.bazel: {error}"
            )));
        }
    }
    staged.persist(&target).map_err(|error| {
        materialization(format!("placing verified registry MODULE: {}", error.error))
    })?;
    Ok(())
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<(), ArchiveMaterializationError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).map_err(|error| {
        materialization(format!(
            "setting selected BCR mode for {}: {error}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<(), ArchiveMaterializationError> {
    Err(materialization(
        "selected BCR executable modes are unsupported on this platform",
    ))
}

fn validate_checksum(block: &[u8; BLOCK]) -> Result<(), ArchiveMaterializationError> {
    let header = tar::Header::from_byte_slice(block);
    let expected = header
        .cksum()
        .map_err(|error| materialization(format!("reading tar checksum: {error}")))?;
    let actual = block[..148]
        .iter()
        .chain([b' '; 8].iter())
        .chain(block[156..].iter())
        .map(|byte| u32::from(*byte))
        .sum::<u32>();
    if expected != actual {
        return Err(materialization("selected BCR tar header checksum mismatch"));
    }
    Ok(())
}

fn is_zero(block: &[u8; BLOCK]) -> bool {
    block.iter().all(|byte| *byte == 0)
}

struct BoundedTarReader<'a, R> {
    reader: R,
    active: &'a dyn Fn() -> bool,
    read: u64,
}

impl<'a, R: Read> BoundedTarReader<'a, R> {
    fn new(reader: R, active: &'a dyn Fn() -> bool) -> Self {
        Self {
            reader,
            active,
            read: 0,
        }
    }

    fn block(&mut self, operation: &str) -> Result<[u8; BLOCK], ArchiveMaterializationError> {
        let mut block = [0u8; BLOCK];
        self.read_exact(&mut block, operation)?;
        Ok(block)
    }

    fn read_exact(
        &mut self,
        mut bytes: &mut [u8],
        operation: &str,
    ) -> Result<(), ArchiveMaterializationError> {
        while !bytes.is_empty() {
            if !(self.active)() {
                return Err(materialization("repository session is no longer active"));
            }
            let remaining = DECOMPRESSED_LIMIT.saturating_sub(self.read);
            if remaining == 0 {
                return Err(materialization(
                    "selected BCR tar exceeds decompressed limit",
                ));
            }
            let allowed = bytes.len().min(remaining as usize).min(64 * 1024);
            let read = self
                .reader
                .read(&mut bytes[..allowed])
                .map_err(|error| materialization(format!("{operation}: {error}")))?;
            if read == 0 {
                return Err(materialization(format!(
                    "{operation}: unexpected end of stream"
                )));
            }
            self.read += read as u64;
            bytes = &mut bytes[read..];
        }
        Ok(())
    }

    fn copy_exact(
        &mut self,
        mut remaining: u64,
        output: &mut impl Write,
        path: &str,
    ) -> Result<(), ArchiveMaterializationError> {
        let mut buffer = [0u8; 64 * 1024];
        while remaining != 0 {
            let length = buffer.len().min(remaining as usize);
            self.read_exact(&mut buffer[..length], "reading tar file payload")?;
            output.write_all(&buffer[..length]).map_err(|error| {
                materialization(format!("writing selected BCR file {path}: {error}"))
            })?;
            remaining -= length as u64;
        }
        Ok(())
    }

    fn padding(&mut self, size: u64) -> Result<(), ArchiveMaterializationError> {
        let padding = (BLOCK as u64 - size % BLOCK as u64) % BLOCK as u64;
        let mut bytes = [0u8; BLOCK];
        self.read_exact(&mut bytes[..padding as usize], "reading tar entry padding")
    }

    fn finish_zero_padding(&mut self) -> Result<(), ArchiveMaterializationError> {
        let mut buffer = [0u8; 64 * 1024];
        loop {
            if !(self.active)() {
                return Err(materialization("repository session is no longer active"));
            }
            if self.read == DECOMPRESSED_LIMIT {
                let mut extra = [0u8; 1];
                return match self.reader.read(&mut extra) {
                    Ok(0) => Ok(()),
                    Ok(_) => Err(materialization(
                        "selected BCR tar exceeds decompressed limit",
                    )),
                    Err(error) => Err(materialization(format!("finishing gzip stream: {error}"))),
                };
            }
            let allowed = buffer.len().min((DECOMPRESSED_LIMIT - self.read) as usize);
            let read = self
                .reader
                .read(&mut buffer[..allowed])
                .map_err(|error| materialization(format!("finishing gzip stream: {error}")))?;
            if read == 0 {
                return Ok(());
            }
            self.read += read as u64;
            if buffer[..read].iter().any(|byte| *byte != 0) {
                return Err(materialization(
                    "selected BCR tar has nonzero trailing data",
                ));
            }
        }
    }
}

fn materialization(message: impl Into<String>) -> ArchiveMaterializationError {
    ArchiveMaterializationError::materialization(message)
}

#[cfg(test)]
#[path = "tests/repository_archive_realize_tests.rs"]
mod tests;
