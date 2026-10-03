//! Test-only transport for the externally owned admission process. No source authority.
use std::ffi::OsStr;
use std::fs::File;
use std::fs::OpenOptions;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

const WORK_ENV: &str = "SLUG_SOURCE_ADMISSION_WORK_DEADLINE_MONOTONIC_NS";
const CLEANUP_ENV: &str = "SLUG_SOURCE_ADMISSION_CLEANUP_DEADLINE_MONOTONIC_NS";
const STAGE_ENV: &str = "SLUG_SOURCE_ADMISSION_STAGE_ROOT";
const WORK_NS: u64 = 27_000_000_000;
const CLEANUP_NS: u64 = 3_000_000_000;
const BRACKET: Duration = Duration::from_millis(10);
const MAX_PATH: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HarnessSetupError {
    MissingTransport,
    InvalidInteger,
    InvalidClock,
    Expired,
    EnvelopeExceeded,
    ClockSamplingStall,
    InstantOverflow,
    InvalidStage,
    StageChanged,
}
impl std::fmt::Display for HarnessSetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // No transported path, environment value or arbitrary OS message is logged.
        write!(f, "source admission harness setup: {self:?}")
    }
}
impl std::error::Error for HarnessSetupError {}

type SetupResult<T> = Result<T, HarnessSetupError>;

#[derive(Clone, Copy)]
struct ClockSample {
    before: Instant,
    ns: u64,
    after: Instant,
}

/// The retained FD is an operational stage pin, never a source-root capability.
pub(super) struct HarnessClockStage {
    work_ns: u64,
    cleanup_ns: u64,
    work: Instant,
    cleanup: Instant,
    stage_path: PathBuf,
    stage: File,
    device: u64,
    inode: u64,
}
impl HarnessClockStage {
    pub(super) fn from_environment() -> SetupResult<Self> {
        // Startup-only reads. The helper never changes process environment.
        let work = std::env::var_os(WORK_ENV);
        let cleanup = std::env::var_os(CLEANUP_ENV);
        let stage = std::env::var_os(STAGE_ENV);
        let tmpdir = std::env::var_os("TMPDIR");
        Self::from_optional_inputs([
            work.as_deref(),
            cleanup.as_deref(),
            stage.as_deref(),
            tmpdir.as_deref(),
        ])
    }

    fn from_optional_inputs(values: [Option<&OsStr>; 4]) -> SetupResult<Self> {
        let [Some(work), Some(cleanup), Some(stage), Some(tmpdir)] = values else {
            return Err(HarnessSetupError::MissingTransport);
        };
        Self::from_inputs(work, cleanup, stage, tmpdir)
    }

    fn from_inputs(
        work: &OsStr,
        cleanup: &OsStr,
        stage: &OsStr,
        tmpdir: &OsStr,
    ) -> SetupResult<Self> {
        let (work_ns, cleanup_ns) = parse_deadlines(work, cleanup)?;
        let (work, cleanup) = translate(work_ns, cleanup_ns, sample_clock()?)?;
        validate_stage_spelling(stage, tmpdir)?;
        let path = Path::new(stage);
        let opened = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)
            .map_err(|_| HarnessSetupError::InvalidStage)?;
        let metadata = opened
            .metadata()
            .map_err(|_| HarnessSetupError::InvalidStage)?;
        validate_stage_metadata(&metadata)?;
        let value = Self {
            work_ns,
            cleanup_ns,
            work,
            cleanup,
            // The borrowed path was bounded before this owned allocation.
            stage_path: path.to_owned(),
            device: metadata.dev(),
            inode: metadata.ino(),
            stage: opened,
        };
        value.revalidate_before_runtime()?;
        Ok(value)
    }

    pub(super) fn work_deadline(&self) -> Instant {
        self.work
    }
    pub(super) fn cleanup_deadline(&self) -> Instant {
        self.cleanup
    }
    pub(super) fn work_deadline_monotonic_ns(&self) -> u64 {
        self.work_ns
    }
    pub(super) fn cleanup_deadline_monotonic_ns(&self) -> u64 {
        self.cleanup_ns
    }
    pub(super) fn stage_path(&self) -> &Path {
        &self.stage_path
    }

    pub(super) fn revalidate_before_runtime(&self) -> SetupResult<()> {
        let sample = sample_clock()?;
        validate_bracket(sample)?;
        // Original transport and translated clocks are both checked; neither is restarted.
        validate_envelope(self.work_ns, self.cleanup_ns, sample.ns)?;
        if sample.after >= self.work || sample.after >= self.cleanup {
            return Err(HarnessSetupError::Expired);
        }
        self.revalidate_stage()?;
        if Instant::now() >= self.work {
            return Err(HarnessSetupError::Expired);
        }
        Ok(())
    }

    /// This check remains usable at exit after the acquisition clock has expired.
    pub(super) fn revalidate_stage(&self) -> SetupResult<()> {
        let held = self
            .stage
            .metadata()
            .map_err(|_| HarnessSetupError::StageChanged)?;
        let named = std::fs::symlink_metadata(&self.stage_path)
            .map_err(|_| HarnessSetupError::StageChanged)?;
        validate_stage_metadata(&held).map_err(|_| HarnessSetupError::StageChanged)?;
        validate_stage_metadata(&named).map_err(|_| HarnessSetupError::StageChanged)?;
        if held.dev() != self.device
            || held.ino() != self.inode
            || named.dev() != self.device
            || named.ino() != self.inode
        {
            return Err(HarnessSetupError::StageChanged);
        }
        Ok(())
    }
}

