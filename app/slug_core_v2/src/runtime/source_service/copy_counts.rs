//! Fixed, nonsemantic regular-file conversion traffic; no retained-byte accounting.
use std::io;
use std::io::Write;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::runtime) struct CopyCounts {
    pub(super) created: u64,
    pub(super) returned: u64,
    pub(super) len_sum: u64,
    pub(super) max_len: u64,
    pub(super) max_capture_capacity: u64,
    pub(super) armed: bool,
    pub(super) overflow: bool,
}
impl CopyCounts {
    fn add(value: &mut u64, amount: u64, overflow: &mut bool) {
        *value = value.checked_add(amount).unwrap_or_else(|| {
            *overflow = true;
            u64::MAX
        });
    }
    pub(super) fn created(&mut self, length: u64, capacity: u64) {
        if self.armed {
            Self::add(&mut self.created, 1, &mut self.overflow);
            Self::add(&mut self.len_sum, length, &mut self.overflow);
            self.max_len = self.max_len.max(length);
            self.max_capture_capacity = self.max_capture_capacity.max(capacity);
        }
    }
    pub(super) fn returned(&mut self) {
        if self.armed {
            Self::add(&mut self.returned, 1, &mut self.overflow);
        }
    }
    pub(in crate::runtime) fn write(&self, writer: &mut impl Write) -> io::Result<()> {
        writeln!(
            writer,
            "WP1127_CAPTURE_COPY created={} returned={} len_sum={} max_len={} max_capture_capacity={} armed={} overflow={} traffic_only=true retained_bytes=unavailable",
            self.created,
            self.returned,
            self.len_sum,
            self.max_len,
            self.max_capture_capacity,
            self.armed,
            self.overflow
        )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;
    use std::time::Instant;

    use slug_workspace_v2::NormalizedAbsolutePath;

    use super::super::super::regular_file::Capture;
    use super::super::super::regular_file::ReadPolicy;
    use super::super::SourceServiceError;
    use super::super::SourceServiceLimits;
    use super::super::SourceServiceScope;
    use super::*;

    fn owner(live: usize) -> Arc<SourceServiceScope> {
        let deadline = Instant::now() + Duration::from_secs(2);
        SourceServiceScope::new(
            NormalizedAbsolutePath::new("/copy-test").unwrap(),
            deadline,
            deadline + Duration::from_secs(1),
            SourceServiceLimits {
                registry_object_bytes: 128,
                live_buffer_bytes: live,
                ..SourceServiceLimits::default()
            },
        )
        .unwrap()
    }
    fn capture(scope: &Arc<SourceServiceScope>, bytes: &[u8]) -> Capture {
        let mut capture = Capture::new(scope, ReadPolicy::Registry).unwrap();
        capture.append(scope, bytes).unwrap();
        capture
    }

    #[test]
    fn capture_copy_traffic_preserves_real_errors_credit_and_lifetimes() {
        let scope = owner(512);
        assert_eq!(scope.copy_counts_for_test(), CopyCounts::default());
        drop(capture(&scope, b"unarmed").finish(&scope).unwrap());
        assert_eq!(scope.copy_counts_for_test(), CopyCounts::default());
        scope.arm_copy_counts_for_test();
        let value = capture(&scope, b"copied").finish(&scope).unwrap();
        drop(capture(&scope, b"").finish(&scope).unwrap());
        let counts = scope.copy_counts_for_test();
        assert_eq!(
            (
                counts.created,
                counts.returned,
                counts.len_sum,
                counts.max_len
            ),
            (2, 2, 6, 6)
        );
        assert_eq!(counts.max_capture_capacity, 128);
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
        let weak = Arc::downgrade(&value);
        let scope_weak = Arc::downgrade(&scope);
        drop(scope);
        assert!(scope_weak.upgrade().is_none());
        assert_eq!(value.as_ref(), b"copied");
        assert!(weak.upgrade().is_some());
        drop(value);
        assert!(weak.upgrade().is_none());
        for after_arc in [false, true] {
            let scope = owner(512);
            scope.arm_copy_counts_for_test();
            let capture = capture(&scope, b"cancel");
            if !after_arc {
                scope.cancel();
            }
            let mut result_weak = None;
            let result = capture.finish_with(&scope, |value| {
                result_weak = Some(Arc::downgrade(value));
                scope.cancel();
            });
            assert_eq!(result.unwrap_err(), SourceServiceError::Cancelled);
            let counts = scope.copy_counts_for_test();
            assert_eq!(
                (counts.created, counts.returned, counts.len_sum),
                (u64::from(after_arc), 0, if after_arc { 6 } else { 0 })
            );
            assert_eq!(result_weak.is_some(), after_arc);
            assert!(result_weak.is_none_or(|weak| weak.upgrade().is_none()));
            assert_eq!(scope.snapshot().live_buffer_bytes, 0);
            assert_eq!(scope.snapshot().registry_admitted_bytes, 6);
            assert_eq!(
                scope.first_error_for_test(),
                Some(SourceServiceError::Cancelled)
            );
        }
        let scope = owner(160);
        scope.arm_copy_counts_for_test();
        assert_eq!(
            capture(&scope, b"refuse").finish(&scope).unwrap_err(),
            SourceServiceError::BufferLimit
        );
        assert_eq!(scope.copy_counts_for_test().created, 0);
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
        assert_eq!(
            scope.first_error_for_test(),
            Some(SourceServiceError::BufferLimit)
        );
    }

    #[test]
    fn capture_copy_traffic_is_fixed_saturating_and_scope_local() {
        let scope = owner(4096);
        scope.arm_copy_counts_for_test();
        std::thread::scope(|threads| {
            for _ in 0..4 {
                threads.spawn(|| {
                    for _ in 0..8 {
                        drop(capture(&scope, b"12345678").finish(&scope).unwrap());
                    }
                });
            }
        });
        let counts = scope.copy_counts_for_test();
        assert_eq!(
            (
                counts.created,
                counts.returned,
                counts.len_sum,
                counts.max_len,
                counts.max_capture_capacity
            ),
            (32, 32, 256, 8, 128)
        );
        assert!(!counts.overflow);
        assert_eq!(owner(512).copy_counts_for_test(), CopyCounts::default());
        let before = scope.snapshot();
        scope.state().copy_counts = CopyCounts {
            created: u64::MAX,
            returned: u64::MAX,
            len_sum: u64::MAX,
            max_len: u64::MAX,
            max_capture_capacity: u64::MAX,
            armed: true,
            overflow: false,
        };
        drop(capture(&scope, b"x").finish(&scope).unwrap());
        drop(capture(&scope, b"").finish(&scope).unwrap());
        let saturated = scope.copy_counts_for_test();
        assert!(saturated.overflow);
        assert_eq!(
            (saturated.created, saturated.returned, saturated.len_sum),
            (u64::MAX, u64::MAX, u64::MAX)
        );
        assert_eq!(
            scope.snapshot().registry_admitted_bytes,
            before.registry_admitted_bytes + 1
        );
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
        assert_eq!(scope.first_error_for_test(), None);
        let mut bytes = [0u8; 512];
        let mut sink = io::Cursor::new(bytes.as_mut_slice());
        CopyCounts {
            armed: false,
            overflow: false,
            ..saturated
        }
        .write(&mut sink)
        .unwrap();
        let length = sink.position() as usize;
        assert!(length <= 512);
        assert!(std::mem::size_of::<CopyCounts>() <= 64);
        assert!(std::mem::size_of_val(&saturated) <= 64);
        let line = std::str::from_utf8(&bytes[..length]).unwrap();
        assert!(line.ends_with(
            "armed=false overflow=false traffic_only=true retained_bytes=unavailable\n"
        ));
        eprintln!(
            "WP1127_CAPTURE_COPY_LAYOUT scope={} counts={} snapshot={} counts_align={} snapshot_align={} max_report={}",
            std::mem::size_of::<SourceServiceScope>(),
            std::mem::size_of::<CopyCounts>(),
            std::mem::size_of_val(&saturated),
            std::mem::align_of::<CopyCounts>(),
            std::mem::align_of_val(&saturated),
            length
        );
    }
}
