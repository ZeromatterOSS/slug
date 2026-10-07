//! Bounded test-only projection of the demand whose charge first refused.
use std::fmt;
use std::os::unix::ffi::OsStrExt;

use super::PathObservationDemand;
use super::PathObservationNamespace;
use super::PathObservationOperation;

const PATH_BYTES: usize = 1024;
const LINE_BYTES: usize = 4096;
// Static fields plus the largest decimal instance/length and LF fit in 256B.
const _: () = assert!(256 + 2 * PATH_BYTES <= LINE_BYTES);

struct SourceAttemptDiagnostic<'a> {
    demand: &'a PathObservationDemand,
}

impl<'a> SourceAttemptDiagnostic<'a> {
    fn new(demand: &'a PathObservationDemand) -> Option<Self> {
        if demand.operation() != PathObservationOperation::FileBytes
            || matches!(demand.namespace(), PathObservationNamespace::Materialization(id) if id.value() == 0)
        {
            return None;
        }
        Some(Self { demand })
    }
}

impl fmt::Display for SourceAttemptDiagnostic<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = self.demand.path().as_path().as_os_str().as_bytes();
        let captured = bytes.len() <= PATH_BYTES;
        write!(
            f,
            "WP989_SOURCE_ATTEMPT capture={} namespace=",
            if captured {
                "captured"
            } else {
                "bounds_refused"
            }
        )?;
        match self.demand.namespace() {
            PathObservationNamespace::Host => f.write_str("host instance=none")?,
            PathObservationNamespace::Materialization(id) => {
                write!(f, "materialization instance={}", id.value())?;
            }
        }
        write!(
            f,
            " operation=file_bytes path_len={} path_hex=",
            bytes.len()
        )?;
        if captured {
            for byte in bytes {
                write!(f, "{byte:02x}")?;
            }
        } else {
            f.write_str("none")?;
        }
        Ok(())
    }
}

pub(super) fn report(demand: &PathObservationDemand) {
    if let Some(diagnostic) = SourceAttemptDiagnostic::new(demand) {
        println!("{diagnostic}");
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use slug_workspace_v2::NormalizedAbsolutePath;

    use super::*;
    use crate::runtime::path_observation::PathObservationInstanceId;

    fn demand(namespace: PathObservationNamespace, bytes: Vec<u8>) -> PathObservationDemand {
        PathObservationDemand::new(
            namespace,
            NormalizedAbsolutePath::new(OsString::from_vec(bytes)).unwrap(),
            PathObservationOperation::FileBytes,
        )
    }

    #[test]
    fn source_attempt_marker_borrows_original_demand_and_bounds_encoding() {
        let host = demand(
            PathObservationNamespace::Host,
            vec![b'/', 0xff, b'\n', b'\t'],
        );
        let diagnostic = SourceAttemptDiagnostic::new(&host).unwrap();
        assert!(std::ptr::eq(diagnostic.demand, &host));
        assert!(std::ptr::eq(
            diagnostic.demand.path().as_path(),
            host.path().as_path()
        ));
        assert_eq!(
            format!("{diagnostic}\n"),
            "WP989_SOURCE_ATTEMPT capture=captured namespace=host instance=none operation=file_bytes path_len=4 path_hex=2fff0a09\n"
        );
        let materialization = demand(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(u64::MAX)),
            vec![b'/', b'a'],
        );
        let diagnostic = SourceAttemptDiagnostic::new(&materialization).unwrap();
        assert!(std::ptr::eq(diagnostic.demand, &materialization));
        assert_eq!(
            format!("{diagnostic}\n"),
            "WP989_SOURCE_ATTEMPT capture=captured namespace=materialization instance=18446744073709551615 operation=file_bytes path_len=2 path_hex=2f61\n"
        );
        let mut bytes = vec![b'a'; PATH_BYTES];
        bytes[0] = b'/';
        let at_bound = demand(PathObservationNamespace::Host, bytes.clone());
        let line = format!("{}\n", SourceAttemptDiagnostic::new(&at_bound).unwrap());
        assert!(line.contains("capture=captured"));
        assert!(line.contains("path_len=1024 path_hex=2f"));
        assert_eq!(
            line.split("path_hex=").nth(1).unwrap().trim_end().len(),
            2048
        );
        assert!(line.len() <= LINE_BYTES);
        assert_eq!(line.bytes().filter(|byte| *byte == b'\n').count(), 1);
        bytes.push(b'a');
        let over_bound = demand(PathObservationNamespace::Host, bytes);
        let line = format!("{}\n", SourceAttemptDiagnostic::new(&over_bound).unwrap());
        assert_eq!(
            line,
            "WP989_SOURCE_ATTEMPT capture=bounds_refused namespace=host instance=none operation=file_bytes path_len=1025 path_hex=none\n"
        );
        assert!(line.len() <= LINE_BYTES);
    }

    #[test]
    fn source_attempt_marker_rejects_unobservable_domains() {
        let zero = demand(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(0)),
            b"/body".to_vec(),
        );
        assert!(SourceAttemptDiagnostic::new(&zero).is_none());
        for operation in [
            PathObservationOperation::Lstat,
            PathObservationOperation::ReadLink,
            PathObservationOperation::FileDigest,
            PathObservationOperation::DirectoryEntries,
        ] {
            let demand = PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new("/body").unwrap(),
                operation,
            );
            assert!(SourceAttemptDiagnostic::new(&demand).is_none());
        }
    }
}
