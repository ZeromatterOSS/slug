use std::ffi::OsString;
use std::fs::File;
use std::io::Write;
use std::ops::Range;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use base64::Engine;
use sha2::Digest;
use sha2::Sha256;
use slug_bzlmod_v2::OverrideAttributeValue;
use slug_bzlmod_v2::RepoSpec;

use super::repository_io::ArchiveMaterializationError;
use super::repository_io::Materialized;
use super::repository_io::local_file_uri;
use super::repository_io::optional_string;
use super::repository_io::reject_extra_attributes;
use super::repository_io::required_string;

const BCR_KEYS: &[&str] = &[
    "urls",
    "integrity",
    "type",
    "strip_prefix",
    "remote_patches",
    "remote_file_urls",
    "remote_file_integrity",
    "remote_patch_strip",
    "remote_module_file_urls",
    "remote_module_file_integrity",
];
const BCR_ONLY_KEYS: &[&str] = &[
    "integrity",
    "remote_patches",
    "remote_file_urls",
    "remote_file_integrity",
    "remote_patch_strip",
    "remote_module_file_urls",
    "remote_module_file_integrity",
];

/// The complete selected-registry semantic view. It deliberately has no root,
/// runtime, generation, or transport capability; realization remains deferred.
#[derive(Debug)]
pub(super) enum ArchivePlan {
    LocalTar,
    SelectedBcrTarGz(SelectedBcrTarGz),
}

#[derive(Debug)]
pub(super) struct SelectedBcrTarGz {
    pub(super) urls: Box<[String]>,
    pub(super) integrity: [u8; 32],
    pub(super) module_url: String,
    pub(super) module_integrity: [u8; 32],
}

pub(super) fn parse_archive_plan(spec: &RepoSpec) -> Result<ArchivePlan, String> {
    let keys = spec
        .attributes
        .keys()
        .map(|key| key.as_str())
        .collect::<Vec<_>>();
    let bcr_candidate = keys.iter().any(|key| BCR_ONLY_KEYS.contains(key))
        || matches!(
            spec.attributes.get("type"),
            Some(OverrideAttributeValue::String(value)) if value == "tar.gz"
        );
    if !bcr_candidate {
        return Ok(ArchivePlan::LocalTar);
    }
    if keys.len() != BCR_KEYS.len() || BCR_KEYS.iter().any(|key| !keys.contains(key)) {
        return Err("http_archive has an unsupported attribute shape".into());
    }
    let urls = strings(spec, "urls")?;
    if urls.is_empty() || urls.iter().any(|url| !https(url)) {
        return Err("selected BCR http_archive urls must be nonempty HTTPS URLs".into());
    }
    if string(spec, "type")? != "tar.gz" {
        return Err("selected BCR http_archive type must be exactly tar.gz".into());
    }
    if string(spec, "strip_prefix")? != "" {
        return Err("selected BCR http_archive strip_prefix must be empty".into());
    }
    for key in [
        "remote_patches",
        "remote_file_urls",
        "remote_file_integrity",
    ] {
        match spec.attributes.get(key) {
            Some(OverrideAttributeValue::Map(values)) if values.is_empty() => {}
            _ => {
                return Err(format!(
                    "selected BCR http_archive {key} must be an empty map"
                ));
            }
        }
    }
    if !matches!(
        spec.attributes.get("remote_patch_strip"),
        Some(OverrideAttributeValue::Int(0))
    ) {
        return Err("selected BCR http_archive remote_patch_strip must be Int(0)".into());
    }
    let module_urls = strings(spec, "remote_module_file_urls")?;
    let [module_url] = module_urls.as_slice() else {
        return Err("selected BCR http_archive requires exactly one remote MODULE URL".into());
    };
    if !https(module_url) {
        return Err("selected BCR http_archive MODULE URL must be HTTPS".into());
    }
    Ok(ArchivePlan::SelectedBcrTarGz(SelectedBcrTarGz {
        urls: urls.into_boxed_slice(),
        integrity: sri(spec, "integrity")?,
        module_url: module_url.clone(),
        module_integrity: sri(spec, "remote_module_file_integrity")?,
    }))
}

