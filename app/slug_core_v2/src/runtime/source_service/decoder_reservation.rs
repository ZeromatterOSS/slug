//! Independent scoped codec allocation reservations. No source authority or
//! graph computation is introduced by this bank.
use std::sync::Arc;

use super::SourceServiceError;
use super::SourceServiceScope;
use super::SourceTimeoutOrigin;

#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
#[path = "xz_allocator.rs"]
mod xz_allocator;
#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
#[path = "xz_decoder.rs"]
mod xz_decoder;
#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
pub(in crate::runtime) use xz_decoder::SourceXzDecoder;

impl SourceServiceScope {
    pub(in crate::runtime) fn reserve_decoder(
        self: &Arc<Self>,
        bytes: usize,
    ) -> Result<SourceDecoderReservation, SourceServiceError> {
        self.reserve_decoder_inner(bytes, true)
    }

    // Only the closed XZ owner may replace the native allowance debit with
    // individually prepaid callback allocations. This is not a caller mode.
    #[cfg(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ))]
    fn reserve_xz_decoder_allowance(
        self: &Arc<Self>,
        bytes: usize,
    ) -> Result<SourceDecoderReservation, SourceServiceError> {
        self.reserve_decoder_inner(bytes, false)
    }

    fn reserve_decoder_inner(
        self: &Arc<Self>,
        bytes: usize,
        _native_payload: bool,
    ) -> Result<SourceDecoderReservation, SourceServiceError> {
        let mut state = self.state();
        self.check_locked(&mut state, false)?;
        let next = state.counters.live_decoder_bytes.checked_add(bytes);
        let Some(next) = next.filter(|next| *next <= self.limits.live_decoder_bytes) else {
            let first = Self::poison_locked(
                &mut state,
                SourceServiceError::DecoderLimit,
                SourceTimeoutOrigin::unattributed(),
            );
            drop(state);
            self.cancelled.send_replace(true);
            return Err(first);
        };
        #[cfg(all(
            target_os = "linux",
            target_arch = "x86_64",
            target_pointer_width = "64"
        ))]
        let command_credit = match (if _native_payload { bytes } else { 0 })
            .checked_add(std::mem::size_of::<SourceDecoderReservation>())
            .ok_or(SourceServiceError::Allocation)
            .and_then(|bytes| self.reserve_command_bytes(bytes))
        {
            Ok(credit) => credit,
            Err(error) => {
                let first =
                    Self::poison_locked(&mut state, error, SourceTimeoutOrigin::unattributed());
                drop(state);
                self.cancelled.send_replace(true);
                return Err(first);
            }
        };
        state.counters.live_decoder_bytes = next;
        state.counters.peak_decoder_bytes = state.counters.peak_decoder_bytes.max(next);
        Ok(SourceDecoderReservation {
            scope: Arc::clone(self),
            bytes,
            #[cfg(all(
                target_os = "linux",
                target_arch = "x86_64",
                target_pointer_width = "64"
            ))]
            _command_credit: command_credit,
        })
    }
}

/// A non-cloneable credit retires only when its sole decoder owner drops it.
pub(in crate::runtime) struct SourceDecoderReservation {
    scope: Arc<SourceServiceScope>,
    bytes: usize,
    #[cfg(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ))]
    _command_credit: Option<crate::runtime::repository_physical_root::ResourceCredit>,
}

impl Drop for SourceDecoderReservation {
    fn drop(&mut self) {
        let mut state = self.scope.state();
        state.counters.live_decoder_bytes -= self.bytes;
    }
}

#[cfg(test)]
#[path = "decoder_reservation_tests.rs"]
mod tests;
