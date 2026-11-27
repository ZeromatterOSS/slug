//! Fixed test-only timing at the actual native workspace dispatch boundaries.
use std::sync::Mutex;
use std::time::Instant;

use serde_json::Value;
use serde_json::json;
use slug_bzlmod_v2::RepositoryWorkspaceOperation;

use super::snapshot_work;
use super::xz_calls;

#[derive(Clone, Copy)]
pub(crate) enum Stage {
    Preflight,
    ReserveAndInitialize,
    SourceSnapshot,
    Archive,
    Execute,
    Filesystem,
    ArchiveCapture,
    ArchiveExtract,
    ArchiveSnapshot,
    ArchiveAcquire,
    ArchiveVerifyCapture,
    FilesystemOperation,
    FilesystemSnapshot,
}

#[derive(Clone, Copy)]
enum SnapshotKind {
    Archive,
    Filesystem,
}

impl Stage {
    pub(crate) fn operation(operation: &RepositoryWorkspaceOperation) -> Self {
        match operation {
            RepositoryWorkspaceOperation::Archive(_) => Self::Archive,
            RepositoryWorkspaceOperation::Execute(_) => Self::Execute,
            RepositoryWorkspaceOperation::File { .. }
            | RepositoryWorkspaceOperation::Template { .. }
            | RepositoryWorkspaceOperation::Symlink { .. }
            | RepositoryWorkspaceOperation::Read(_)
            | RepositoryWorkspaceOperation::Exists(_)
            | RepositoryWorkspaceOperation::Delete(_) => Self::Filesystem,
        }
    }

    fn snapshot_kind(self) -> Option<SnapshotKind> {
        match self {
            Self::ArchiveSnapshot => Some(SnapshotKind::Archive),
            Self::FilesystemSnapshot => Some(SnapshotKind::Filesystem),
            _ => None,
        }
    }
}

const LABELS: [&str; 13] = [
    "preflight",
    "reserve_and_initialize",
    "source_snapshot",
    "archive",
    "execute",
    "filesystem",
    "archive_verified_capture",
    "archive_rooted_extract",
    "archive_snapshot_and_record",
    "archive_acquire",
    "archive_verify_closed_capture",
    "filesystem_operation",
    "filesystem_snapshot_and_record",
];

#[derive(Default)]
pub(super) struct Totals {
    // Two independent outcomes per stage. Column pairs are calls and wall ns.
    rows: [[u64; 4]; LABELS.len()],
    invalid: bool,
    xz: [xz_calls::Work; 2],
    snapshots: [[snapshot_work::Work; 2]; 2],
}

impl Totals {
    fn record(&mut self, stage: Stage, completed: bool, wall_ns: Option<u64>) {
        let row = &mut self.rows[stage as usize];
        let column = if completed { 0 } else { 2 };
        let sum = wall_ns.and_then(|ns| row[column + 1].checked_add(ns));
        match row[column].checked_add(1).zip(sum) {
            Some((calls, wall_ns)) => {
                row[column] = calls;
                row[column + 1] = wall_ns;
            }
            None => self.invalid = true,
        }
    }

    fn snapshot_report(&self, kind: SnapshotKind) -> Value {
        let [completed, aborted] = self.snapshots[kind as usize];
        json!({
            "phases": snapshot_work::LABELS,
            "columns": ["calls", "wall_ns", "requested_bytes"],
            "completed_snapshots": completed.rows,
            "aborted_snapshots": aborted.rows,
            "unaligned_chunks": [completed.unaligned_chunks, aborted.unaligned_chunks],
            "invalid_clock_or_overflow": completed.invalid || aborted.invalid,
            "compare_and_append_nested_in_capture": true,
        })
    }

    pub(super) fn report(&self) -> Value {
        json!({
            "stages": LABELS,
            "columns": ["completed", "completed_wall_ns", "aborted", "aborted_wall_ns"],
            "rows": self.rows,
            "invalid_clock_or_overflow": self.invalid,
            "dispatch_completion_includes_replay": true,
            "archive_substages_are_nested_and_exclude_replay": true,
            "archive_acquire_and_verify_are_nested_in_verified_capture": true,
            "filesystem_substages_are_nested_and_exclude_replay": true,
            "overlaps_outer_progress_wall": true,
            "archive_stages_use_probe_thread": true,
            "xz_calls_within_rooted_extract": {
                "columns": xz_calls::COLUMNS,
                "completed_extracts": self.xz[0].row,
                "aborted_extracts": self.xz[1].row,
                "invalid_clock_counts_or_buffer_deltas": self.xz.iter().any(|work| work.invalid),
                "decode_intervals_include_buffer_bookkeeping": true,
            },
            "archive_snapshot_work": self.snapshot_report(SnapshotKind::Archive),
            "filesystem_snapshot_work": self.snapshot_report(SnapshotKind::Filesystem),
        })
    }
}

