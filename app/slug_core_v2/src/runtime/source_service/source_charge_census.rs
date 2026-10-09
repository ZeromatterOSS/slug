//! Default-off, borrowed source-charge provenance; never an admission authority.
use std::fmt::Write;
use std::fmt::{self};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;

use super::SourceCounter;
use super::SourceServiceError;

pub(super) const EVENT_LIMIT: u64 = 129;
const PATH_LIMIT: usize = 1024;
const RAW_LIMIT: u64 = 131072;
const ENCODED_LIMIT: u64 = 524288;
const SUMMARY_RESERVE: u64 = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum SourceChargeDomain {
    Host,
    ArchiveFile,
    ArchiveHttps,
    ArchiveRedirect,
    Unassociated,
}
impl SourceChargeDomain {
    fn tag(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::ArchiveFile => "archive_file",
            Self::ArchiveHttps => "archive_https",
            Self::ArchiveRedirect => "archive_redirect",
            Self::Unassociated => "unassociated",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum SourceChargePhase {
    CommandPreflight,
    NeedPaths,
    CertificatePrecheck,
    CertificateFinalize,
    Unassociated,
}
impl SourceChargePhase {
    fn tag(self) -> &'static str {
        match self {
            Self::CommandPreflight => "command_preflight",
            Self::NeedPaths => "need_paths",
            Self::CertificatePrecheck => "certificate_precheck",
            Self::CertificateFinalize => "certificate_finalize",
            Self::Unassociated => "unassociated",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum SourceChargeOutcome {
    Admitted,
    FirstSourceLimit,
    StickySourceLimit,
    Overflow,
    OtherError,
}
impl SourceChargeOutcome {
    fn tag(self) -> &'static str {
        match self {
            Self::Admitted => "admitted",
            Self::FirstSourceLimit => "first_source_limit",
            Self::StickySourceLimit => "sticky_source_limit",
            Self::Overflow => "overflow",
            Self::OtherError => "other_error",
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub(in crate::runtime) struct SourceChargeRecord {
    pub(in crate::runtime) event: u64,
    pub(in crate::runtime) amount: u64,
    pub(in crate::runtime) before: u64,
    pub(in crate::runtime) after: u64,
    pub(in crate::runtime) next: Option<u64>,
    pub(in crate::runtime) outcome: SourceChargeOutcome,
    pub(in crate::runtime) first_created: bool,
    pub(in crate::runtime) domain: SourceChargeDomain,
}
#[derive(Clone, Copy)]
pub(in crate::runtime) enum SourceChargeIdentity<'a> {
    Demand(&'a PathObservationDemand),
    HostPath(&'a Path),
    Anonymous,
}

#[derive(Default)]
pub(super) struct CensusState {
    seen: u64,
    emitted: u64,
    in_flight: u64,
    admitted: u64,
    raw_bytes: u64,
    encoded_bytes: u64,
    reserved_raw: u64,
    reserved_encoded: u64,
    first_limit: Option<u64>,
    skipped: u64,
    bounds: u64,
    writer_failed: u64,
    identity_unavailable: u64,
    invalid: bool,
}
impl CensusState {
    fn add(slot: &mut u64, amount: u64, invalid: &mut bool) {
        if let Some(next) = slot.checked_add(amount) {
            *slot = next;
        } else {
            *invalid = true;
        }
    }
    pub(super) fn record(
        &mut self,
        amount: u64,
        domain: SourceChargeDomain,
        before: u64,
        after: u64,
        result: Result<(), SourceServiceError>,
        first_created: bool,
    ) -> Option<SourceChargeRecord> {
        Self::add(&mut self.seen, 1, &mut self.invalid);
        if result.is_ok() {
            Self::add(&mut self.admitted, amount, &mut self.invalid);
        }
        if first_created {
            self.first_limit = Some(self.seen);
        }
        let next = before.checked_add(amount);
        let outcome = match result {
            Ok(()) => SourceChargeOutcome::Admitted,
            Err(SourceServiceError::Limit(SourceCounter::SourceAttempts)) if first_created => {
                SourceChargeOutcome::FirstSourceLimit
            }
            Err(SourceServiceError::Limit(SourceCounter::SourceAttempts)) => {
                SourceChargeOutcome::StickySourceLimit
            }
            Err(SourceServiceError::Overflow(SourceCounter::SourceAttempts)) => {
                SourceChargeOutcome::Overflow
            }
            Err(_) => SourceChargeOutcome::OtherError,
        };
        if next.is_none() || outcome == SourceChargeOutcome::Overflow {
            self.invalid = true;
        }
        if (self.invalid && self.seen == u64::MAX) || self.seen > EVENT_LIMIT {
            Self::add(&mut self.skipped, 1, &mut self.invalid);
            return None;
        }
        Self::add(&mut self.in_flight, 1, &mut self.invalid);
        Some(SourceChargeRecord {
            event: self.seen,
            amount,
            before,
            after,
            next,
            outcome,
            first_created,
            domain,
        })
    }
    pub(super) fn reserve(&mut self, row: &Row<'_>, bytes: u64) -> bool {
        // A failed writer may already have transmitted a prefix. Its released
        // reservation must never become reusable output budget. Reservations
        // made before the failure remain bounded and may still finish.
        if self.writer_failed != 0 {
            self.finish_flight();
            Self::add(&mut self.skipped, 1, &mut self.invalid);
            return false;
        }
        if row.capture == "bounds_refused" {
            Self::add(&mut self.bounds, 1, &mut self.invalid);
        }
        if !row.identity_complete {
            Self::add(&mut self.identity_unavailable, 1, &mut self.invalid);
        }
        let raw = row.path.map_or(0, |p| p.len() as u64);
        let raw_next = self
            .raw_bytes
            .checked_add(self.reserved_raw)
            .and_then(|n| n.checked_add(raw));
        let encoded_next = self
            .encoded_bytes
            .checked_add(self.reserved_encoded)
            .and_then(|n| n.checked_add(bytes))
            .and_then(|n| n.checked_add(SUMMARY_RESERVE));
        if raw_next.is_none_or(|n| n > RAW_LIMIT) || encoded_next.is_none_or(|n| n > ENCODED_LIMIT)
        {
            self.finish_flight();
            Self::add(&mut self.skipped, 1, &mut self.invalid);
            return false;
        }
        Self::add(&mut self.reserved_raw, raw, &mut self.invalid);
        Self::add(&mut self.reserved_encoded, bytes, &mut self.invalid);
        true
    }
    fn finish_flight(&mut self) {
        if let Some(n) = self.in_flight.checked_sub(1) {
            self.in_flight = n;
        } else {
            self.invalid = true;
        }
    }
    pub(super) fn finish(&mut self, row: &Row<'_>, bytes: u64, success: bool) {
        let raw = row.path.map_or(0, |p| p.len() as u64);
        match (
            self.reserved_raw.checked_sub(raw),
            self.reserved_encoded.checked_sub(bytes),
        ) {
            (Some(r), Some(e)) => {
                self.reserved_raw = r;
                self.reserved_encoded = e;
            }
            _ => self.invalid = true,
        }
        self.finish_flight();
        if success {
            Self::add(&mut self.emitted, 1, &mut self.invalid);
            Self::add(&mut self.raw_bytes, raw, &mut self.invalid);
            Self::add(&mut self.encoded_bytes, bytes, &mut self.invalid);
        } else {
            Self::add(&mut self.writer_failed, 1, &mut self.invalid);
        }
    }
    pub(super) fn encoding_failed(&mut self, event_in_flight: bool) {
        if event_in_flight {
            self.finish_flight();
        }
        Self::add(&mut self.writer_failed, 1, &mut self.invalid);
    }
    pub(super) fn summary(&self, limit: u64, source: u64) -> Summary {
        let complete = !self.invalid
            && self.skipped == 0
            && self.bounds == 0
            && self.writer_failed == 0
            && self.in_flight == 0
            && self.seen == self.emitted
            && self.admitted == source;
        Summary {
            limit,
            source,
            seen: self.seen,
            emitted: self.emitted,
            admitted: self.admitted,
            first_limit: self.first_limit,
            in_flight: self.in_flight,
            skipped: self.skipped,
            bounds: self.bounds,
            writer_failed: self.writer_failed,
            raw_bytes: self.raw_bytes,
            encoded_bytes: self.encoded_bytes,
            accounting_complete: complete,
            identity_complete: complete && self.identity_unavailable == 0,
        }
    }
}

pub(super) struct Row<'a> {
    record: SourceChargeRecord,
    phase: SourceChargePhase,
    capture: &'static str,
    namespace: &'static str,
    instance: Option<u64>,
    operation: &'static str,
    path: Option<&'a [u8]>,
    identity_complete: bool,
}
impl<'a> Row<'a> {
    pub(super) fn new(
        record: SourceChargeRecord,
        phase: SourceChargePhase,
        identity: SourceChargeIdentity<'a>,
    ) -> Self {
        let mut row = Self {
            record,
            phase,
            capture: "unassociated",
            namespace: "none",
            instance: None,
            operation: "none",
            path: None,
            identity_complete: false,
        };
        if record.domain != SourceChargeDomain::Host {
            row.phase = SourceChargePhase::Unassociated;
            return row;
        }
        match identity {
            SourceChargeIdentity::Demand(demand)
                if demand.operation() == PathObservationOperation::FileBytes =>
            {
                match demand.namespace() {
                    PathObservationNamespace::Host => row.namespace = "host",
                    PathObservationNamespace::Materialization(instance)
                        if instance.value() != 0 =>
                    {
                        row.namespace = "materialization";
                        row.instance = Some(instance.value());
                    }
                    _ => return row,
                }
                row.operation = "file_bytes";
                row.path = Some(demand.path().as_path().as_os_str().as_bytes());
                row.identity_complete = phase != SourceChargePhase::Unassociated;
            }
            SourceChargeIdentity::HostPath(path) => {
                row.namespace = "host";
                row.phase = SourceChargePhase::Unassociated;
                row.path = Some(path.as_os_str().as_bytes());
            }
            _ => return row,
        }
        row.capture = "captured";
        if row.path.is_some_and(|p| p.len() > PATH_LIMIT) {
            row.capture = "bounds_refused";
            row.namespace = "none";
            row.instance = None;
            row.operation = "none";
            row.path = None;
            row.identity_complete = false;
        }
        row
    }
}
struct Optional(u64, bool);
impl fmt::Display for Optional {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.1 {
            self.0.fmt(f)
        } else {
            f.write_str("none")
        }
    }
}
fn optional(value: Option<u64>) -> Optional {
    Optional(value.unwrap_or(0), value.is_some())
}
struct Hex<'a>(Option<&'a [u8]>);
impl fmt::Display for Hex<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            None => f.write_str("none"),
            Some([]) => f.write_str("-"),
            Some(bytes) => {
                for byte in bytes {
                    write!(f, "{byte:02x}")?;
                }
                Ok(())
            }
        }
    }
}
impl fmt::Display for Row<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = self.record;
        write!(
            f,
            "WP1033_SOURCE_CHARGE event={} amount={} before={} after={} next={} outcome={} first_created={} domain={} phase={} capture={} namespace={} instance={} operation={} path_len={} path_hex={}\n",
            r.event,
            r.amount,
            r.before,
            r.after,
            optional(r.next),
            r.outcome.tag(),
            r.first_created,
            r.domain.tag(),
            self.phase.tag(),
            self.capture,
            self.namespace,
            optional(self.instance),
            self.operation,
            optional(self.path.map(|p| p.len() as u64)),
            Hex(self.path)
        )
    }
}
#[derive(Clone, Copy)]
pub(super) struct Summary {
    limit: u64,
    source: u64,
    seen: u64,
    emitted: u64,
    admitted: u64,
    first_limit: Option<u64>,
    in_flight: u64,
    skipped: u64,
    bounds: u64,
    writer_failed: u64,
    raw_bytes: u64,
    encoded_bytes: u64,
    accounting_complete: bool,
    identity_complete: bool,
}
impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WP1033_SOURCE_CHARGE_SUMMARY enabled=true limit={} source={} seen={} emitted={} admitted={} first_limit={} in_flight={} skipped={} bounds={} writer_failed={} raw_bytes={} encoded_bytes={} accounting_complete={} identity_complete={}\n",
            self.limit,
            self.source,
            self.seen,
            self.emitted,
            self.admitted,
            optional(self.first_limit),
            self.in_flight,
            self.skipped,
            self.bounds,
            self.writer_failed,
            self.raw_bytes,
            self.encoded_bytes,
            self.accounting_complete,
            self.identity_complete
        )
    }
}
pub(super) fn encoded_len(value: &impl fmt::Display) -> Result<u64, fmt::Error> {
    struct Counter(u64);
    impl Write for Counter {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            self.0 = self.0.checked_add(value.len() as u64).ok_or(fmt::Error)?;
            Ok(())
        }
    }
    let mut counter = Counter(0);
    write!(&mut counter, "{value}")?;
    Ok(counter.0)
}
impl Summary {
    pub(super) fn including_summary(mut self) -> Result<Self, fmt::Error> {
        let rows = self.encoded_bytes;
        // Decimal width can grow when the summary includes its own framing.
        for _ in 0..4 {
            let length = encoded_len(&self)?;
            if length > SUMMARY_RESERVE {
                return Err(fmt::Error);
            }
            let total = rows.checked_add(length).ok_or(fmt::Error)?;
            if total > ENCODED_LIMIT {
                return Err(fmt::Error);
            }
            if self.encoded_bytes == total {
                return Ok(self);
            }
            self.encoded_bytes = total;
        }
        Err(fmt::Error)
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    use slug_workspace_v2::NormalizedAbsolutePath;

    use super::*;

    #[test]
    fn source_charge_census_is_bounded_borrowed_and_honest() {
        let mut state = CensusState::default();
        let first = state
            .record(0, SourceChargeDomain::Host, 0, 0, Ok(()), false)
            .unwrap();
        let second = state
            .record(2, SourceChargeDomain::Host, 0, 2, Ok(()), false)
            .unwrap();
        let demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new("/borrowed").unwrap(),
            PathObservationOperation::FileBytes,
        );
        let original = demand.path().as_path().as_os_str().as_bytes().as_ptr();
        let row = Row::new(
            second,
            SourceChargePhase::NeedPaths,
            SourceChargeIdentity::Demand(&demand),
        );
        assert_eq!(row.path.unwrap().as_ptr(), original);
        let bytes = encoded_len(&row).unwrap();
        assert_eq!(bytes, format!("{row}").len() as u64);
        assert!(state.reserve(&row, bytes));
        assert!(!state.summary(128, 2).accounting_complete);
        state.finish(&row, bytes, true); // Emission order need not equal charge order.
        let row = Row::new(
            first,
            SourceChargePhase::CertificateFinalize,
            SourceChargeIdentity::Demand(&demand),
        );
        let first_bytes = encoded_len(&row).unwrap();
        assert!(state.reserve(&row, first_bytes));
        state.finish(&row, first_bytes, true);
        let summary = state.summary(128, 2).including_summary().unwrap();
        assert!(summary.accounting_complete && summary.identity_complete);
        assert_eq!(
            summary.encoded_bytes,
            bytes + first_bytes + format!("{summary}").len() as u64
        );
        assert_eq!(summary.raw_bytes, 18);

        let empty = Row::new(
            first,
            SourceChargePhase::Unassociated,
            SourceChargeIdentity::HostPath(Path::new("")),
        );
        assert!(format!("{empty}").ends_with("path_len=0 path_hex=-\n"));
        assert!(!empty.identity_complete);
        let opaque = Row::new(
            first,
            SourceChargePhase::Unassociated,
            SourceChargeIdentity::HostPath(Path::new(OsStr::from_bytes(b"/\xff"))),
        );
        assert!(format!("{opaque}").ends_with("path_len=2 path_hex=2fff\n"));
        let longest = [b'x'; PATH_LIMIT];
        let longest = Row::new(
            first,
            SourceChargePhase::Unassociated,
            SourceChargeIdentity::HostPath(Path::new(OsStr::from_bytes(&longest))),
        );
        assert_eq!(longest.capture, "captured");
        let oversized = [b'x'; PATH_LIMIT + 1];
        let refused = Row::new(
            first,
            SourceChargePhase::NeedPaths,
            SourceChargeIdentity::HostPath(Path::new(OsStr::from_bytes(&oversized))),
        );
        assert_eq!(refused.capture, "bounds_refused");
        assert!(format!("{refused}").ends_with(
            "namespace=none instance=none operation=none path_len=none path_hex=none\n"
        ));
        let mut bounds = CensusState::default();
        bounds
            .record(0, SourceChargeDomain::Host, 0, 0, Ok(()), false)
            .unwrap();
        let n = encoded_len(&refused).unwrap();
        assert!(bounds.reserve(&refused, n));
        bounds.finish(&refused, n, true);
        assert!(!bounds.summary(128, 0).accounting_complete);

        for domain in [
            SourceChargeDomain::ArchiveFile,
            SourceChargeDomain::ArchiveHttps,
            SourceChargeDomain::ArchiveRedirect,
            SourceChargeDomain::Unassociated,
        ] {
            let record = SourceChargeRecord { domain, ..first };
            let row = Row::new(
                record,
                SourceChargePhase::NeedPaths,
                SourceChargeIdentity::Demand(&demand),
            );
            assert_eq!(
                (row.capture, row.phase, row.path),
                ("unassociated", SourceChargePhase::Unassociated, None)
            );
        }
        for phase in [
            SourceChargePhase::CommandPreflight,
            SourceChargePhase::NeedPaths,
            SourceChargePhase::CertificatePrecheck,
            SourceChargePhase::CertificateFinalize,
        ] {
            let row = Row::new(first, phase, SourceChargeIdentity::Demand(&demand));
            assert_eq!(row.path.unwrap().as_ptr(), original);
            assert!(row.identity_complete);
            assert!(format!("{row}").contains(&format!(
                "phase={} capture=captured namespace=host instance=none operation=file_bytes",
                phase.tag()
            )));
        }
        let materialized = PathObservationDemand::new(
            PathObservationNamespace::Materialization(
                slug_workspace_v2::PathObservationInstanceId::new(7),
            ),
            NormalizedAbsolutePath::new("/borrowed").unwrap(),
            PathObservationOperation::FileBytes,
        );
        let row = Row::new(
            first,
            SourceChargePhase::NeedPaths,
            SourceChargeIdentity::Demand(&materialized),
        );
        assert_eq!((row.namespace, row.instance), ("materialization", Some(7)));
        assert_eq!(
            row.path.unwrap().as_ptr(),
            materialized
                .path()
                .as_path()
                .as_os_str()
                .as_bytes()
                .as_ptr()
        );
        assert!(
            format!("{row}").contains("namespace=materialization instance=7 operation=file_bytes")
        );
        let wrong_op = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new("/borrowed").unwrap(),
            PathObservationOperation::Lstat,
        );
        assert_eq!(
            Row::new(
                first,
                SourceChargePhase::NeedPaths,
                SourceChargeIdentity::Demand(&wrong_op)
            )
            .capture,
            "unassociated"
        );

