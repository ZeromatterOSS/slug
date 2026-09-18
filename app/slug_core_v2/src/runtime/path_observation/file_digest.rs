//! Fixed-memory native content observation. Handles and scratch never enter DICE.

use std::io;
use std::io::Read;

use sha2::Digest;
use sha2::Sha256;
use slug_workspace_v2::FileContentDigest;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationError;

use super::PrimaryFailure;
#[cfg(unix)]
use super::retry_interrupted as retry;
#[cfg(windows)]
use super::windows_pure::retry_interrupted as retry;

const BUFFER_BYTES: usize = 64 * 1024;

fn add_size(size: u64, read: usize) -> io::Result<u64> {
    size.checked_add(read as u64)
        .filter(|size| *size <= i64::MAX as u64)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "file exceeds REAPI size domain"))
}

fn digest_reader(reader: &mut impl Read) -> io::Result<FileContentDigest> {
    let mut hasher = Sha256::new();
    let mut buffer = [0; BUFFER_BYTES];
    let mut size = 0;
    loop {
        let read = match reader.read(&mut buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if read == 0 {
            break;
        }
        size = add_size(size, read)?;
        hasher.update(&buffer[..read]);
    }
    Ok(FileContentDigest::new(hasher.finalize().into(), size)
        .expect("checked byte count fits REAPI"))
}

fn classify(error: io::Error) -> PrimaryFailure {
    #[cfg(unix)]
    {
        let observed = super::path_io_error(&error);
        if super::is_missing_error(&error) || error.kind() == io::ErrorKind::IsADirectory {
            PrimaryFailure::Refine(observed)
        } else {
            PrimaryFailure::Final(observed)
        }
    }
    #[cfg(windows)]
    {
        super::windows_pure::classify_file_bytes_error(
            error.kind(),
            error.raw_os_error().map(|raw| raw as u32),
        )
    }
}

pub(super) fn observe(path: &NormalizedAbsolutePath) -> Result<FileContentDigest, PrimaryFailure> {
    let mut options = std::fs::File::options();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // A regular file can race to a FIFO after resolution. Open without blocking,
        // then check the opened object before attempting any read.
        options.custom_flags(nix::libc::O_NONBLOCK);
    }
    let mut file = retry(|| options.open(path.as_path())).map_err(classify)?;
    let metadata = retry(|| file.metadata()).map_err(classify)?;
    if !metadata.is_file() {
        return Err(PrimaryFailure::Final(PathObservationError::WrongKind {
            expected: PathNodeKind::RegularFile,
            actual: if metadata.is_dir() {
                PathNodeKind::Directory
            } else {
                PathNodeKind::SpecialFile
            },
        }));
    }
    digest_reader(&mut file).map_err(classify)
}

#[cfg(test)]
mod tests;
