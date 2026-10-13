//! Terminal, bounded observation of the private generated repository tree.
use std::fs;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use sha2::Digest;
use sha2::Sha256;

use super::ArchiveMaterializationError;
use super::SourceServiceScope;
use super::check;
use super::io_error;
use super::source_service;
use super::spec;

#[derive(Clone, Copy)]
struct Limits {
    entries: usize,
    paths: usize,
    bytes: u64,
}
const LIMITS: Limits = Limits {
    entries: 131_072,
    paths: 4 * 1024 * 1024,
    bytes: 1024 * 1024 * 1024,
};
// Includes one filename and the temporary joining/reallocation of a <=4096
// byte path. Allocator overhead is outside these modeled credits, not an RSS claim.
const PATH_SCRATCH: usize = 16 * 1024;
const PATH_LIMIT: usize = 4096;

struct Frontier {
    paths: Vec<PathBuf>,
    discovered: usize,
    charged: usize,
    limits: Limits,
}
impl Frontier {
    fn push(&mut self, path: PathBuf) -> Result<(), ArchiveMaterializationError> {
        self.discovered = self
            .discovered
            .checked_add(1)
            .filter(|n| *n <= self.limits.entries)
            .ok_or_else(|| spec("archive output manifest exceeds entry budget"))?;
        self.charge(path.capacity())?;
        if self.paths.len() == self.paths.capacity() {
            let capacity = self
                .paths
                .capacity()
                .checked_mul(2)
                .unwrap_or(usize::MAX)
                .max(4)
                .min(self.limits.entries);
            // Charge the entire new allocation, conservatively retaining all
            // prior allocation charges across growth and popped entries.
            self.charge(
                capacity
                    .checked_mul(std::mem::size_of::<PathBuf>())
                    .ok_or_else(|| spec("archive manifest frontier size overflows"))?,
            )?;
            self.paths
                .try_reserve_exact(capacity - self.paths.len())
                .map_err(io_error)?;
        }
        self.paths.push(path);
        Ok(())
    }
    fn charge(&mut self, bytes: usize) -> Result<(), ArchiveMaterializationError> {
        self.charged = self
            .charged
            .checked_add(bytes)
            .filter(|n| *n <= self.limits.paths.saturating_sub(PATH_SCRATCH))
            .ok_or_else(|| spec("archive output manifest exceeds path storage budget"))?;
        Ok(())
    }
}

pub(super) fn final_manifest(
    root: &Path,
    active: &dyn Fn() -> bool,
    scope: Option<&Arc<SourceServiceScope>>,
) -> Result<Arc<str>, ArchiveMaterializationError> {
    capture(root, active, scope, LIMITS)
}

#[cfg(unix)]
fn capture(
    root: &Path,
    active: &dyn Fn() -> bool,
    scope: Option<&Arc<SourceServiceScope>>,
    limits: Limits,
) -> Result<Arc<str>, ArchiveMaterializationError> {
    use std::os::unix::fs::MetadataExt;
    check(active, scope)?;
    if root.as_os_str().len() > PATH_LIMIT {
        return Err(spec("archive manifest root path exceeds budget"));
    }
    let _frontier_credit = source_service::buffer(scope, limits.paths).map_err(io_error)?;
    let _buffer_credit = source_service::buffer(scope, 64 * 1024).map_err(io_error)?;
    let mut frontier = Frontier {
        paths: Vec::new(),
        discovered: 0,
        charged: 0,
        limits,
    };
    frontier.push(root.to_path_buf())?;
    let mut digest = Sha256::new();
    digest.update(b"slug.verified-generated-archive-output.v1\0");
    let mut buffer = [0u8; 64 * 1024];
    let mut remaining = limits.bytes;
    while let Some(path) = frontier.paths.pop() {
        check(active, scope)?;
        let metadata = fs::symlink_metadata(&path).map_err(io_error)?;
        let text = path
            .strip_prefix(root)
            .map_err(io_error)?
            .to_str()
            .ok_or_else(|| spec("non-Unicode archive output path"))?;
        digest.update((text.len() as u64).to_be_bytes());
        digest.update(text.as_bytes());
        digest.update((metadata.mode() & 0o777).to_be_bytes());
        if metadata.file_type().is_dir() {
            digest.update(b"dir\0");
            let start = frontier.paths.len();
            for entry in fs::read_dir(&path).map_err(io_error)? {
                check(active, scope)?;
                // Refuse breadth before creating a joined child path.
                if frontier.discovered == limits.entries {
                    return Err(spec("archive output manifest exceeds entry budget"));
                }
                let name = entry.map_err(io_error)?.file_name();
                if name.len() > 255 || path.as_os_str().len() + 1 + name.len() > PATH_LIMIT {
                    return Err(spec("archive manifest child path exceeds budget"));
                }
                frontier.push(path.join(name))?;
            }
            // One vector owns both the old stack and this directory's children.
            frontier.paths[start..].sort_unstable_by(|a, b| b.cmp(a));
        } else if metadata.file_type().is_file() {
            digest.update(b"file\0");
            digest.update(metadata.len().to_be_bytes());
            if metadata.len() > remaining {
                return Err(spec("archive output manifest exceeds byte budget"));
            }
            remaining -= metadata.len();
            let mut file = fs::File::open(&path).map_err(io_error)?;
            read_file(
                &mut file,
                metadata.len(),
                &mut buffer,
                &mut digest,
                active,
                scope,
            )?;
        } else {
            return Err(spec("unsupported archive output manifest entry"));
        }
    }
    Ok(format!(
        "verified-generated-archive:{}",
        hex::encode(digest.finalize())
    )
    .into())
}

fn read_file(
    file: &mut impl Read,
    mut size: u64,
    buffer: &mut [u8],
    digest: &mut Sha256,
    active: &dyn Fn() -> bool,
    scope: Option<&Arc<SourceServiceScope>>,
) -> Result<(), ArchiveMaterializationError> {
    while size != 0 {
        check(active, scope)?;
        let length = buffer.len().min(size.try_into().unwrap_or(usize::MAX));
        let count = file.read(&mut buffer[..length]).map_err(io_error)?;
        if count == 0 {
            return Err(spec("archive manifest file was truncated"));
        }
        digest.update(&buffer[..count]);
        size -= count as u64;
    }
    // One bounded, un-hashed EOF probe is admitted even at the exact byte quota.
    check(active, scope)?;
    if file.read(&mut [0u8; 1]).map_err(io_error)? != 0 {
        return Err(spec("archive manifest file grew after size observation"));
    }
    check(active, scope)
}

#[cfg(not(unix))]
fn capture(
    _: &Path,
    _: &dyn Fn() -> bool,
    _: Option<&Arc<SourceServiceScope>>,
    _: Limits,
) -> Result<Arc<str>, ArchiveMaterializationError> {
    Err(spec(
        "verified archive output manifests currently require Unix",
    ))
}

#[cfg(all(test, unix))]
#[path = "tests/repository_filesystem_manifest_tests.rs"]
mod tests;