fn decimal(value: &OsStr) -> SetupResult<u64> {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 20 || !bytes.iter().all(u8::is_ascii_digit) {
        return Err(HarnessSetupError::InvalidInteger);
    }
    let mut number = 0_u64;
    for byte in bytes {
        number = number
            .checked_mul(10)
            .and_then(|n| n.checked_add(u64::from(byte - b'0')))
            .ok_or(HarnessSetupError::InvalidInteger)?;
    }
    if number == 0 {
        return Err(HarnessSetupError::InvalidInteger);
    }
    Ok(number)
}

fn parse_deadlines(work: &OsStr, cleanup: &OsStr) -> SetupResult<(u64, u64)> {
    let work = decimal(work)?;
    let cleanup = decimal(cleanup)?;
    if cleanup.checked_sub(work).is_none_or(|gap| gap > CLEANUP_NS) {
        return Err(HarnessSetupError::EnvelopeExceeded);
    }
    Ok((work, cleanup))
}

fn validate_envelope(work: u64, cleanup: u64, now: u64) -> SetupResult<()> {
    if work <= now {
        return Err(HarnessSetupError::Expired);
    }
    if work - now > WORK_NS || cleanup.checked_sub(work).is_none_or(|gap| gap > CLEANUP_NS) {
        return Err(HarnessSetupError::EnvelopeExceeded);
    }
    Ok(())
}

fn timespec_ns(seconds: libc::time_t, nanos: libc::c_long) -> SetupResult<u64> {
    let seconds = u64::try_from(seconds).map_err(|_| HarnessSetupError::InvalidClock)?;
    let nanos = u64::try_from(nanos).map_err(|_| HarnessSetupError::InvalidClock)?;
    if nanos >= 1_000_000_000 {
        return Err(HarnessSetupError::InvalidClock);
    }
    seconds
        .checked_mul(1_000_000_000)
        .and_then(|s| s.checked_add(nanos))
        .ok_or(HarnessSetupError::InvalidClock)
}

fn clock_ns() -> SetupResult<u64> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // Linux profile only; the parent registers this module under cfg(test,Linux).
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) } != 0 {
        return Err(HarnessSetupError::InvalidClock);
    }
    timespec_ns(time.tv_sec, time.tv_nsec)
}

fn sample_clock() -> SetupResult<ClockSample> {
    let before = Instant::now();
    let ns = clock_ns()?;
    let after = Instant::now();
    Ok(ClockSample { before, ns, after })
}

fn translate(work: u64, cleanup: u64, sample: ClockSample) -> SetupResult<(Instant, Instant)> {
    validate_envelope(work, cleanup, sample.ns)?;
    validate_bracket(sample)?;
    // BEFORE, not AFTER, makes the translation conservative by the sampling latency.
    let work = add_remaining(sample.before, Duration::from_nanos(work - sample.ns))?;
    let cleanup = add_remaining(sample.before, Duration::from_nanos(cleanup - sample.ns))?;
    if work <= sample.after {
        return Err(HarnessSetupError::Expired);
    }
    Ok((work, cleanup))
}

