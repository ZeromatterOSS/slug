//! Immutable semantic-evaluation prints with deterministic, pre-copy admission.

use std::cell::RefCell;
use std::mem::size_of;
use std::sync::Arc;

use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_events_v2::StarlarkSourceLocation;
use starlark::PrintHandler;
use starlark::PrintLocation;

const RECORD_LIMIT: usize = 65_536;
const BYTE_LIMIT: usize = 64 * 1024 * 1024;
const LIMIT_ERROR: &str = "semantic .bzl evaluation event resource limit";

/// Input credits describe logical complete inputs, never their physical sharing.
pub(super) struct SemanticPrintCapture {
    input_records: usize,
    input_bytes: usize,
    direct_presentation: bool,
    state: RefCell<CaptureState>,
}

struct CaptureState {
    events: Vec<EvaluationEvent>,
    payload_bytes: usize,
    capacity_credit: usize,
    poisoned: bool,
}

fn add(a: usize, b: usize) -> Result<usize, String> {
    a.checked_add(b).ok_or_else(|| LIMIT_ERROR.to_owned())
}

fn mul(a: usize, b: usize) -> Result<usize, String> {
    a.checked_mul(b).ok_or_else(|| LIMIT_ERROR.to_owned())
}

impl SemanticPrintCapture {
    pub(super) fn new(
        input_records: usize,
        input_bytes: usize,
        direct_presentation: bool,
    ) -> Result<Self, String> {
        let capture = Self {
            input_records,
            input_bytes,
            direct_presentation,
            state: RefCell::new(CaptureState {
                events: Vec::new(),
                payload_bytes: 0,
                capacity_credit: 0,
                poisoned: false,
            }),
        };
        capture.forecast(0, 0, 0, 0)?;
        Ok(capture)
    }

    // Vec growth and final Vec -> collected Vec -> Arc are conservatively
    // forecast together. This is a resource preflight, not an allocator fence.
    // Capacity credits follow our own schedule, not allocator/source Arc state.
    fn forecast(
        &self,
        len: usize,
        old_capacity: usize,
        capacity: usize,
        payload_bytes: usize,
    ) -> Result<(), String> {
        if add(self.input_records, len)? > RECORD_LIMIT {
            return Err(LIMIT_ERROR.to_owned());
        }
        let growth_slots = add(old_capacity, capacity)?;
        let transfer_slots = add(capacity, mul(len, 2)?)?;
        let slots = growth_slots.max(transfer_slots);
        let fixed = add(
            size_of::<Self>(),
            add(size_of::<EventBatch>(), 4 * size_of::<usize>())?,
        )?;
        let bytes = add(
            add(self.input_bytes, fixed)?,
            add(payload_bytes, mul(slots, size_of::<EvaluationEvent>())?)?,
        )?;
        if bytes > BYTE_LIMIT {
            return Err(LIMIT_ERROR.to_owned());
        }
        Ok(())
    }

    fn capture(&self, file: Arc<str>, line: u32, column: u32, text: &str) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        if state.poisoned {
            return Err(LIMIT_ERROR.to_owned());
        }
        let admitted = (|| {
            let len = add(state.events.len(), 1)?;
            let capacity = if len > state.capacity_credit {
                mul(state.capacity_credit.max(2), 2)?.min(RECORD_LIMIT)
            } else {
                state.capacity_credit
            };
            // Filename ownership can be shared, but every logical event pays
            // its full filename. CompactString's conservative copy credit is
            // independent of its inline/allocation representation.
            let payload = add(
                state.payload_bytes,
                add(
                    add(file.len(), 2 * size_of::<usize>())?,
                    add(mul(text.len(), 2)?, 1)?,
                )?,
            )?;
            self.forecast(len, state.capacity_credit, capacity, payload)?;
            Ok((capacity, payload))
        })();
        let (capacity, payload) = match admitted {
            Ok(admitted) => admitted,
            Err(error) => {
                state.poisoned = true;
                return Err(error);
            }
        };
        if capacity > state.capacity_credit {
            let additional = capacity.saturating_sub(state.events.len());
            state.events.reserve_exact(additional);
        }
        state.events.push(EvaluationEvent::StarlarkPrint {
            location: StarlarkSourceLocation::new(file, line, column),
            text: text.into(),
        });
        state.capacity_credit = capacity;
        state.payload_bytes = payload;
        Ok(())
    }

    pub(super) fn finish(self) -> Result<Arc<EventBatch>, String> {
        let state = self.state.into_inner();
        if state.poisoned {
            return Err(LIMIT_ERROR.to_owned());
        }
        // Each successful append already admitted this final transfer peak.
        Ok(Arc::new(EventBatch::from_events(state.events)))
    }
}

impl PrintHandler for SemanticPrintCapture {
    fn println(&self, location: PrintLocation, text: &str) -> starlark::Result<()> {
        let presentation = self.direct_presentation.then(|| location.clone());
        let (file, line, column) = location.into_parts();
        self.capture(file, line, column, text)
            .map_err(|error| starlark::Error::new_other(anyhow::anyhow!(error)))?;
        // Match the Engine's default StderrPrintHandler; this presentation is
        // operational and does not influence the immutable captured batch.
        if let Some(location) = presentation {
            eprintln!("{location}: {text}");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