        let full_path = [b'x'; PATH_LIMIT];
        let mut exhausted = CensusState::default();
        for index in 1..=EVENT_LIMIT {
            let record = exhausted
                .record(0, SourceChargeDomain::Host, 0, 0, Ok(()), false)
                .unwrap();
            assert_eq!(record.event, index);
            let row = Row::new(
                record,
                SourceChargePhase::Unassociated,
                SourceChargeIdentity::HostPath(Path::new(OsStr::from_bytes(&full_path))),
            );
            let n = encoded_len(&row).unwrap();
            // 128 full paths fill raw budget; event129 must be honestly skipped.
            let reserved = exhausted.reserve(&row, n);
            assert_eq!(reserved, index <= RAW_LIMIT / PATH_LIMIT as u64);
            if reserved {
                exhausted.finish(&row, n, true);
            }
        }
        assert!(
            exhausted
                .record(1, SourceChargeDomain::Host, 0, 1, Ok(()), false)
                .is_none()
        );
        let summary = exhausted.summary(128, 1).including_summary().unwrap();
        assert_eq!(
            (
                summary.seen,
                summary.emitted,
                summary.skipped,
                summary.in_flight
            ),
            (130, 128, 2, 0)
        );
        assert!(!summary.accounting_complete && !summary.identity_complete);
        assert!(summary.encoded_bytes <= ENCODED_LIMIT);
        let mut encoded_full = CensusState::default();
        encoded_full
            .record(0, SourceChargeDomain::Host, 0, 0, Ok(()), false)
            .unwrap();
        encoded_full.encoded_bytes = ENCODED_LIMIT - SUMMARY_RESERVE;
        assert!(!encoded_full.reserve(&row, bytes));
        assert_eq!(encoded_full.skipped, 1);