fn validate_bracket(sample: ClockSample) -> SetupResult<()> {
    let width = sample
        .after
        .checked_duration_since(sample.before)
        .ok_or(HarnessSetupError::InvalidClock)?;
    if width > BRACKET {
        return Err(HarnessSetupError::ClockSamplingStall);
    }
    Ok(())
}

fn add_remaining(before: Instant, remaining: Duration) -> SetupResult<Instant> {
    before
        .checked_add(remaining)
        .ok_or(HarnessSetupError::InstantOverflow)
}

fn validate_stage_spelling(stage: &OsStr, tmpdir: &OsStr) -> SetupResult<()> {
    let bytes = stage.as_bytes();
    if stage != tmpdir || bytes.len() > MAX_PATH || bytes.len() < 2 || bytes[0] != b'/' {
        return Err(HarnessSetupError::InvalidStage);
    }
    if bytes.contains(&0)
        || bytes[1..]
            .split(|b| *b == b'/')
            .any(|part| part.is_empty() || part == b"." || part == b"..")
    {
        return Err(HarnessSetupError::InvalidStage);
    }
    Ok(())
}

fn validate_stage_metadata(metadata: &std::fs::Metadata) -> SetupResult<()> {
    validate_stage_attributes(
        metadata.file_type().is_dir(),
        metadata.mode(),
        metadata.uid(),
        unsafe { libc::geteuid() },
    )
}

