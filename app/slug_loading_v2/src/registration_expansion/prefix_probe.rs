//! Fixed scalars from actual observed module-toolchain Need returns.

#[cfg(feature = "native-probe-observer")]
use std::sync::atomic::AtomicU64;
#[cfg(feature = "native-probe-observer")]
use std::sync::atomic::Ordering;

#[cfg(feature = "native-probe-observer")]
static COUNTS: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
#[cfg(feature = "native-probe-observer")]
static OVERFLOW: AtomicU64 = AtomicU64::new(0);

/// Process-wide, non-atomic snapshot of Need returns, Need returns preceded by
/// a completed prefix, and completed prefix rows discarded by those returns.
#[cfg(feature = "native-probe-observer")]
#[derive(Clone, Copy)]
pub struct ModuleToolchainNeedPrefixSnapshot {
    pub counts: [u64; 3],
    pub overflow_mask: u64,
}

#[cfg(feature = "native-probe-observer")]
pub fn module_toolchain_need_prefix_snapshot() -> ModuleToolchainNeedPrefixSnapshot {
    ModuleToolchainNeedPrefixSnapshot {
        counts: COUNTS
            .each_ref()
            .map(|counter| counter.load(Ordering::Relaxed)),
        overflow_mask: OVERFLOW.load(Ordering::Relaxed),
    }
}

#[cfg(feature = "native-probe-observer")]
pub(super) fn record(counts: [u64; 3]) {
    for (index, (counter, count)) in COUNTS.iter().zip(counts).enumerate() {
        if count == 0 {
            continue;
        }
        let previous = counter
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                Some(value.saturating_add(count))
            })
            .expect("the saturating update always returns Some");
        if previous.checked_add(count).is_none() {
            OVERFLOW.fetch_or(1 << index, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct TestWitness(std::sync::Arc<std::sync::Mutex<[u64; 3]>>);

#[cfg(test)]
impl TestWitness {
    pub(super) fn record(&self, counts: [u64; 3]) {
        for (total, count) in self.0.lock().unwrap().iter_mut().zip(counts) {
            *total += count;
        }
    }

    pub(crate) fn counts(&self) -> [u64; 3] {
        *self.0.lock().unwrap()
    }
}