/// Holds a sampler reference only. The mutex is acquired after work returns,
/// including early returns, and never spans native IO or child execution.
#[must_use]
pub(crate) struct Span<'a> {
    totals: &'a Mutex<Totals>,
    stage: Stage,
    start: Instant,
    completed: bool,
    xz: Option<xz_calls::Scope>,
    snapshot: Option<(SnapshotKind, snapshot_work::Scope)>,
}

impl<'a> Span<'a> {
    pub(super) fn start(totals: &'a Mutex<Totals>, stage: Stage) -> Self {
        Self {
            totals,
            stage,
            start: Instant::now(),
            completed: false,
            xz: matches!(stage, Stage::ArchiveExtract).then(xz_calls::Scope::begin),
            snapshot: stage
                .snapshot_kind()
                .map(|kind| (kind, snapshot_work::Scope::begin())),
        }
    }

    pub(crate) fn finish(mut self) {
        self.completed = true;
    }
}

impl Drop for Span<'_> {
    fn drop(&mut self) {
        let wall_ns = Instant::now()
            .checked_duration_since(self.start)
            .and_then(|duration| u64::try_from(duration.as_nanos()).ok());
        let xz_work = self.xz.take().map(xz_calls::Scope::finish);
        let snapshot_work = self
            .snapshot
            .take()
            .map(|(kind, scope)| (kind, scope.finish()));
        let mut totals = self
            .totals
            .lock()
            .expect("workspace progress sampler poisoned");
        totals.record(self.stage, self.completed, wall_ns);
        if let Some(work) = xz_work {
            totals.xz[usize::from(!self.completed)].merge(work);
        }
        if let Some((kind, work)) = snapshot_work {
            totals.snapshots[kind as usize][usize::from(!self.completed)].merge(work);
        }
    }
}

#[test]
fn workspace_progress_spans_distinguish_completion_early_drop_and_invalid_totals() {
    let totals = Mutex::new(Totals::default());
    Span::start(&totals, Stage::Archive).finish();
    drop(Span::start(&totals, Stage::Execute));
    let mut totals = totals.into_inner().unwrap();
    assert_eq!(totals.rows[Stage::Archive as usize][0], 1);
    assert_eq!(totals.rows[Stage::Archive as usize][2], 0);
    assert_eq!(totals.rows[Stage::Execute as usize][0], 0);
    assert_eq!(totals.rows[Stage::Execute as usize][2], 1);

    let before = totals.rows;
    totals.record(Stage::Preflight, true, None);
    assert!(totals.invalid);
    assert_eq!(totals.rows, before);
    totals.invalid = false;
    totals.rows[Stage::Preflight as usize][1] = u64::MAX;
    let before = totals.rows;
    totals.record(Stage::Preflight, true, Some(1));
    assert!(totals.invalid);
    assert_eq!(totals.rows, before);
}

#[test]
#[cfg(target_pointer_width = "64")]
fn workspace_progress_folds_only_in_scope_lib_lzma_calls_into_extract_outcomes() {
    use std::io::Read;
    use std::io::Write;

    use crate::runtime::dice::source_service::SourceXzDecoder;

    let payload = b"same decoder owner as rooted XZ extraction";
    let mut encoder = xz2::write::XzEncoder::new(Vec::new(), 1);
    encoder.write_all(payload).unwrap();
    let compressed = encoder.finish().unwrap();
    let decode = |input: &[u8]| {
        let mut decoded = Vec::new();
        SourceXzDecoder::new(input, None)?.read_to_end(&mut decoded)?;
        Ok::<_, std::io::Error>(decoded)
    };

    let totals = Mutex::new(Totals::default());
    let capture = Span::start(&totals, Stage::ArchiveCapture);
    assert_eq!(decode(&compressed).unwrap(), payload);
    capture.finish();
    let extract = Span::start(&totals, Stage::ArchiveExtract);
    assert_eq!(decode(&compressed).unwrap(), payload);
    extract.finish();
    let aborted = Span::start(&totals, Stage::ArchiveExtract);
    assert!(decode(&compressed[..compressed.len() - 1]).is_err());
    drop(aborted);
    assert_eq!(decode(&compressed).unwrap(), payload);

    let totals = totals.into_inner().unwrap();
    let [complete, abort] = totals.xz;
    assert!(!complete.invalid && !abort.invalid);
    assert_eq!((complete.row[0], complete.row[2]), (1, 1));
    assert_eq!((abort.row[0], abort.row[2]), (1, 1));
    assert!(complete.row[1] > 0 && abort.row[1] > 0);
    assert_eq!(complete.row[4], compressed.len() as u64);
    assert_eq!(complete.row[5], payload.len() as u64);
    let extract = totals.rows[Stage::ArchiveExtract as usize];
    assert!(complete.row[3] <= extract[1] && abort.row[3] <= extract[3]);
}

