/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License found in the LICENSE-APACHE file in the root directory of this
 * source tree. You may select the license that applies to you.
 */

use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;

/// Request-local capability selecting capture instead of direct event output.
///
/// Its presence in DICE per-transaction data is operational only. It is not a
/// semantic input and does not participate in key equality.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Allocative, Dupe)]
pub struct CaptureEvaluationEvents;

/// One evaluation-local event captured for later command-owned publication.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum EvaluationEvent {
    StarlarkPrint { text: CompactString },
}

/// An immutable ordered batch of evaluation events.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct EventBatch {
    events: Arc<[EvaluationEvent]>,
}

impl EventBatch {
    pub fn empty() -> Self {
        Self {
            events: Arc::from([]),
        }
    }

    pub fn from_events(events: impl IntoIterator<Item = EvaluationEvent>) -> Self {
        Self {
            events: events.into_iter().collect::<Vec<_>>().into(),
        }
    }

    pub fn events(&self) -> &[EvaluationEvent] {
        &self.events
    }
}

impl FromIterator<EvaluationEvent> for EventBatch {
    fn from_iter<T: IntoIterator<Item = EvaluationEvent>>(iter: T) -> Self {
        Self::from_events(iter)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use compact_str::CompactString;
    use dupe::Dupe;

    use super::EvaluationEvent;
    use super::EventBatch;

    fn print(text: &str) -> EvaluationEvent {
        EvaluationEvent::StarlarkPrint {
            text: CompactString::new(text),
        }
    }

    #[test]
    fn event_batch_empty_is_explicit() {
        assert!(EventBatch::empty().events().is_empty());
    }

    #[test]
    fn event_batch_preserves_singleton_and_multiple_order() {
        assert_eq!(
            EventBatch::from_events([print("one")]).events(),
            &[print("one")]
        );

        let batch = [print("first"), print("second"), print("third")]
            .into_iter()
            .collect::<EventBatch>();
        assert_eq!(
            batch.events(),
            &[print("first"), print("second"), print("third")]
        );
    }

    #[test]
    fn event_batch_equality_is_structural_and_order_sensitive() {
        let first = EventBatch::from_events([print("one"), print("two")]);
        let same = EventBatch::from_events([print("one"), print("two")]);
        let reversed = EventBatch::from_events([print("two"), print("one")]);
        assert_eq!(first, same);
        assert_ne!(first, reversed);
        assert_ne!(first, EventBatch::from_events([print("one")]));
    }

    #[test]
    fn event_batch_dupe_shares_storage_and_retains_exact_utf8() {
        let text = "π你好🙂\nexact";
        let batch = EventBatch::from_events([print(text)]);
        let cloned = batch.dupe();
        assert!(Arc::ptr_eq(&batch.events, &cloned.events));
        assert_eq!(
            cloned.events(),
            &[EvaluationEvent::StarlarkPrint {
                text: CompactString::new(text),
            }]
        );
    }
}
