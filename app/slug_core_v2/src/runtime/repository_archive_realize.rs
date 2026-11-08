use std::collections::BTreeMap;
use std::fs::File;
use std::fs::FileTimes;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::ops::Bound::Included;
use std::ops::Bound::Unbounded;
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
use super::repository_io::source_service::SourceCounter;
use super::repository_io::source_service::SourceServiceScope;
use super::repository_io::source_service::{self};

#[path = "repository_archive_realize/destination.rs"]
mod destination;
use destination::Destination;

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
    scope: Option<&Arc<SourceServiceScope>>,
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
        scope,
    )?;
    archive
        .close()
        .map_err(|error| materialization(format!("deleting verified archive capture: {error}")))?;
    for (overlay, capture) in plan.overlays.iter().zip(overlays) {
        place_overlay(capture, root.path(), &overlay.destination, active, scope)?;
    }
    for capture in patches {
        let _buffer = scope
            .map(|s| {
                let size = capture
                    .as_file()
                    .metadata()
                    .map_err(|e| materialization(e.to_string()))?
                    .len();
                s.reserve_buffer_at(
                    usize::try_from(size).map_err(|_| materialization("patch size overflow"))?,
                    source_service::SourceBufferSite::PatchCapture,
                )
                .map_err(|e| materialization(e.to_string()))
            })
            .transpose()?;
        let bytes = read_capture(capture, "patch", 8 * 1024 * 1024, active, scope)?;
        super::repository_archive_patch::apply_selected_bcr_patch(
            root.path(),
            &bytes,
            plan.patch_strip,
            active,
            scope,
        )?;
    }
    place_module(module, root.path(), active, scope)?;
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
    scope: Option<&Arc<SourceServiceScope>>,
) -> Result<(), ArchiveMaterializationError> {
    extract_to(
        capture,
        &destination::PathDestination(root),
        strip_prefix,
        active,
        scope,
    )
}

#[cfg(target_os = "linux")]
pub(super) fn extract_selected_bcr_at(
    capture: &File,
    root: &super::repository_root_io::RootIo<'_>,
    strip_prefix: Option<&str>,
    active: &dyn Fn() -> bool,
    scope: Option<&Arc<SourceServiceScope>>,
) -> Result<(), ArchiveMaterializationError> {
    extract_to(capture, root, strip_prefix, active, scope)
}