pub(super) fn materialize_selected_bcr_capture(
    plan: &SelectedBcrTarGz,
    runtime: &tokio::runtime::Runtime,
    active: &dyn Fn() -> bool,
) -> Result<Materialized, ArchiveMaterializationError> {
    let archive = super::repository_archive_http::capture_selected_bcr(plan, runtime, active)?;
    super::repository_archive_realize::realize_selected_bcr(plan, archive, active, || {
        super::repository_archive_http::capture_selected_bcr_module(plan, runtime, active)
    })
}

fn string<'a>(spec: &'a RepoSpec, key: &str) -> Result<&'a str, String> {
    match spec.attributes.get(key) {
        Some(OverrideAttributeValue::String(value)) => Ok(value),
        _ => Err(format!("selected BCR http_archive {key} must be a string")),
    }
}

fn strings(spec: &RepoSpec, key: &str) -> Result<Vec<String>, String> {
    match spec.attributes.get(key) {
        Some(OverrideAttributeValue::Iterable(values)) => values
            .iter()
            .map(|value| match value {
                OverrideAttributeValue::String(value) => Ok(value.to_string()),
                _ => Err(format!(
                    "selected BCR http_archive {key} must contain strings"
                )),
            })
            .collect(),
        _ => Err(format!("selected BCR http_archive {key} must be a list")),
    }
}

fn sri(spec: &RepoSpec, key: &str) -> Result<[u8; 32], String> {
    let encoded = string(spec, key)?
        .strip_prefix("sha256-")
        .ok_or_else(|| format!("selected BCR http_archive {key} must be SHA-256 SRI"))?;
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| format!("selected BCR http_archive {key} has malformed SRI"))?
        .try_into()
        .map_err(|_| format!("selected BCR http_archive {key} must contain 32 bytes"))
}

fn https(value: &str) -> bool {
    url::Url::parse(value)
        .map(|url| url.scheme() == "https" && url.host_str().is_some())
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "tests/repository_archive_tests.rs"]
mod tests;
enum SavedChecksum {
    Valid(String),
    Malformed,
}

struct CapturedArchive {
    bytes: Vec<u8>,
    _artifact: tempfile::NamedTempFile,
}

trait ArchiveIo: ArchiveDestination {
    fn create_root(&mut self) -> std::io::Result<tempfile::TempDir>;
    fn create_capture(&mut self) -> std::io::Result<tempfile::NamedTempFile>;
    fn read_source(&mut self, source: &Path) -> std::io::Result<Vec<u8>>;
    fn write_capture(
        &mut self,
        capture: &mut tempfile::NamedTempFile,
        bytes: &[u8],
    ) -> std::io::Result<()>;
    fn flush_capture(&mut self, capture: &mut tempfile::NamedTempFile) -> std::io::Result<()>;
}

struct NativeArchiveIo;

impl ArchiveIo for NativeArchiveIo {
    fn create_root(&mut self) -> std::io::Result<tempfile::TempDir> {
        tempfile::tempdir()
    }

    fn create_capture(&mut self) -> std::io::Result<tempfile::NamedTempFile> {
        tempfile::NamedTempFile::new()
    }

    fn read_source(&mut self, source: &Path) -> std::io::Result<Vec<u8>> {
        std::fs::read(source)
    }

    fn write_capture(
        &mut self,
        capture: &mut tempfile::NamedTempFile,
        bytes: &[u8],
    ) -> std::io::Result<()> {
        capture.write_all(bytes)
    }

    fn flush_capture(&mut self, capture: &mut tempfile::NamedTempFile) -> std::io::Result<()> {
        capture.flush()
    }
}