fn validate_stage_attributes(
    directory: bool,
    mode: u32,
    uid: u32,
    current_uid: u32,
) -> SetupResult<()> {
    if !directory || mode & 0o7777 != 0o700 || uid != current_uid {
        return Err(HarnessSetupError::InvalidStage);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::fs::symlink;

    use super::*;

    #[test]
    fn missing_transport_and_invalid_stage_do_not_invoke_constructor() {
        let placeholder = OsStr::new("invalid");
        let called = Cell::new(0);
        for missing in 0..4 {
            let mut values = [Some(placeholder); 4];
            values[missing] = None;
            let result = HarnessClockStage::from_optional_inputs(values);
            if result.is_ok() {
                called.set(called.get() + 1);
            }
            assert!(matches!(result, Err(HarnessSetupError::MissingTransport)));
        }
        let now = clock_ns().unwrap();
        let work = (now + 5_000_000_000).to_string();
        let cleanup = (now + 6_000_000_000).to_string();
        let invalid = HarnessClockStage::from_inputs(
            OsStr::new(&work),
            OsStr::new(&cleanup),
            OsStr::new("/a/../b"),
            OsStr::new("/a/../b"),
        );
        if invalid.is_ok() {
            called.set(called.get() + 1);
        }
        assert!(matches!(invalid, Err(HarnessSetupError::InvalidStage)));
        assert_eq!(called.get(), 0);
        assert_eq!(
            validate_stage_attributes(true, 0o700, 41, 42),
            Err(HarnessSetupError::InvalidStage)
        );
        assert_eq!(
            validate_stage_attributes(true, 0o1700, 42, 42),
            Err(HarnessSetupError::InvalidStage)
        );
        assert_eq!(validate_stage_attributes(true, 0o700, 42, 42), Ok(()));
    }

    #[test]
    fn strict_transport_integer_and_exact_envelope_boundaries() {
        for value in [
            b"".as_slice(),
            b"0",
            b"+1",
            b"-1",
            b" 1",
            b"1 ",
            b"1\0",
            b"a",
            b"\xff",
            b"18446744073709551616",
            b"000000000000000000001",
        ] {
            assert_eq!(
                decimal(OsStr::from_bytes(value)),
                Err(HarnessSetupError::InvalidInteger)
            );
        }
        assert_eq!(decimal(OsStr::new("18446744073709551615")), Ok(u64::MAX));
        assert_eq!(
            parse_deadlines(OsStr::new("10"), OsStr::new("9")),
            Err(HarnessSetupError::EnvelopeExceeded)
        );
        assert_eq!(
            parse_deadlines(OsStr::new("10"), OsStr::new("3000000010")),
            Ok((10, 3_000_000_010))
        );
        assert_eq!(
            parse_deadlines(OsStr::new("10"), OsStr::new("3000000011")),
            Err(HarnessSetupError::EnvelopeExceeded)
        );
        assert_eq!(
            validate_envelope(1 + WORK_NS, 1 + WORK_NS + CLEANUP_NS, 1),
            Ok(())
        );
        assert_eq!(
            validate_envelope(2 + WORK_NS, 2 + WORK_NS, 1),
            Err(HarnessSetupError::EnvelopeExceeded)
        );
        assert_eq!(validate_envelope(1, 2, 1), Err(HarnessSetupError::Expired));
        assert_eq!(validate_envelope(1, 1, 2), Err(HarnessSetupError::Expired));
    }

    #[test]
    fn clock_translation_before_base_consumes_latency_and_never_restarts() {
        let before = Instant::now();
        let sample = ClockSample {
            before,
            ns: 1_000_000_000,
            after: before + Duration::from_millis(5),
        };
        let (work, cleanup) = translate(
            sample.ns + WORK_NS,
            sample.ns + WORK_NS + CLEANUP_NS,
            sample,
        )
        .unwrap();
        assert_eq!(work, before + Duration::from_secs(27));
        assert_eq!(cleanup - work, Duration::from_secs(3));
        assert_eq!(work - sample.after, Duration::from_millis(26_995));
        let advanced = ClockSample {
            before: before + Duration::from_secs(1),
            ns: sample.ns + 1_000_000_000,
            after: before + Duration::from_millis(1_005),
        };
        assert_eq!(
            translate(
                sample.ns + WORK_NS,
                sample.ns + WORK_NS + CLEANUP_NS,
                advanced
            )
            .unwrap(),
            (work, cleanup)
        );
        let reversed = ClockSample {
            after: before - Duration::from_millis(1),
            ..sample
        };
        assert_eq!(
            translate(sample.ns + WORK_NS, sample.ns + WORK_NS, reversed),
            Err(HarnessSetupError::InvalidClock)
        );
        let stalled = ClockSample {
            after: before + BRACKET + Duration::from_nanos(1),
            ..sample
        };
        assert_eq!(
            translate(sample.ns + WORK_NS, sample.ns + WORK_NS, stalled),
            Err(HarnessSetupError::ClockSamplingStall)
        );
        let expired_during_bracket = sample.ns + 1;
        assert_eq!(
            translate(expired_during_bracket, expired_during_bracket, sample),
            Err(HarnessSetupError::Expired)
        );
        assert_eq!(timespec_ns(-1, 0), Err(HarnessSetupError::InvalidClock));
        assert_eq!(timespec_ns(0, -1), Err(HarnessSetupError::InvalidClock));
        assert_eq!(
            timespec_ns(0, 1_000_000_000),
            Err(HarnessSetupError::InvalidClock)
        );
        assert_eq!(
            timespec_ns(libc::time_t::MAX, 0),
            Err(HarnessSetupError::InvalidClock)
        );
        assert_eq!(timespec_ns(7, 9), Ok(7_000_000_009));
        assert_eq!(
            add_remaining(before, Duration::MAX),
            Err(HarnessSetupError::InstantOverflow)
        );
        let exact_bracket = ClockSample {
            after: before + BRACKET,
            ..sample
        };
        assert!(translate(sample.ns + WORK_NS, sample.ns + WORK_NS, exact_bracket).is_ok());
    }

    fn transport(stage: &Path) -> HarnessClockStage {
        let now = clock_ns().unwrap();
        HarnessClockStage::from_inputs(
            OsStr::new(&(now + 5_000_000_000).to_string()),
            OsStr::new(&(now + 6_000_000_000).to_string()),
            stage.as_os_str(),
            stage.as_os_str(),
        )
        .unwrap()
    }

    #[test]
    fn stage_pin_real_clock_and_constructor_order_without_environment_mutation() {
        let stage = tempfile::tempdir().unwrap();
        // Python mkdtemp supplies the0700 profile; tempfile's mode is umask-dependent.
        std::fs::set_permissions(stage.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let value = transport(stage.path());
        value.revalidate_before_runtime().unwrap();
        assert_eq!(value.stage_path(), stage.path());
        assert_eq!(
            value.cleanup_deadline() - value.work_deadline(),
            Duration::from_secs(1)
        );
        assert_eq!(
            value.cleanup_deadline_monotonic_ns() - value.work_deadline_monotonic_ns(),
            1_000_000_000
        );
        // At a later raw sample, translated remaining time cannot exceed original remaining time.
        let later_before = Instant::now();
        let later_ns = clock_ns().unwrap();
        assert!(
            value.work_deadline().duration_since(later_before)
                <= Duration::from_nanos(value.work_deadline_monotonic_ns() - later_ns)
                    + later_before.elapsed()
        );
        let called = Cell::new(0);
        let observed = (|| {
            value.revalidate_before_runtime()?;
            called.set(called.get() + 1);
            Ok::<_, HarnessSetupError>((value.work_deadline(), value.cleanup_deadline()))
        })()
        .unwrap();
        assert_eq!(called.get(), 1);
        assert_eq!(observed, (value.work, value.cleanup));
        let now = clock_ns().unwrap();
        let expired = HarnessClockStage::from_inputs(
            OsStr::new(&now.to_string()),
            OsStr::new(&now.to_string()),
            stage.path().as_os_str(),
            stage.path().as_os_str(),
        );
        if expired.is_ok() {
            called.set(called.get() + 1);
        }
        assert!(matches!(expired, Err(HarnessSetupError::Expired)));
        assert_eq!(called.get(), 1);
        // Explicit tempdir_in does not alter startup TMPDIR or grant a source capability.
        let temporary = tempfile::tempdir_in(value.stage_path()).unwrap();
        assert_eq!(temporary.path().parent(), Some(value.stage_path()));
        drop(temporary);
        value.revalidate_stage().unwrap();
    }

    #[test]
    fn strict_stage_spelling_link_mode_file_and_replacement_refuse() {
        for path in [
            "", "/", "relative", "/a/.", "/a/../b", "/a//b", "/a/", "/a\0b",
        ] {
            assert_eq!(
                validate_stage_spelling(OsStr::new(path), OsStr::new(path)),
                Err(HarnessSetupError::InvalidStage)
            );
        }
        assert_eq!(
            validate_stage_spelling(OsStr::new("/a"), OsStr::new("/b")),
            Err(HarnessSetupError::InvalidStage)
        );
        let oversized = format!("/{}", "a".repeat(MAX_PATH));
        assert_eq!(
            validate_stage_spelling(OsStr::new(&oversized), OsStr::new(&oversized)),
            Err(HarnessSetupError::InvalidStage)
        );
        let outer = tempfile::tempdir().unwrap();
        let stage = tempfile::tempdir_in(outer.path()).unwrap();
        std::fs::set_permissions(stage.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let value = transport(stage.path());
        let link = outer.path().join("link");
        symlink(stage.path(), &link).unwrap();
        let now = clock_ns().unwrap();
        let work = (now + 5_000_000_000).to_string();
        let cleanup = (now + 6_000_000_000).to_string();
        assert!(matches!(
            HarnessClockStage::from_inputs(
                OsStr::new(&work),
                OsStr::new(&cleanup),
                link.as_os_str(),
                link.as_os_str()
            ),
            Err(HarnessSetupError::InvalidStage)
        ));
        let file = outer.path().join("file");
        std::fs::write(&file, b"x").unwrap();
        assert!(matches!(
            HarnessClockStage::from_inputs(
                OsStr::new(&work),
                OsStr::new(&cleanup),
                file.as_os_str(),
                file.as_os_str()
            ),
            Err(HarnessSetupError::InvalidStage)
        ));
        std::fs::set_permissions(stage.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            value.revalidate_stage(),
            Err(HarnessSetupError::StageChanged)
        );
        assert!(matches!(
            HarnessClockStage::from_inputs(
                OsStr::new(&work),
                OsStr::new(&cleanup),
                stage.path().as_os_str(),
                stage.path().as_os_str()
            ),
            Err(HarnessSetupError::InvalidStage)
        ));
        std::fs::set_permissions(stage.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        value.revalidate_stage().unwrap();
        let moved = outer.path().join("moved");
        std::fs::rename(stage.path(), &moved).unwrap();
        std::fs::create_dir(stage.path()).unwrap();
        std::fs::set_permissions(stage.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            value.revalidate_stage(),
            Err(HarnessSetupError::StageChanged)
        );
    }
}
