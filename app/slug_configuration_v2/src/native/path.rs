use std::error::Error;
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;

use super::host::HostPathFlavor;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative, Dupe)]
pub struct NormalizedBazelPath(Arc<NormalizedBazelPathData>);

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
struct NormalizedBazelPathData {
    flavor: HostPathFlavor,
    spelling: CompactString,
    absolute: bool,
}

impl NormalizedBazelPath {
    pub fn new(flavor: HostPathFlavor, path: &str) -> Result<Self, BazelPathError> {
        let windows = matches!(flavor, HostPathFlavor::Windows);
        if windows && path.split(['/', '\\']).any(is_windows_short_path) {
            return Err(BazelPathError::WindowsShortPathRequiresObservation { path: path.into() });
        }
        let (spelling, absolute) = normalize_segments(path, windows);
        Ok(Self(Arc::new(NormalizedBazelPathData {
            flavor,
            spelling: spelling.into(),
            absolute,
        })))
    }

    pub fn as_str(&self) -> &str {
        self.0.spelling.as_str()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative, Dupe)]
pub struct NormalizedAbsoluteBazelPath(NormalizedBazelPath);

impl NormalizedAbsoluteBazelPath {
    pub fn new(flavor: HostPathFlavor, path: &str) -> Result<Self, BazelPathError> {
        Self::try_from(NormalizedBazelPath::new(flavor, path)?)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl TryFrom<NormalizedBazelPath> for NormalizedAbsoluteBazelPath {
    type Error = BazelPathError;

    fn try_from(path: NormalizedBazelPath) -> Result<Self, Self::Error> {
        if !path.0.absolute {
            return Err(BazelPathError::NotAbsolute {
                path: path.as_str().into(),
            });
        }
        Ok(Self(path))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BazelPathError {
    WindowsShortPathRequiresObservation { path: CompactString },
    NotAbsolute { path: CompactString },
}

impl fmt::Display for BazelPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WindowsShortPathRequiresObservation { path } => write!(
                formatter,
                "Windows short path requires a Host filesystem observation: {path}"
            ),
            Self::NotAbsolute { path } => {
                write!(formatter, "absolute path required, got: {path}")
            }
        }
    }
}

impl Error for BazelPathError {}

fn normalize_segments(path: &str, windows: bool) -> (String, bool) {
    let bytes = path.as_bytes();
    let rooted = bytes
        .first()
        .is_some_and(|value| *value == b'/' || (windows && *value == b'\\'));
    let drive = windows
        && bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\');
    let absolute = rooted || drive;
    let needs_normalize = (windows && path.contains('\\'))
        || path.contains("//")
        || path.split('/').any(|segment| matches!(segment, "." | ".."))
        || (path.len() > 1 && path.ends_with('/'));
    if !needs_normalize {
        return (path.to_owned(), absolute);
    }

    let mut raw = path
        .split(|character| character == '/' || (windows && character == '\\'))
        .filter(|segment| !segment.is_empty());
    if drive {
        let _ = raw.next();
    }
    let mut segments = Vec::new();
    for segment in raw {
        match segment {
            "." => {}
            ".." if segments.last().is_some_and(|last| *last != "..") => {
                segments.pop();
            }
            ".." if absolute => {}
            _ => segments.push(segment),
        }
    }

    let prefix = if drive {
        format!("{}:/", char::from(bytes[0]).to_ascii_uppercase())
    } else if rooted {
        "/".to_owned()
    } else {
        String::new()
    };
    (format!("{prefix}{}", segments.join("/")), absolute)
}

fn is_windows_short_path(segment: &str) -> bool {
    let chars = segment.chars().collect::<Vec<_>>();
    if chars.len() > 12 {
        return false;
    }
    chars.iter().enumerate().any(|(tilde, value)| {
        if *value != '~' || !(1..=6).contains(&tilde) {
            return false;
        }
        let tail = &chars[tilde + 1..];
        let digits = tail
            .iter()
            .take_while(|value| value.is_ascii_digit())
            .count();
        if !(1..=6).contains(&digits) || tilde + digits >= 8 {
            return false;
        }
        let extension = &tail[digits..];
        extension.is_empty() || (extension[0] == '.' && extension.len() <= 4)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_unix_and_windows_without_cross_flavor_aliases() {
        let unix = NormalizedBazelPath::new(HostPathFlavor::Unix, "/a//b/../c").unwrap();
        assert_eq!(unix.as_str(), "/a/c");
        assert!(NormalizedAbsoluteBazelPath::try_from(unix.clone()).is_ok());
        let windows = NormalizedBazelPath::new(HostPathFlavor::Windows, "d:\\a\\.\\c").unwrap();
        assert_eq!(windows.as_str(), "D:/a/c");
        assert!(NormalizedAbsoluteBazelPath::try_from(windows.clone()).is_ok());
        assert_ne!(
            unix,
            NormalizedBazelPath::new(HostPathFlavor::Windows, "/a/c").unwrap()
        );
        assert_eq!(
            NormalizedBazelPath::new(HostPathFlavor::Windows, "c:/plain/path")
                .unwrap()
                .as_str(),
            "c:/plain/path"
        );
    }

    #[test]
    fn clamps_absolute_parents_and_preserves_relative_parents() {
        assert_eq!(
            NormalizedBazelPath::new(HostPathFlavor::Unix, "/../../a")
                .unwrap()
                .as_str(),
            "/a"
        );
        assert_eq!(
            NormalizedBazelPath::new(HostPathFlavor::Unix, "../../a")
                .unwrap()
                .as_str(),
            "../../a"
        );
        assert!(NormalizedAbsoluteBazelPath::new(HostPathFlavor::Unix, "relative/path").is_err());
    }

    #[test]
    fn rejects_windows_short_candidates_without_observing_the_filesystem() {
        for path in [
            "C:/PROGRA~1/tool.exe",
            "c:\\Users\\RUNNER~2\\tool.exe",
            "relative/ABCD~123.TXT",
        ] {
            assert!(matches!(
                NormalizedBazelPath::new(HostPathFlavor::Windows, path),
                Err(BazelPathError::WindowsShortPathRequiresObservation { .. })
            ));
        }
    }
}
