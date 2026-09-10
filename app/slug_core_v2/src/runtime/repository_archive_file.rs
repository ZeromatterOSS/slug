use std::path::PathBuf;

/// The selected-BCR file subset is deliberately narrower than URL parsing.
/// Inspect raw spelling first: url normalizes dot segments before to_file_path.
pub(super) fn file_path(value: &str) -> Result<PathBuf, String> {
    let invalid = || "BCR file URL is outside the admitted Linux path grammar".to_owned();
    if !cfg!(target_os = "linux")
        || value.len() > 16384
        || !value
            .get(..8)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file:///"))
        || value
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '\\' | '?' | '#'))
    {
        return Err(invalid());
    }
    let raw_path = &value[7..];
    if raw_path.starts_with("//") || raw_path.ends_with('/') {
        return Err(invalid());
    }
    let bytes = raw_path.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let escape = bytes.get(index + 1..index + 3).ok_or_else(invalid)?;
            let high = (escape[0] as char).to_digit(16).ok_or_else(invalid)?;
            let low = (escape[1] as char).to_digit(16).ok_or_else(invalid)?;
            if matches!((high * 16 + low) as u8, b'/' | b'\\') {
                return Err(invalid());
            }
            index += 3;
        } else {
            index += 1;
        }
    }
    for segment in raw_path.split('/') {
        // Decode only dots for the pre-normalization guard, not the pathname.
        let dots = segment.replace("%2e", ".").replace("%2E", ".");
        if matches!(dots.as_str(), "." | "..") {
            return Err(invalid());
        }
    }
    let parsed = url::Url::parse(value).map_err(|_| invalid())?;
    let path = parsed.to_file_path().map_err(|_| invalid())?;
    let text = path.to_str().ok_or_else(invalid)?;
    let first = text.trim_start_matches('/').as_bytes();
    if !path.is_absolute()
        || text.len() > 4095
        || text.bytes().any(|b| b.is_ascii_control() || b == b'\\')
        || (first.first().is_some_and(u8::is_ascii_alphabetic)
            && first.get(1).is_some_and(|b| matches!(b, b':' | b'|')))
    {
        return Err(invalid());
    }
    Ok(path)
}

#[cfg(not(target_os = "linux"))]
pub(super) fn capture_file(
    _original: &str,
    _capture: tempfile::NamedTempFile,
    _integrity: [u8; 32],
    _limit: u64,
    _subject: &str,
    _active: &dyn Fn() -> bool,
) -> Result<tempfile::NamedTempFile, String> {
    Err("BCR file capture is supported only on Linux".into())
}

#[cfg(target_os = "linux")]
pub(super) fn capture_file(
    original: &str,
    mut capture: tempfile::NamedTempFile,
    integrity: [u8; 32],
    limit: u64,
    subject: &str,
    active: &dyn Fn() -> bool,
) -> Result<tempfile::NamedTempFile, String> {
    use std::fs::OpenOptions;
    use std::io::Read;
    use std::io::Write;
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::OpenOptionsExt;

    use sha2::Digest;
    use sha2::Sha256;

    let check_active = || {
        if active() {
            Ok(())
        } else {
            Err("repository session is no longer active".to_owned())
        }
    };
    let oversized = || format!("BCR {subject} exceeds {limit} byte capture limit");
    let path = file_path(original)?;
    check_active()?;
    // O_PATH does not open the object for data, even for a FIFO or device.
    // A symlink may select an inode; SRI, not its mutable pathname, binds bytes.
    let pin = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_PATH | libc::O_CLOEXEC)
        .open(path)
        .map_err(|error| format!("pinning BCR {subject} file: {error}"))?;
    let pinned = pin
        .metadata()
        .map_err(|error| format!("stat pinned file: {error}"))?;
    if !pinned.is_file() {
        return Err(format!("BCR {subject} source is not a regular file"));
    }
    if pinned.len() > limit {
        return Err(oversized());
    }
    check_active()?;
    // Reopen the held descriptor, never the original path. No procfs fallback.
    let mut input = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(format!("/proc/self/fd/{}", pin.as_raw_fd()))
        .map_err(|error| format!("opening pinned BCR {subject} file: {error}"))?;
    let readable = input
        .metadata()
        .map_err(|error| format!("stat readable file: {error}"))?;
    if !readable.is_file() || readable.dev() != pinned.dev() || readable.ino() != pinned.ino() {
        return Err(format!("BCR {subject} pinned file identity changed"));
    }
    check_active()?;
    if readable.len() > limit {
        return Err(oversized());
    }
    // Regular-file syscalls need responsive local storage; O_NONBLOCK cannot
    // interrupt a stuck filesystem. Cancellation is cooperative between reads.
    let mut buffer = [0u8; 64 * 1024];
    let mut total = 0u64;
    let mut hasher = Sha256::new();
    loop {
        check_active()?;
        let count = match input.read(&mut buffer) {
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(format!("reading BCR {subject} file: {error}")),
        };
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .filter(|total| *total <= limit)
            .ok_or_else(oversized)?;
        capture
            .write_all(&buffer[..count])
            .map_err(|error| format!("writing capture: {error}"))?;
        hasher.update(&buffer[..count]);
    }
    check_active()?;
    if hasher.finalize().as_slice() != integrity {
        return Err(format!("BCR {subject} SHA-256 SRI mismatch"));
    }
    capture
        .flush()
        .map_err(|error| format!("flushing capture: {error}"))?;
    check_active()?;
    Ok(capture)
}

#[cfg(all(test, target_os = "linux"))]
#[path = "tests/repository_archive_file_tests.rs"]
mod tests;
