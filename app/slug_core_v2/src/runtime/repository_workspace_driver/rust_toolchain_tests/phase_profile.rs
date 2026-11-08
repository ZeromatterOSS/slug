//! Fixed scalar diagnostics over the existing observer ABI, never graph facts.
use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use serde_json::Value;
use serde_json::json;

use crate::runtime::WorkspaceRuntime;
use crate::runtime::probe_observer::ProbeGuard;

pub(super) const REPORT_BYTES: usize = 4096;
const WORDS: usize = 64;
const MAGIC: u64 = u64::from_le_bytes(*b"SLGOBS01");

pub(super) struct InstalledProfile {
    guard: ProbeGuard,
}
impl InstalledProfile {
    pub(super) fn attach(runtime: &mut WorkspaceRuntime) -> io::Result<Self> {
        let raw = unsafe {
            libc::memfd_create(
                c"slug-rust-phase-profile".as_ptr(),
                libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
            )
        };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        if unsafe { libc::ftruncate(fd.as_raw_fd(), 512) } != 0
            || unsafe {
                libc::fcntl(
                    fd.as_raw_fd(),
                    libc::F_ADD_SEALS,
                    libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL,
                )
            } != 0
        {
            return Err(io::Error::last_os_error());
        }
        let guard = ProbeGuard::install_from_fd(fd)?;
        runtime.attach_probe_observer(guard.observer());
        Ok(Self { guard })
    }
    /// Called only after the outcome and runtime have dropped and native owners
    /// have refunded. Disable first, then sample the still-owned atomic mapping.
    pub(super) fn finish(self) -> Value {
        let observer = self.guard.observer();
        drop(self.guard);
        let words = std::array::from_fn(|i| observer.words_for_test()[i].load(Ordering::Acquire));
        decode(&words)
    }
}