        struct PartialWriter {
            written: usize,
            remaining: usize,
        }
        impl Write for PartialWriter {
            fn write_str(&mut self, value: &str) -> fmt::Result {
                let count = value.len().min(self.remaining);
                self.written += count;
                self.remaining -= count;
                if count < value.len() {
                    Err(fmt::Error)
                } else {
                    Ok(())
                }
            }
        }
        let mut failed = CensusState::default();
        failed
            .record(0, SourceChargeDomain::Host, 0, 0, Ok(()), false)
            .unwrap();
        assert!(failed.reserve(&row, bytes));
        let mut writer = PartialWriter {
            written: 0,
            remaining: 64,
        };
        let success = write!(&mut writer, "{row}").is_ok();
        assert_eq!(writer.written, 64);
        assert!(!success);
        failed.finish(&row, bytes, success);
        let summary = failed.summary(128, 0);
        assert_eq!(
            (
                summary.emitted,
                summary.writer_failed,
                summary.encoded_bytes,
                summary.in_flight
            ),
            (0, 1, 0, 0)
        );
        assert!(!summary.accounting_complete);
        let later = failed
            .record(1, SourceChargeDomain::Host, 0, 1, Ok(()), false)
            .unwrap();
        let later = Row::new(
            later,
            SourceChargePhase::NeedPaths,
            SourceChargeIdentity::Demand(&demand),
        );
        let later_bytes = encoded_len(&later).unwrap();
        let reserved = failed.reserve(&later, later_bytes);
        assert!(!reserved);
        if reserved {
            write!(&mut writer, "{later}").unwrap();
        }
        assert_eq!(writer.written, 64); // No prefix/output attempt for the later event.
        let summary = failed.summary(128, 1);
        assert_eq!(
            (
                summary.seen,
                summary.emitted,
                summary.admitted,
                summary.skipped,
                summary.in_flight
            ),
            (2, 0, 1, 1, 0)
        );
        assert!(!summary.accounting_complete);
        assert_eq!((failed.reserved_raw, failed.reserved_encoded), (0, 0));
        let mut encoding_failed = CensusState::default();
        encoding_failed
            .record(0, SourceChargeDomain::Host, 0, 0, Ok(()), false)
            .unwrap();
        encoding_failed.encoding_failed(true);
        let later = encoding_failed
            .record(0, SourceChargeDomain::Host, 0, 0, Ok(()), false)
            .unwrap();
        let later = Row::new(
            later,
            SourceChargePhase::NeedPaths,
            SourceChargeIdentity::Demand(&demand),
        );
        assert!(!encoding_failed.reserve(&later, encoded_len(&later).unwrap()));
        assert_eq!(encoding_failed.summary(128, 0).skipped, 1);
        let mut abandoned = CensusState::default();
        abandoned
            .record(0, SourceChargeDomain::Host, 0, 0, Ok(()), false)
            .unwrap();
        assert_eq!(abandoned.summary(128, 0).in_flight, 1);
        assert!(!abandoned.summary(128, 0).accounting_complete);
        let mut overflow = CensusState::default();
        let record = overflow
            .record(
                1,
                SourceChargeDomain::Host,
                u64::MAX,
                u64::MAX,
                Err(SourceServiceError::Overflow(SourceCounter::SourceAttempts)),
                false,
            )
            .unwrap();
        assert_eq!(
            (record.next, record.outcome),
            (None, SourceChargeOutcome::Overflow)
        );
        assert!(!overflow.summary(128, u64::MAX).accounting_complete);

        let mut scalar_overflow = CensusState {
            seen: u64::MAX,
            ..Default::default()
        };
        assert!(
            scalar_overflow
                .record(0, SourceChargeDomain::Host, 0, 0, Ok(()), false)
                .is_none()
        );
        assert!(!scalar_overflow.summary(128, 0).accounting_complete);
        failed.encoding_failed(false);
        assert_eq!(failed.summary(128, 0).writer_failed, 2);
        let (old_state, new_state, old_scope, new_scope) =
            super::super::scope::census_layout_for_test();
        println!(
            "WP1033_CENSUS_LAYOUT state={} option={} old_state={old_state} new_state={new_state} old_scope={old_scope} new_scope={new_scope}",
            std::mem::size_of::<CensusState>(),
            std::mem::size_of::<Option<CensusState>>()
        );
        assert!(new_state >= old_state && new_scope >= old_scope);
    }
}
