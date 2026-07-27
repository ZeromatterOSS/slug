/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License found in the LICENSE-APACHE file in the root directory of this
 * source tree. You may select the license that applies to you.
 */

use std::fmt;
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

/// Severity of a neutral evaluation diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub enum EvaluationDiagnosticLevel {
    Warning,
    Error,
}

/// Owned Bazel-shaped location of one Starlark print call.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct StarlarkSourceLocation {
    file: Arc<str>,
    line: u32,
    column: u32,
}

impl StarlarkSourceLocation {
    pub fn new(file: Arc<str>, line: u32, column: u32) -> Self {
        assert!(
            line != 0 || column == 0,
            "a source column requires a source line"
        );
        Self { file, line, column }
    }
}

impl fmt::Display for StarlarkSourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.file)?;
        if self.line != 0 {
            write!(f, ":{}", self.line)?;
            if self.column != 0 {
                write!(f, ":{}", self.column)?;
            }
        }
        Ok(())
    }
}

/// One evaluation-local event captured for later command-owned publication.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum EvaluationEvent {
    StarlarkPrint {
        location: StarlarkSourceLocation,
        text: CompactString,
    },
    Diagnostic {
        level: EvaluationDiagnosticLevel,
        text: CompactString,
    },
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

    use super::EvaluationDiagnosticLevel;
    use super::EvaluationEvent;
    use super::EventBatch;
    use super::StarlarkSourceLocation;

    fn print(text: &str) -> EvaluationEvent {
        EvaluationEvent::StarlarkPrint {
            location: StarlarkSourceLocation::new(Arc::from("test.bzl"), 1, 6),
            text: CompactString::new(text),
        }
    }

    fn diagnostic(level: EvaluationDiagnosticLevel, text: &str) -> EvaluationEvent {
        EvaluationEvent::Diagnostic {
            level,
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
    fn diagnostic_levels_are_structurally_unequal() {
        assert_ne!(
            diagnostic(EvaluationDiagnosticLevel::Warning, "same"),
            diagnostic(EvaluationDiagnosticLevel::Error, "same")
        );
    }

    #[test]
    fn diagnostic_retains_exact_utf8_and_newline_text() {
        let text = "π你好🙂\nfirst\r\nsecond\n";
        assert_eq!(
            diagnostic(EvaluationDiagnosticLevel::Warning, text),
            EvaluationEvent::Diagnostic {
                level: EvaluationDiagnosticLevel::Warning,
                text: CompactString::new(text),
            }
        );
    }

    #[test]
    fn event_batch_preserves_mixed_print_and_diagnostic_order() {
        let events = [
            print("before"),
            diagnostic(EvaluationDiagnosticLevel::Warning, "warning"),
            print("between"),
            diagnostic(EvaluationDiagnosticLevel::Error, "error"),
        ];
        let batch = EventBatch::from_events(events.clone());
        assert_eq!(batch.events(), &events);
        assert_ne!(
            batch,
            EventBatch::from_events([
                diagnostic(EvaluationDiagnosticLevel::Warning, "warning"),
                print("before"),
                print("between"),
                diagnostic(EvaluationDiagnosticLevel::Error, "error"),
            ])
        );
    }

    #[test]
    fn event_batch_dupe_shares_mixed_storage_and_retains_exact_utf8() {
        let text = "π你好🙂\nexact";
        let batch = EventBatch::from_events([
            print("before"),
            diagnostic(EvaluationDiagnosticLevel::Warning, text),
        ]);
        let cloned = batch.dupe();
        assert!(Arc::ptr_eq(&batch.events, &cloned.events));
        assert_eq!(
            cloned.events(),
            &[
                EvaluationEvent::StarlarkPrint {
                    location: StarlarkSourceLocation::new(Arc::from("test.bzl"), 1, 6),
                    text: CompactString::new("before"),
                },
                EvaluationEvent::Diagnostic {
                    level: EvaluationDiagnosticLevel::Warning,
                    text: CompactString::new(text),
                },
            ]
        );
    }

    #[test]
    fn starlark_location_is_structural_and_shares_filename_storage() {
        let file: Arc<str> = Arc::from("/long/apparent/workspace/path/defs.bzl");
        let first = StarlarkSourceLocation::new(file.clone(), 3, 14);
        let second = StarlarkSourceLocation::new(file.clone(), 3, 14);
        let different = StarlarkSourceLocation::new(file.clone(), 3, 15);
        assert_eq!(first, second);
        assert_ne!(first, different);
        assert_eq!(
            first.to_string(),
            "/long/apparent/workspace/path/defs.bzl:3:14"
        );
        assert!(Arc::ptr_eq(&first.file, &second.file));
    }

    #[test]
    fn starlark_builtin_location_omits_zero_line_and_column() {
        assert_eq!(
            StarlarkSourceLocation::new(Arc::from("<builtin>"), 0, 0).to_string(),
            "<builtin>"
        );
    }
}