fn decode(words: &[u64; WORDS]) -> Value {
    // Existing scalar ABI V2: never decode phase/activity frames (words14..50).
    if words[0] != MAGIC || words[1] != 2 {
        return json!({"valid_abi":false});
    }
    let names = [
        "injection_revision",
        "root_compute",
        "legacy_need",
        "idle_drain",
    ];
    let intervals = names
        .iter()
        .enumerate()
        .map(|(category, name)| {
            let base = 51 + category * 3;
            let status = (words[63] >> (category * 12)) & 0xfff;
            json!({
                "kind":name,"closed_count":words[base],"wall_ns":words[base+1],
                "process_cpu_ns":words[base+2],"status":status,
                "missing_wall":status&1!=0,"missing_cpu":status&2!=0,
                "reversed_wall":status&4!=0,"reversed_cpu":status&8!=0,
                "count_overflow":status&16!=0,"wall_overflow":status&32!=0,
                "cpu_overflow":status&64!=0,"aborted":status&128!=0,
                "suppressed":status&256!=0,"unknown_status_bits":status&!0x1ff,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "valid_abi":true,"version":2,"disabled":words[3],
        "event_overflow":words[4],"sampled_event_dropped":words[5],
        "unknown_interval_status_bits":words[63]>>48,"intervals":intervals,
        "runtime_construction_measured":false,"cpu_includes_helpers":false,
        "closed_count_includes_aborted":true,
        "categories_are_exclusive":false,
    })
}

#[derive(Default, Clone, Copy)]
struct Bucket {
    count: u64,
    wall_ns: u64,
    max_wall_ns: u64,
}
#[derive(Default)]
pub(super) struct ProgressTiming {
    buckets: [Bucket; 7],
    pending: Option<(usize, Instant)>,
    pairing_mismatch: bool,
    incomplete: bool,
    clock_reversed: bool,
    overflow: bool,
}
impl ProgressTiming {
    pub(super) fn begin(&mut self, attempt: usize) {
        self.begin_at(attempt, Instant::now());
    }
    fn begin_at(&mut self, attempt: usize, start: Instant) {
        if self.pending.replace((attempt, start)).is_some() {
            self.pairing_mismatch = true;
            self.incomplete = true;
        }
    }
    pub(super) fn finish(&mut self, attempt: usize, kind: usize, end: Instant) {
        let Some((expected, start)) = self.pending.take() else {
            self.pairing_mismatch = true;
            self.incomplete = true;
            return;
        };
        if expected != attempt || kind >= self.buckets.len() {
            self.pairing_mismatch = true;
            self.incomplete = true;
            return;
        }
        let Some(duration) = end.checked_duration_since(start) else {
            self.clock_reversed = true;
            self.incomplete = true;
            return;
        };
        self.fold(kind, duration);
    }
    fn fold(&mut self, kind: usize, duration: Duration) {
        let bucket = &mut self.buckets[kind];
        let Ok(ns) = u64::try_from(duration.as_nanos()) else {
            self.overflow = true;
            return;
        };
        let Some((count, wall)) = bucket
            .count
            .checked_add(1)
            .zip(bucket.wall_ns.checked_add(ns))
        else {
            self.overflow = true;
            return;
        };
        bucket.count = count;
        bucket.wall_ns = wall;
        bucket.max_wall_ns = bucket.max_wall_ns.max(ns);
    }
    pub(super) fn summary(&self) -> Value {
        json!({
            "buckets":self.buckets.iter().enumerate().map(|(kind,b)|json!({
                "kind":super::progress_name(kind),"closed_count":b.count,
                "wall_ns":b.wall_ns,"max_wall_ns":b.max_wall_ns,
            })).collect::<Vec<_>>(),
            "pairing_mismatch":self.pairing_mismatch,"incomplete":self.incomplete||self.pending.is_some(),
            "clock_reversed":self.clock_reversed,"overflow":self.overflow,
            "includes_fixed_callback_dispatch_overhead":true,
            "overlaps_legacy_need":true,
        })
    }
}

#[test]
fn phase_profile_scalar_v2_qualifiers_and_progress_pairing_are_bounded() {
    let mut words = [0; WORDS];
    words[0] = MAGIC;
    words[1] = 2;
    words[3] = 1;
    words[4] = 4;
    words[5] = 3;
    words[54] = 2;
    words[55] = 23;
    words[56] = 7;
    words[63] = (128 | 32 | 2) << 12;
    words[14..51].fill(u64::MAX);
    let decoded = decode(&words);
    assert_eq!(decoded["intervals"][1]["closed_count"], 2);
    assert_eq!(decoded["intervals"][1]["wall_ns"], 23);
    assert_eq!(decoded["intervals"][1]["aborted"], true);
    assert_eq!(decoded["intervals"][1]["wall_overflow"], true);
    assert_eq!(decoded["intervals"][1]["missing_cpu"], true);
    assert_eq!(decoded["event_overflow"], 4);
    assert_eq!(decoded["sampled_event_dropped"], 3);
    assert!(!decoded.to_string().contains(&u64::MAX.to_string()));
    words[1] = 3;
    assert_eq!(decode(&words), json!({"valid_abi":false}));
    let start = Instant::now();
    let mut timing = ProgressTiming::default();
    timing.begin_at(1, start);
    timing.finish(1, 5, start + Duration::from_nanos(11));
    timing.begin_at(2, start);
    timing.finish(2, 6, start + Duration::from_nanos(3));
    assert_eq!(timing.summary()["buckets"][5]["wall_ns"], 11);
    assert_eq!(timing.summary()["buckets"][6]["closed_count"], 1);
    timing.begin_at(3, start);
    timing.finish(4, 0, start);
    assert!(timing.pairing_mismatch && timing.incomplete);
    timing.buckets[0].wall_ns = u64::MAX;
    timing.fold(0, Duration::from_nanos(1));
    assert!(timing.overflow);
    assert_eq!(timing.buckets[0].wall_ns, u64::MAX);
    timing.begin_at(5, start);
    assert_eq!(timing.summary()["incomplete"], true);
    assert!(decoded.to_string().len() + timing.summary().to_string().len() < REPORT_BYTES);
}
