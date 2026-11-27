use std::fs::Metadata;
use std::fs::OpenOptions;
use std::io::Read;
use std::path::Path;

pub use slug_analysis_v2::CQUERY_STARLARK_SOURCE_MAX_BYTES;
use slug_analysis_v2::CqueryStarlarkError;
use slug_analysis_v2::render_cquery_starlark;

use super::CommandOutput;
use super::CqueryCommandEvaluation;
use super::PublishedCommand;
use super::TerminalOutput;

pub const CQUERY_STARLARK_NAME_MAX_BYTES: usize = 4096;
// The existing daemon frame ceiling also bounds the raw combined terminal stream.
const MAX_PUBLISHED_STDERR_BYTES: usize = 64 * 1024 * 1024;

pub fn publish_cquery_starlark_file<T>(output: CommandOutput<T>) -> PublishedCommand<T> {
    output.publish_with_stderr_limit(MAX_PUBLISHED_STDERR_BYTES)
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > CQUERY_STARLARK_NAME_MAX_BYTES || name.contains('\0') {
        return Err("invalid --starlark:file: invalid formatter file name".into());
    }
    Ok(())
}

pub fn validate_cquery_starlark_source(name: &str, source: &str) -> Result<(), String> {
    validate_name(name)?;
    if source.len() > CQUERY_STARLARK_SOURCE_MAX_BYTES {
        return Err("invalid --starlark:file: formatter source byte limit exceeded".into());
    }
    Ok(())
}

/// Capture using the caller's file authority. The returned path is a diagnostic
/// name and the captured bytes are the command's complete formatter input.
pub fn capture_cquery_starlark_file(path: &Path) -> Result<(String, String), String> {
    let name = path
        .to_str()
        .ok_or_else(|| "invalid --starlark:file: file name is not UTF-8".to_owned())?;
    validate_name(name)?;
    let failure = |reason: &dyn std::fmt::Display| {
        format!("invalid --starlark:file: failed to read {name:?}: {reason}")
    };
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // A FIFO may race a regular pathname. Open without waiting for a writer.
        options.custom_flags(libc::O_NONBLOCK);
    }
    if !path.metadata().map_err(|e| failure(&e))?.is_file() {
        return Err(failure(&"expected a regular file"));
    }
    let mut file = options.open(path).map_err(|e| failure(&e))?;
    let initial = file.metadata().map_err(|e| failure(&e))?;
    let limit = CQUERY_STARLARK_SOURCE_MAX_BYTES as u64;
    if !initial.is_file() {
        return Err(failure(&"expected a regular file"));
    }
    if initial.len() > limit {
        return Err(failure(&"formatter source byte limit exceeded"));
    }
    let mut bytes = Vec::with_capacity(initial.len() as usize);
    (&mut file)
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| failure(&e))?;
    if bytes.len() as u64 > limit {
        return Err(failure(&"formatter source byte limit exceeded"));
    }
    let end = file.metadata().map_err(|e| failure(&e))?;
    if initial.len() != bytes.len() as u64 || !same_version(&initial, &end) {
        return Err(failure(&"formatter file changed during capture"));
    }
    let source = String::from_utf8(bytes).map_err(|_| failure(&"formatter source is not UTF-8"))?;
    Ok((name.to_owned(), source))
}

fn same_version(before: &Metadata, after: &Metadata) -> bool {
    if before.len() != after.len() || !after.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        before.dev() == after.dev()
            && before.ino() == after.ino()
            && before.mtime() == after.mtime()
            && before.mtime_nsec() == after.mtime_nsec()
            && before.ctime() == after.ctime()
            && before.ctime_nsec() == after.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        before.modified().ok() == after.modified().ok()
    }
}

pub fn format_cquery_starlark_file(
    evaluation: &CqueryCommandEvaluation,
    name: &str,
    source: &str,
) -> TerminalOutput {
    if let Err(error) = validate_cquery_starlark_source(name, source) {
        return TerminalOutput::new(2, String::new(), format!("ERROR: {error}\n"));
    }
    // The diagnostic name came from the caller and may include control bytes.
    let name = name.escape_debug().to_string();
    match render_cquery_starlark(&name, source, evaluation.formatter_rows()) {
        Ok((stdout, stderr)) => TerminalOutput::new(0, stdout, stderr),
        Err(CqueryStarlarkError::Setup { stderr }) => TerminalOutput::new(1, String::new(), stderr),
        Err(CqueryStarlarkError::Request(error)) => {
            TerminalOutput::new(2, String::new(), format!("ERROR: {error}\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cquery_formatter_capture_checks_utf8_byte_limit_and_refreshes_each_time() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("format.star");
        let exact = "é".repeat(CQUERY_STARLARK_SOURCE_MAX_BYTES / 2);
        std::fs::write(&path, exact.as_bytes()).unwrap();
        assert_eq!(capture_cquery_starlark_file(&path).unwrap().1, exact);
        std::fs::write(&path, format!("{exact}x")).unwrap();
        assert!(
            capture_cquery_starlark_file(&path)
                .unwrap_err()
                .contains("byte limit")
        );
        std::fs::write(&path, [0xff]).unwrap();
        assert!(
            capture_cquery_starlark_file(&path)
                .unwrap_err()
                .contains("not UTF-8")
        );
        std::fs::write(&path, "format = 1").unwrap();
        assert_eq!(capture_cquery_starlark_file(&path).unwrap().1, "format = 1");
    }

    #[cfg(unix)]
    #[test]
    fn cquery_formatter_capture_refuses_fifo_without_waiting_for_a_writer() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("pipe");
        nix::unistd::mkfifo(&path, nix::sys::stat::Mode::S_IRUSR).unwrap();
        assert!(
            capture_cquery_starlark_file(&path)
                .unwrap_err()
                .contains("regular file")
        );
    }
}