fn extract_to(
    capture: &File,
    root: &impl Destination,
    strip_prefix: Option<&str>,
    active: &dyn Fn() -> bool,
    scope: Option<&Arc<SourceServiceScope>>,
) -> Result<(), ArchiveMaterializationError> {
    let mut capture = capture
        .try_clone()
        .map_err(|error| materialization(format!("opening verified archive capture: {error}")))?;
    capture
        .seek(SeekFrom::Start(0))
        .map_err(|error| materialization(format!("seeking verified archive capture: {error}")))?;
    let decoder = MultiGzDecoder::new(capture);
    let _decoder_buffer_credit =
        source_service::buffer(scope, 64 * 1024 + 1024).map_err(materialization)?;
    let mut tar = BoundedTarReader::new(decoder, active, scope);
    let mut namespace = BTreeMap::<String, EntryKind>::new();
    let mut symlinks = Vec::new();
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
        } else if entry_type.is_symlink() {
            EntryKind::Symlink
        } else {
            return Err(materialization(
                "selected BCR tar contains an unsupported entry type",
            ));
        };
        let mode = header
            .mode()
            .map_err(|error| materialization(format!("reading tar mode: {error}")))?;
        if (kind == EntryKind::Directory && !matches!(mode, 0o750 | 0o755 | 0o775))
            || (kind == EntryKind::Regular
                && !matches!(mode, 0o444 | 0o555 | 0o640 | 0o644 | 0o664 | 0o755 | 0o775))
            || (kind == EntryKind::Symlink && mode != 0o777)
        {
            return Err(materialization("selected BCR unsupported entry mode"));
        }
        if kind == EntryKind::Symlink && size != 0 {
            return Err(materialization("selected BCR symbolic link has a payload"));
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
        match kind {
            EntryKind::Directory => {
                if size != 0 {
                    return Err(materialization("selected BCR directory has a payload"));
                }
                root.directory(&path)?;
            }
            EntryKind::Regular => {
                if size > ENTRY_LIMIT {
                    return Err(materialization("selected BCR entry exceeds size limit"));
                }
                payload = checked_payload(payload, size)?;
                root.parent(&path)?;
                let mut output = root.regular(&path)?;
                tar.copy_exact(size, &mut output, &path)?;
                output.flush().map_err(|error| {
                    materialization(format!("flushing selected BCR file {path}: {error}"))
                })?;
                root.mode(&path, &output, mode | 0o400)?;
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
            EntryKind::Symlink => {
                let target = validate_path_text(&header.link_name_bytes().unwrap_or_default())?;
                let resolved = relative_link_destination(&path, &target)?;
                root.parent(&path)?;
                symlinks.push((path, target, resolved));
            }
        }
        tar.padding(size)?;
    }
    if !prefix_found {
        return Err(materialization("selected BCR strip_prefix was not found"));
    }
    install_symlinks(root, &namespace, &symlinks, active)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum EntryKind {
    Regular,
    Directory,
    Symlink,
}

// A preserved relative target may begin with dot/parent components, but its
// ordinary tail must be normalized. Interior cancellation could traverse an
// absent directory or a regular file despite a valid normalized endpoint.
fn relative_link_destination(
    path: &str,
    target: &str,
) -> Result<String, ArchiveMaterializationError> {
    if target.starts_with('/') {
        return Err(materialization(
            "selected BCR symbolic link target is absolute",
        ));
    }
    let mut components = path.rsplit_once('/').map_or_else(Vec::new, |(parent, _)| {
        parent.split('/').collect::<Vec<_>>()
    });
    let mut ordinary_tail = false;
    for component in target.split('/') {
        match component {
            "" => {
                return Err(materialization(
                    "selected BCR symbolic link target is not normalized",
                ));
            }
            "." | ".." if ordinary_tail => {
                return Err(materialization(
                    "selected BCR symbolic link has interior dot components",
                ));
            }
            "." => {}
            ".." => {
                if components.pop().is_none() {
                    return Err(materialization(
                        "selected BCR symbolic link escapes stripped root",
                    ));
                }
            }
            _ => {
                ordinary_tail = true;
                components.push(component);
            }
        }
    }
    if !ordinary_tail || components.len() > COMPONENT_LIMIT {
        return Err(materialization(
            "selected BCR symbolic link target has no bounded file tail",
        ));
    }
    let resolved = components.join("/");
    if resolved.len() > PATH_LIMIT {
        return Err(materialization(
            "selected BCR symbolic link target exceeds resolved path limit",
        ));
    }
    Ok(resolved)
}

fn install_symlinks(
    root: &impl Destination,
    namespace: &BTreeMap<String, EntryKind>,
    symlinks: &[(String, String, String)],
    active: &dyn Fn() -> bool,
) -> Result<(), ArchiveMaterializationError> {
    for (_, _, resolved) in symlinks {
        if namespace.get(resolved) != Some(&EntryKind::Regular) {
            return Err(materialization(
                "selected BCR symbolic link target is not an extracted regular file",
            ));
        }
    }
    for (path, target, _) in symlinks {
        if !active() {
            return Err(materialization("repository session is no longer active"));
        }
        root.link(path, target)?;
    }
    Ok(())
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
    namespace: &BTreeMap<String, EntryKind>,
    path: &str,
    kind: EntryKind,
) -> Result<(), ArchiveMaterializationError> {
    if namespace.contains_key(path) {
        return Err(materialization("selected BCR tar has a duplicate path"));
    }
    let mut ancestor = path;
    while let Some((parent, _)) = ancestor.rsplit_once('/') {
        match namespace.get(parent) {
            Some(EntryKind::Regular) => {
                return Err(materialization(
                    "selected BCR tar has a regular-file ancestor collision",
                ));
            }
            Some(EntryKind::Symlink) => {
                return Err(materialization(
                    "selected BCR tar has a symbolic-link ancestor collision",
                ));
            }
            _ => {}
        }
        ancestor = parent;
    }
    // Extraction already validated this UTF-8 path and its PATH_LIMIT bound.
    // Start at the slash: a near-prefix sibling such as `a!` must not hide `a/b`.
    let mut prefix = [0; PATH_LIMIT + 1];
    prefix[..path.len()].copy_from_slice(path.as_bytes());
    prefix[path.len()] = b'/';
    let prefix = std::str::from_utf8(&prefix[..path.len() + 1])
        .expect("validated archive path with appended slash");
    if kind != EntryKind::Directory
        && namespace
            .range::<str, _>((Included(prefix), Unbounded))
            .next()
            .is_some_and(|(entry, _)| entry.starts_with(prefix))
    {
        return Err(materialization(if kind == EntryKind::Regular {
            "selected BCR tar has a regular-file descendant collision"
        } else {
            "selected BCR tar has a symbolic-link descendant collision"
        }));
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
    capture: tempfile::NamedTempFile,
    subject: &str,
    limit: u64,
    active: &dyn Fn() -> bool,
    scope: Option<&Arc<SourceServiceScope>>,
) -> Result<Vec<u8>, ArchiveMaterializationError> {
    read_capture_with_reserve(
        capture,
        subject,
        limit,
        active,
        scope,
        Vec::try_reserve_exact,
    )
}

fn read_capture_with_reserve(
    mut capture: tempfile::NamedTempFile,
    subject: &str,
    limit: u64,
    active: &dyn Fn() -> bool,
    scope: Option<&Arc<SourceServiceScope>>,
    reserve: impl FnOnce(&mut Vec<u8>, usize) -> Result<(), std::collections::TryReserveError>,
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
    // This copy only fills an already admitted Vec, so it deliberately does
    // not count WrittenBytes. Its fixed scratch still belongs to this scope.
    let _copy_scratch_credit = source_service::buffer(scope, 64 * 1024).map_err(materialization)?;
    let capacity = if scope.is_some() {
        usize::try_from(size).map_err(|_| materialization("verified capture size overflow"))?
    } else {
        size as usize
    };
    let mut bytes = if let Some(scope) = scope {
        let mut bytes = Vec::new();
        reserve(&mut bytes, capacity).map_err(|_| {
            materialization(
                scope
                    .poison(source_service::SourceServiceError::Allocation)
                    .to_string(),
            )
        })?;
        // Spare capacity must fit the caller's existing payload credit.
        if bytes.capacity() > capacity {
            return Err(materialization(
                scope
                    .poison(source_service::SourceServiceError::Allocation)
                    .to_string(),
            ));
        }
        bytes
    } else {
        Vec::with_capacity(capacity)
    };
    if scope.is_none() {
        capture
            .as_file_mut()
            .read_to_end(&mut bytes)
            .map_err(|e| materialization(e.to_string()))?;
    } else {
        let copied = source_service::copy_counted(
            capture.as_file_mut(),
            &mut bytes,
            size,
            &|| active() && source_service::check(scope).is_ok(),
            None,
        )
        .map_err(|e| materialization(e.to_string()))?;
        source_service::check(scope).map_err(materialization)?;
        if copied != size {
            return Err(materialization("verified capture size changed"));
        }
    }
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
    scope: Option<&Arc<SourceServiceScope>>,
) -> Result<(), ArchiveMaterializationError> {
    let mut captured = Some(capture);
    let bytes = if scope.is_none() {
        Some(read_capture(
            captured.take().unwrap(),
            "overlay",
            64 * 1024 * 1024,
            active,
            None,
        )?)
    } else {
        None
    };
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
    if let Some(bytes) = bytes {
        staged.as_file_mut().write_all(&bytes).map_err(|error| {
            materialization(format!(
                "writing selected BCR overlay {destination}: {error}"
            ))
        })?;
    } else {
        let mut capture = captured.take().unwrap();
        capture
            .as_file_mut()
            .seek(SeekFrom::Start(0))
            .map_err(|e| materialization(e.to_string()))?;
        source_service::copy_counted(
            capture.as_file_mut(),
            staged.as_file_mut(),
            64 * 1024 * 1024,
            active,
            scope,
        )
        .map_err(|e| materialization(e.to_string()))?;
        capture
            .close()
            .map_err(|e| materialization(e.to_string()))?;
    }
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
    scope: Option<&Arc<SourceServiceScope>>,
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
    if scope.is_none() {
        std::io::copy(capture.as_file_mut(), staged.as_file_mut())
            .map_err(|e| materialization(e.to_string()))?;
    } else {
        source_service::copy_counted(
            capture.as_file_mut(),
            staged.as_file_mut(),
            MODULE_LIMIT,
            active,
            scope,
        )
        .map_err(|e| materialization(e.to_string()))?;
    }
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
    scope: Option<&'a Arc<SourceServiceScope>>,
    read: u64,
}

impl<'a, R: Read> BoundedTarReader<'a, R> {
    fn new(
        reader: R,
        active: &'a dyn Fn() -> bool,
        scope: Option<&'a Arc<SourceServiceScope>>,
    ) -> Self {
        Self {
            reader,
            active,
            scope,
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
            let read = source_service::read_counted(
                &mut self.reader,
                &mut bytes[..allowed],
                self.active,
                self.scope,
                SourceCounter::DecodedBytes,
            )
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
            source_service::write_counted(output, &buffer[..length], self.active, self.scope)
                .map_err(|error| {
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
                return match source_service::read_counted(
                    &mut self.reader,
                    &mut extra,
                    self.active,
                    self.scope,
                    SourceCounter::DecodedBytes,
                ) {
                    Ok(0) => Ok(()),
                    Ok(_) => Err(materialization(
                        "selected BCR tar exceeds decompressed limit",
                    )),
                    Err(error) => Err(materialization(format!("finishing gzip stream: {error}"))),
                };
            }
            let allowed = buffer.len().min((DECOMPRESSED_LIMIT - self.read) as usize);
            let read = source_service::read_counted(
                &mut self.reader,
                &mut buffer[..allowed],
                self.active,
                self.scope,
                SourceCounter::DecodedBytes,
            )
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
