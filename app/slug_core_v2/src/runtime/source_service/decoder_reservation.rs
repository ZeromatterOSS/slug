//! Independent scoped codec allocation reservations. No source authority or
//! graph computation is introduced by this bank.
use std::sync::Arc;

use super::SourceServiceError;
use super::SourceServiceScope;
use super::SourceTimeoutOrigin;

impl SourceServiceScope {
    pub(in crate::runtime) fn reserve_decoder(
        self: &Arc<Self>,
        bytes: usize,
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
        state.counters.live_decoder_bytes = next;
        state.counters.peak_decoder_bytes = state.counters.peak_decoder_bytes.max(next);
        Ok(SourceDecoderReservation {
            scope: Arc::clone(self),
            bytes,
        })
    }
}

/// A non-cloneable credit retires only when its sole decoder owner drops it.
pub(in crate::runtime) struct SourceDecoderReservation {
    scope: Arc<SourceServiceScope>,
    bytes: usize,
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