impl ArchiveDestination for NativeArchiveIo {
    fn create_parent(&mut self, path: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn create_directory(&mut self, path: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn write_regular(&mut self, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        let mut output = File::create(path)?;
        output.write_all(bytes)
    }
}

fn materialize_archive_with(
    repo_spec: &RepoSpec,
    io: &mut impl ArchiveIo,
) -> Result<Materialized, ArchiveMaterializationError> {
    reject_extra_attributes(repo_spec, &["urls", "sha256", "type", "strip_prefix"])
        .map_err(|error| ArchiveMaterializationError::spec(error.message))?;
    let urls = repo_spec.attributes.get("urls").ok_or_else(|| {
        ArchiveMaterializationError::spec("http_archive requires exactly one file URL")
    })?;
    let OverrideAttributeValue::Iterable(urls) = urls else {
        return Err(ArchiveMaterializationError::spec(
            "http_archive urls must contain exactly one file URL",
        ));
    };
    let [OverrideAttributeValue::String(url)] = urls.as_ref() else {
        return Err(ArchiveMaterializationError::spec(
            "http_archive urls must contain exactly one file URL",
        ));
    };
    let archive =
        local_file_uri(url).map_err(|error| ArchiveMaterializationError::spec(error.message))?;
    if optional_string(repo_spec, "type")
        .map_err(|error| ArchiveMaterializationError::spec(error.message))?
        != Some("tar")
    {
        return Err(ArchiveMaterializationError::spec(
            "http_archive type must be exactly tar",
        ));
    }
    let strip_prefix = optional_string(repo_spec, "strip_prefix")
        .map_err(|error| ArchiveMaterializationError::spec(error.message))?
        .map(latin1_bytes);
    if strip_prefix
        .as_ref()
        .is_some_and(|prefix| prefix.contains(&0))
    {
        return Err(ArchiveMaterializationError::spec(
            "http_archive strip_prefix contains a NUL byte",
        ));
    }
    let prefix = strip_prefix
        .as_deref()
        .map(|value| normalize_raw_tar_path(value, native_path_flavor()))
        .transpose()?;
    if let Some(prefix) = prefix.as_deref() {
        validate_strip_prefix(prefix, native_path_flavor())?;
    }
    let expected_sha256 = required_string(repo_spec, "sha256")
        .map_err(|error| ArchiveMaterializationError::spec(error.message))?;
    let saved_checksum = if expected_sha256.len() == 64
        && expected_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        SavedChecksum::Valid(expected_sha256.to_owned())
    } else {
        SavedChecksum::Malformed
    };

    let root = io.create_root().map_err(|error| {
        ArchiveMaterializationError::materialization(format!(
            "creating archive materialization root: {error}"
        ))
    })?;
    let mut artifact = io.create_capture().map_err(|error| {
        ArchiveMaterializationError::materialization(format!(
            "creating temporary http_archive capture: {error}"
        ))
    })?;
    let bytes = io.read_source(&archive).map_err(|error| {
        ArchiveMaterializationError::transport(format!(
            "reading http_archive {}: {error}",
            archive.display()
        ))
    })?;
    io.write_capture(&mut artifact, &bytes).map_err(|error| {
        ArchiveMaterializationError::transport(format!(
            "writing temporary http_archive capture: {error}"
        ))
    })?;
    io.flush_capture(&mut artifact).map_err(|error| {
        ArchiveMaterializationError::transport(format!(
            "flushing temporary http_archive capture: {error}"
        ))
    })?;
    let captured = CapturedArchive {
        bytes,
        _artifact: artifact,
    };

    let SavedChecksum::Valid(expected_sha256) = saved_checksum else {
        return Err(ArchiveMaterializationError::spec(
            "http_archive sha256 must be an exact 64-character hexadecimal digest",
        ));
    };
    let actual_sha256 = format!("{:x}", Sha256::digest(&captured.bytes));
    if !actual_sha256.eq_ignore_ascii_case(&expected_sha256) {
        return Err(ArchiveMaterializationError::transport(
            "http_archive sha256 does not match the local tar",
        ));
    }

    let plan = inspect_and_plan_ustar(&captured.bytes, prefix.as_deref(), root.path())?;
    extract_ustar_plan(&captured.bytes, &plan, io)?;
    Ok(Materialized::Immutable {
        bytes: captured.bytes,
        root,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)] // Both flavors are exercised by host-pure tests.
enum PathFlavor {
    Unix,
    Windows,
}

#[cfg(windows)]
fn native_path_flavor() -> PathFlavor {
    PathFlavor::Windows
}

#[cfg(not(windows))]
fn native_path_flavor() -> PathFlavor {
    PathFlavor::Unix
}

fn latin1_bytes(value: &str) -> Vec<u8> {
    value
        .chars()
        .map(|character| u8::try_from(u32::from(character)).unwrap_or(b'?'))
        .collect()
}

fn validate_strip_prefix(
    prefix: &[Vec<u8>],
    flavor: PathFlavor,
) -> Result<(), ArchiveMaterializationError> {
    if prefix.is_empty() || join_raw_components(Path::new(""), prefix, flavor).is_err() {
        return Err(ArchiveMaterializationError::spec(
            "http_archive strip_prefix must normalize to a safe relative path",
        ));
    }
    Ok(())
}

fn normalize_raw_tar_path(
    value: &[u8],
    flavor: PathFlavor,
) -> Result<Vec<Vec<u8>>, ArchiveMaterializationError> {
    let is_separator = |byte| byte == b'/' || (flavor == PathFlavor::Windows && byte == b'\\');
    let drive_absolute = flavor == PathFlavor::Windows
        && value.len() >= 3
        && value[0].is_ascii_alphabetic()
        && value[1] == b':'
        && is_separator(value[2]);
    let absolute = value.first().is_some_and(|byte| is_separator(*byte)) || drive_absolute;
    let mut start = if drive_absolute { 3 } else { 0 };
    while start < value.len() && is_separator(value[start]) {
        start += 1;
    }
    let mut components: Vec<Vec<u8>> = Vec::new();
    for component in value[start..].split(|byte| is_separator(*byte)) {
        if component.is_empty() || component == b"." {
            continue;
        }
        if component == b".." {
            if components
                .last()
                .is_some_and(|last| last.as_slice() != b"..")
            {
                components.pop();
            } else if !absolute {
                components.push(component.to_vec());
            }
        } else {
            components.push(component.to_vec());
        }
    }
    Ok(components)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlannedUstarKind {
    Regular,
    Directory,
}

#[derive(Debug)]
struct PlannedUstarEntry {
    components: Vec<Vec<u8>>,
    path: PathBuf,
    payload: Range<usize>,
    kind: PlannedUstarKind,
}

#[derive(Debug, Default)]
struct UstarExtractionPlan {
    entries: Vec<PlannedUstarEntry>,
}

fn inspect_and_plan_ustar(
    bytes: &[u8],
    prefix: Option<&[Vec<u8>]>,
    root: &Path,
) -> Result<UstarExtractionPlan, ArchiveMaterializationError> {
    let flavor = native_path_flavor();
    inspect_and_plan_ustar_for_flavor(bytes, prefix, flavor, root)
}

fn inspect_and_plan_ustar_for_flavor(
    bytes: &[u8],
    prefix: Option<&[Vec<u8>]>,
    flavor: PathFlavor,
    root: &Path,
) -> Result<UstarExtractionPlan, ArchiveMaterializationError> {
    let mut plan = UstarExtractionPlan::default();
    let mut offset = 0usize;
    let mut found_prefix = false;
    while offset < bytes.len() {
        if bytes.len() - offset < 512 {
            break;
        }
        let header = &bytes[offset..offset + 512];
        if header.iter().all(|byte| *byte == 0) {
            break;
        }
        let size = parse_ustar_octal(&header[124..136])?;
        parse_ustar_octal(&header[148..156]).map_err(|_| {
            ArchiveMaterializationError::materialization(
                "http_archive tar entry has a malformed checksum field",
            )
        })?;
        reject_non_ustar_layout(header)?;
        let payload_start = offset + 512;
        let payload_end = payload_start.checked_add(size).ok_or_else(|| {
            ArchiveMaterializationError::materialization(
                "http_archive tar entry payload length overflows",
            )
        })?;
        if payload_end > bytes.len() {
            return Err(ArchiveMaterializationError::materialization(
                "http_archive tar entry payload is truncated",
            ));
        }
        let padding = (512 - size % 512) % 512;
        let next_offset = payload_end.checked_add(padding).ok_or_else(|| {
            ArchiveMaterializationError::materialization(
                "http_archive tar entry padding length overflows",
            )
        })?;
        if next_offset > bytes.len() {
            return Err(ArchiveMaterializationError::materialization(
                "http_archive tar entry padding is truncated",
            ));
        }

        let name = nul_terminated(&header[..100]);
        let raw_prefix = nul_terminated(&header[345..500]);
        let mut raw_path =
            Vec::with_capacity(raw_prefix.len() + usize::from(!raw_prefix.is_empty()) + name.len());
        if !raw_prefix.is_empty() {
            raw_path.extend_from_slice(raw_prefix);
            raw_path.push(b'/');
        }
        raw_path.extend_from_slice(name);
        let normalized = normalize_raw_tar_path(&raw_path, flavor)?;
        let selected = match prefix {
            None => Some(normalized.as_slice()),
            Some(prefix) if normalized.starts_with(prefix) => {
                found_prefix = true;
                Some(&normalized[prefix.len()..])
            }
            Some(_) => None,
        };
        if let Some(selected) = selected {
            let kind = match header[156] {
                b'5' => PlannedUstarKind::Directory,
                0 | b'0' if raw_path.ends_with(b"/") => PlannedUstarKind::Directory,
                0 | b'0' => PlannedUstarKind::Regular,
                _ => {
                    return Err(ArchiveMaterializationError::materialization(
                        "http_archive contains an unsupported tar entry type",
                    ));
                }
            };
            if !selected.is_empty() {
                let path = join_raw_components(root, selected, flavor)?;
                reject_namespace_collision(&plan.entries, selected, kind)?;
                plan.entries.push(PlannedUstarEntry {
                    components: selected.to_vec(),
                    path,
                    payload: payload_start..payload_end,
                    kind,
                });
            }
        }
        offset = next_offset;
    }
    if prefix.is_some() && !found_prefix {
        return Err(ArchiveMaterializationError::materialization(
            "http_archive strip_prefix was not found",
        ));
    }
    Ok(plan)
}

fn reject_non_ustar_layout(header: &[u8]) -> Result<(), ArchiveMaterializationError> {
    if &header[257..263] == b"ustar " || (&header[257..263] == b"ustar\0" && is_xstar(header)) {
        return Err(ArchiveMaterializationError::materialization(
            "http_archive contains an unsupported tar header layout",
        ));
    }
    Ok(())
}

fn is_xstar(header: &[u8]) -> bool {
    if &header[508..512] == b"tar\0" {
        return true;
    }
    if header[475] != 0
        && (header[156] != b'M' || ((header[464] & 0x80) == 0 && header[475] != b' '))
    {
        return false;
    }
    xstar_time_is_valid(&header[476..488]) && xstar_time_is_valid(&header[488..500])
}

fn xstar_time_is_valid(field: &[u8]) -> bool {
    field[0] & 0x80 != 0
        || (field[..field.len() - 1]
            .iter()
            .all(|byte| matches!(byte, b'0'..=b'7'))
            && matches!(field[field.len() - 1], 0 | b' '))
}

fn parse_ustar_octal(field: &[u8]) -> Result<usize, ArchiveMaterializationError> {
    if field.first() == Some(&0) {
        return Ok(0);
    }
    if field.first().is_some_and(|byte| byte & 0x80 != 0) {
        return Err(ArchiveMaterializationError::materialization(
            "http_archive tar entry uses an unsupported base-256 size",
        ));
    }
    let mut start = 0;
    while field.get(start) == Some(&b' ') {
        start += 1;
    }
    let mut end = field.len();
    while end > start && matches!(field[end - 1], 0 | b' ') {
        end -= 1;
    }
    let mut value = 0usize;
    for byte in &field[start..end] {
        if !matches!(byte, b'0'..=b'7') {
            return Err(ArchiveMaterializationError::materialization(
                "http_archive tar entry has a malformed size",
            ));
        }
        value = value
            .checked_mul(8)
            .and_then(|value| value.checked_add(usize::from(*byte - b'0')))
            .ok_or_else(|| {
                ArchiveMaterializationError::materialization(
                    "http_archive tar entry size overflows",
                )
            })?;
    }
    Ok(value)
}

fn nul_terminated(field: &[u8]) -> &[u8] {
    &field[..field
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(field.len())]
}

fn reject_namespace_collision(
    entries: &[PlannedUstarEntry],
    components: &[Vec<u8>],
    kind: PlannedUstarKind,
) -> Result<(), ArchiveMaterializationError> {
    for entry in entries {
        if entry.components == components {
            if entry.kind != kind {
                return Err(ArchiveMaterializationError::materialization(
                    "http_archive tar entries collide as file and directory",
                ));
            }
        } else if entry.kind == PlannedUstarKind::Regular
            && components.starts_with(&entry.components)
            || kind == PlannedUstarKind::Regular && entry.components.starts_with(components)
        {
            return Err(ArchiveMaterializationError::materialization(
                "http_archive tar entry collides with a regular-file ancestor",
            ));
        }
    }
    Ok(())
}

fn join_raw_components(
    root: &Path,
    components: &[Vec<u8>],
    flavor: PathFlavor,
) -> Result<PathBuf, ArchiveMaterializationError> {
    let mut result = root.to_path_buf();
    for component in components {
        if component.is_empty()
            || matches!(component.as_slice(), b"." | b"..")
            || component.contains(&0)
            || (flavor == PathFlavor::Windows
                && component
                    .iter()
                    .any(|byte| matches!(byte, b'/' | b'\\' | b':')))
        {
            return Err(ArchiveMaterializationError::materialization(
                "http_archive tar entry contains a non-normal path component",
            ));
        }
        let component = raw_os_string(component);
        let mut parsed = Path::new(&component).components();
        if !matches!(parsed.next(), Some(Component::Normal(value)) if value == component)
            || parsed.next().is_some()
        {
            return Err(ArchiveMaterializationError::materialization(
                "http_archive tar entry contains a non-normal OS component",
            ));
        }
        result.push(component);
    }
    if !result.starts_with(root) {
        return Err(ArchiveMaterializationError::materialization(
            "http_archive tar entry escapes the destination directory",
        ));
    }
    Ok(result)
}

#[cfg(unix)]
fn raw_os_string(bytes: &[u8]) -> OsString {
    use std::os::unix::ffi::OsStringExt;

    OsString::from_vec(bytes.to_vec())
}

#[cfg(windows)]
fn raw_os_string(bytes: &[u8]) -> OsString {
    OsString::from(
        bytes
            .iter()
            .map(|byte| char::from(*byte))
            .collect::<String>(),
    )
}

trait ArchiveDestination {
    fn create_parent(&mut self, path: &Path) -> std::io::Result<()>;
    fn create_directory(&mut self, path: &Path) -> std::io::Result<()>;
    fn write_regular(&mut self, path: &Path, bytes: &[u8]) -> std::io::Result<()>;
}

fn extract_ustar_plan(
    bytes: &[u8],
    plan: &UstarExtractionPlan,
    destination: &mut impl ArchiveDestination,
) -> Result<(), ArchiveMaterializationError> {
    for entry in &plan.entries {
        if let Some(parent) = entry.path.parent() {
            destination.create_parent(parent).map_err(|error| {
                ArchiveMaterializationError::materialization(format!(
                    "creating http_archive tar entry parent: {error}"
                ))
            })?;
        }
        let result = match entry.kind {
            PlannedUstarKind::Directory => destination.create_directory(&entry.path),
            PlannedUstarKind::Regular => {
                destination.write_regular(&entry.path, &bytes[entry.payload.clone()])
            }
        };
        result.map_err(|error| {
            ArchiveMaterializationError::materialization(format!(
                "extracting http_archive tar entry: {error}"
            ))
        })?;
    }
    Ok(())
}

pub(super) fn materialize_local_tar(
    repo_spec: &RepoSpec,
) -> Result<Materialized, ArchiveMaterializationError> {
    materialize_archive_with(repo_spec, &mut NativeArchiveIo)
}