#[test]
fn workspace_progress_snapshot_work_stays_in_the_operation_and_outcome_that_started_it() {
    use snapshot_work::Phase;
    use snapshot_work::span;

    let totals = Mutex::new(Totals::default());
    let filesystem = Span::start(&totals, Stage::Filesystem);
    assert!(span(Phase::Capture, 0).is_none());
    snapshot_work::record_unaligned_comparison_chunk();
    filesystem.finish();
    let snapshot = Span::start(&totals, Stage::ArchiveSnapshot);
    {
        let _capture = span(Phase::Capture, 0).unwrap();
        drop(span(Phase::PriorCompare, 12).unwrap());
        drop(span(Phase::BuilderAppend, 34).unwrap());
    }
    drop(span(Phase::Record, 0).unwrap());
    snapshot.finish();
    {
        let _snapshot = Span::start(&totals, Stage::ArchiveSnapshot);
        let _capture = span(Phase::Capture, 0).unwrap();
        drop(span(Phase::BuilderAppend, 56).unwrap());
    }
    let snapshot = Span::start(&totals, Stage::FilesystemSnapshot);
    {
        let _capture = span(Phase::Capture, 0).unwrap();
        drop(span(Phase::PriorCompare, 78).unwrap());
    }
    drop(span(Phase::Record, 0).unwrap());
    snapshot.finish();
    {
        let _snapshot = Span::start(&totals, Stage::FilesystemSnapshot);
        let _capture = span(Phase::Capture, 0).unwrap();
        drop(span(Phase::PriorCompare, 90).unwrap());
        snapshot_work::record_unaligned_comparison_chunk();
    }
    assert!(span(Phase::Capture, 0).is_none());

    let totals = totals.into_inner().unwrap();
    let [[archive, archive_abort], [filesystem, filesystem_abort]] = totals.snapshots;
    assert!(totals.snapshots.iter().flatten().all(|work| !work.invalid));
    assert_eq!(
        totals
            .snapshots
            .map(|family| family.map(|work| work.unaligned_chunks)),
        [[0, 0], [0, 1]]
    );
    let calls_and_bytes = |rows: [[u64; 3]; 4]| rows.map(|[calls, _, bytes]| (calls, bytes));
    assert_eq!(
        calls_and_bytes(archive.rows),
        [(1, 0), (1, 0), (1, 12), (1, 34)]
    );
    assert_eq!(
        calls_and_bytes(archive_abort.rows),
        [(1, 0), (0, 0), (0, 0), (1, 56)]
    );
    assert_eq!(
        calls_and_bytes(filesystem.rows),
        [(1, 0), (1, 0), (1, 78), (0, 0)]
    );
    assert_eq!(
        calls_and_bytes(filesystem_abort.rows),
        [(1, 0), (0, 0), (1, 90), (0, 0)]
    );
}

#[test]
fn workspace_progress_report_is_bounded_at_maximum_counts() {
    let report = Totals {
        rows: [[u64::MAX; 4]; LABELS.len()],
        invalid: false,
        xz: [xz_calls::Work {
            row: [u64::MAX; xz_calls::COLUMNS.len()],
            invalid: false,
        }; 2],
        snapshots: [[snapshot_work::Work {
            rows: [[u64::MAX; 3]; snapshot_work::LABELS.len()],
            unaligned_chunks: u64::MAX,
            invalid: false,
        }; 2]; 2],
    }
    .report();
    assert!(json!({"workspace_progress": report}).to_string().len() < REPORT_BYTES);
}

pub(crate) const REPORT_BYTES: usize = 4 * 1024;
