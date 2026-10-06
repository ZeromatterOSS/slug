//! Workspace-global default status bytes computed from the tracked native capture.
use std::fmt::Write as _;
use std::io::Write as _;

use slug_analysis_v2::WorkspaceStatusArtifactsKey;
use slug_build_api_v2::WorkspaceStatusArtifactKind;
use slug_build_api_v2::WorkspaceStatusArtifacts;

use super::super::workspace_status_inputs::*;
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct WorkspaceStatusContent {
    pub(super) capture: Arc<WorkspaceStatusCapture>,
    pub(super) artifacts: Arc<WorkspaceStatusArtifacts>,
    pub(super) stable: Vec<u8>,
    pub(super) volatile: Vec<u8>,
}

impl WorkspaceStatusContent {
    pub(super) fn bytes(&self, kind: WorkspaceStatusArtifactKind) -> &[u8] {
        match kind {
            WorkspaceStatusArtifactKind::Stable => &self.stable,
            WorkspaceStatusArtifactKind::Volatile => &self.volatile,
        }
    }
}

pub(super) fn formatted_date(seconds: u64) -> Result<String, StatusError> {
    if seconds > MAX_MILLIS / 1000 {
        return Err(StatusError::TimeRange);
    }
    #[cfg(unix)]
    {
        let seconds = libc::time_t::try_from(seconds).map_err(|_| StatusError::TimeRange)?;
        let mut calendar = std::mem::MaybeUninit::<libc::tm>::uninit();
        // Both pointers refer to live local storage; gmtime_r initializes tm on
        // success and has no shared calendar/locale buffer or asynchronous work.
        let result = unsafe { libc::gmtime_r(&seconds, calendar.as_mut_ptr()) };
        if result.is_null() {
            return Err(StatusError::Calendar);
        }
        let calendar = unsafe { calendar.assume_init() };
        let year = calendar
            .tm_year
            .checked_add(1900)
            .ok_or(StatusError::Calendar)?;
        if !(1970..=9999).contains(&year)
            || !(0..12).contains(&calendar.tm_mon)
            || !(0..7).contains(&calendar.tm_wday)
            || !(1..=31).contains(&calendar.tm_mday)
            || !(0..24).contains(&calendar.tm_hour)
            || !(0..60).contains(&calendar.tm_min)
            || !(0..60).contains(&calendar.tm_sec)
        {
            return Err(StatusError::Calendar);
        }
        const MONTHS: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        let mut date = String::with_capacity(24);
        write!(
            date,
            "{year:04} {} {:02} {:02} {:02} {:02} {}",
            MONTHS[calendar.tm_mon as usize],
            calendar.tm_mday,
            calendar.tm_hour,
            calendar.tm_min,
            calendar.tm_sec,
            DAYS[calendar.tm_wday as usize]
        )
        .unwrap();
        Ok(date)
    }
    #[cfg(not(unix))]
    {
        Err(StatusError::UnsupportedPlatform)
    }
}

pub(super) fn produce(
    capture: Arc<WorkspaceStatusCapture>,
    artifacts: Arc<WorkspaceStatusArtifacts>,
) -> Result<Arc<WorkspaceStatusContent>, StatusError> {
    let raw = &capture.raw;
    if raw.policy != StatusPolicy::DefaultEnglishUtc {
        return Err(StatusError::UnsupportedPolicy);
    }
    if raw.source_date_epoch != ObservedText::Absent {
        return Err(StatusError::SourceDateEpochPresent);
    }
    if raw.source != StatusSource::UnixReportedUserKernelHost {
        return Err(StatusError::UnsupportedPlatform);
    }
    if artifacts.stable().root().workspace() != &capture.workspace {
        return Err(StatusError::ForeignCapture);
    }
    let user = raw.user.identity()?;
    let full_host = raw.hostname.identity()?;
    let host = full_host.split('.').next().unwrap();
    if host.is_empty() {
        return Err(StatusError::InvalidIdentity);
    }
    raw.uid?;
    let millis = raw.millis?;
    if millis > MAX_MILLIS {
        return Err(StatusError::TimeRange);
    }
    let seconds = millis / 1000;
    let date = formatted_date(seconds)?;
    // Fixed sorted fields are borrowed. Only the two independent output buffers
    // are retained; neither a full map nor globally interned clock text is built.
    let mut stable = Vec::with_capacity(43 + user.len() + host.len());
    stable.extend_from_slice(b"BUILD_EMBED_LABEL \nBUILD_HOST ");
    stable.extend_from_slice(host.as_bytes());
    stable.extend_from_slice(b"\nBUILD_USER ");
    stable.extend_from_slice(user.as_bytes());
    stable.push(b'\n');
    // Fixed keys, two LFs, a 24-byte date and at most 12 timestamp digits.
    let mut volatile = Vec::with_capacity(69);
    writeln!(volatile, "BUILD_TIMESTAMP {seconds}\nFORMATTED_DATE {date}").unwrap();
    Ok(Arc::new(WorkspaceStatusContent {
        capture,
        artifacts,
        stable,
        volatile,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct WorkspaceStatusContentKey(pub(super) NormalizedAbsolutePath);

impl fmt::Display for WorkspaceStatusContentKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "workspace-status-content:{}", self.0)
    }
}

#[async_trait]
impl Key for WorkspaceStatusContentKey {
    type Value = Result<Arc<WorkspaceStatusContent>, StatusError>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        ctx.store_evaluation_data(EventBatch::empty())
            .expect("one status content batch");
        let value = ctx
            .compute(&WorkspaceStatusCaptureKey(self.0.dupe()))
            .await
            .map_err(|_| StatusError::Unauthorized)?;
        let WorkspaceStatusCaptureValue::Present(capture) = value else {
            return Err(StatusError::Unauthorized);
        };
        let carrier = ctx
            .per_transaction_data()
            .data
            .get::<WorkspaceStatusTransaction>()
            .map_err(|_| StatusError::ForeignCapture)?;
        if capture.workspace != self.0 || !Arc::ptr_eq(&capture, &carrier.0) {
            return Err(StatusError::ForeignCapture);
        }
        let artifacts = ctx
            .compute(&WorkspaceStatusArtifactsKey(self.0.dupe()))
            .await
            .map_err(|_| StatusError::Unauthorized)?;
        produce(capture, artifacts)
    }
    fn equality(left: &Self::Value, right: &Self::Value) -> bool {
        left.is_ok() && right.is_ok() && left == right
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_ok()
    }
}
