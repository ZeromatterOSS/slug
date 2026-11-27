//! Fixed test-only timing at the actual native workspace dispatch boundaries.
use std::sync::Mutex;
use std::time::Instant;

use serde_json::Value;
use serde_json::json;
use slug_bzlmod_v2::RepositoryWorkspaceOperation;

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
}

const LABELS: [&str; 9] = [
    "preflight",
    "reserve_and_initialize",
    "source_snapshot",
    "archive",
    "execute",
    "filesystem",
    "archive_verified_capture",
    "archive_rooted_extract",
    "archive_snapshot_and_record",
];

#[derive(Default)]
pub(super) struct Totals {
    // Two independent outcomes per stage. Column pairs are calls and wall ns.
    rows: [[u64; 4]; LABELS.len()],
    invalid: bool,
    xz: [xz_calls::Work; 2],
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

    pub(super) fn report(&self) -> Value {
        json!({
            "stages": LABELS,
            "columns": ["completed", "completed_wall_ns", "aborted", "aborted_wall_ns"],
            "rows": self.rows,
            "invalid_clock_or_overflow": self.invalid,
            "dispatch_completion_includes_replay": true,
            "archive_substages_are_nested_and_exclude_replay": true,
            "overlaps_outer_progress_wall": true,
            "archive_stages_use_probe_thread": true,
            "xz_calls_within_rooted_extract": {
                "columns": xz_calls::COLUMNS,
                "completed_extracts": self.xz[0].row,
                "aborted_extracts": self.xz[1].row,
                "invalid_clock_counts_or_buffer_deltas": self.xz.iter().any(|work| work.invalid),
                "decode_intervals_include_buffer_bookkeeping": true,
            },
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
}

impl<'a> Span<'a> {
    pub(super) fn start(totals: &'a Mutex<Totals>, stage: Stage) -> Self {
        Self {
            totals,
            stage,
            start: Instant::now(),
            completed: false,
            xz: matches!(stage, Stage::ArchiveExtract).then(xz_calls::Scope::begin),
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
        let mut totals = self
            .totals
            .lock()
            .expect("workspace progress sampler poisoned");
        totals.record(self.stage, self.completed, wall_ns);
        if let Some(work) = xz_work {
            totals.xz[usize::from(!self.completed)].merge(work);
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
fn workspace_progress_report_is_bounded_at_maximum_counts() {
    let report = Totals {
        rows: [[u64::MAX; 4]; LABELS.len()],
        invalid: false,
        xz: [xz_calls::Work {
            row: [u64::MAX; xz_calls::COLUMNS.len()],
            invalid: false,
        }; 2],
    }
    .report();
    assert!(json!({"workspace_progress": report}).to_string().len() < REPORT_BYTES);
}

pub(crate) const REPORT_BYTES: usize = 2 * 1024;
