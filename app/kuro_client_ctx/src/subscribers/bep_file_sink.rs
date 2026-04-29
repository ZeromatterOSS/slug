//! Event subscriber that writes Build Event Protocol (BEP) events to local
//! files. Wired by the `--build_event_binary_file` and
//! `--build_event_text_file` CLI flags; mirrors Bazel's own file-output
//! behaviour.
//!
//! JSON file output (`--build_event_json_file`) is not yet supported — it
//! requires proto3-canonical JSON encoding, tracked as follow-up in Plan 18.8.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use kuro_build_event_stream::build_event_stream as bep;
use kuro_build_event_stream::file_sink::Encoding;
use kuro_build_event_stream::file_sink::FileSink;
use kuro_build_event_stream::translate::BepStreamState;
use kuro_build_event_stream::translate::BuildEventContext;
use kuro_build_event_stream::translate::make_aborted;
use kuro_build_event_stream::translate::translate_buck_event;
use kuro_events::BuckEvent;

use crate::subscribers::subscriber::EventSubscriber;

/// One file output, paired with the encoding it writes.
struct Output {
    sink: FileSink,
}

/// Subscribes to Kuro's event stream and mirrors each event into one or more
/// BEP-format files.
pub struct BepFileSubscriber {
    outputs: Vec<Output>,
    ctx: BuildEventContext,
    stream_state: BepStreamState,
    saw_command_end: bool,
}

impl BepFileSubscriber {
    /// Creates a subscriber given the output paths from the CLI flags.
    ///
    /// If both paths are `None`, returns `Ok(None)` so callers can `?`-chain
    /// without building a no-op subscriber.
    pub fn maybe_new(
        binary_path: Option<PathBuf>,
        text_path: Option<PathBuf>,
        ctx: BuildEventContext,
    ) -> kuro_error::Result<Option<Self>> {
        if binary_path.is_none() && text_path.is_none() {
            return Ok(None);
        }

        let mut outputs = Vec::with_capacity(2);
        if let Some(path) = binary_path {
            let sink = FileSink::create(path, Encoding::Binary).map_err(|e| {
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "Failed to open --build_event_binary_file: {e}"
                )
            })?;
            outputs.push(Output { sink });
        }
        if let Some(path) = text_path {
            let sink = FileSink::create(path, Encoding::Text).map_err(|e| {
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "Failed to open --build_event_text_file: {e}"
                )
            })?;
            outputs.push(Output { sink });
        }

        Ok(Some(Self {
            outputs,
            stream_state: BepStreamState::with_invocation_id(ctx.invocation_id.clone()),
            ctx,
            saw_command_end: false,
        }))
    }

    fn write_event(&mut self, event: &kuro_data::BuckEvent) {
        // See bep_bes_sink: kuro-side stats (critical path,
        // BuildGraph node/edge counts, Command span timestamps) feed
        // BuildMetrics but don't translate to BEP.
        self.stream_state.observe_kuro_event(event);
        let bep_events = translate_buck_event(&self.ctx, event);
        for bep_event in bep_events {
            self.stream_state.observe(Some(event), &bep_event);
            self.write_bep(&bep_event);
        }
    }

    fn write_bep(&self, event: &bep::BuildEvent) {
        for output in &self.outputs {
            if let Err(e) = output.sink.write(event) {
                tracing::warn!(
                    "BEP file sink write failed for {}: {e}",
                    output.sink.path().display()
                );
            }
        }
    }
}

#[async_trait]
impl EventSubscriber for BepFileSubscriber {
    fn name(&self) -> &'static str {
        "bep_file_sink"
    }

    async fn handle_events(&mut self, events: &[Arc<BuckEvent>]) -> kuro_error::Result<()> {
        for event in events {
            if is_command_end(event.event()) {
                self.saw_command_end = true;
            }
            self.write_event(event.event());
        }
        Ok(())
    }

    async fn finalize(&mut self) -> kuro_error::Result<()> {
        if !self.saw_command_end {
            let aborted = make_aborted(
                bep::BuildEventId {
                    id: Some(bep::build_event_id::Id::BuildFinished(
                        bep::build_event_id::BuildFinishedId {},
                    )),
                },
                bep::aborted::AbortReason::UserInterrupted,
                "Build interrupted before CommandEnd was emitted",
            );
            self.write_bep(&aborted);
        }

        // Mirror what BesSubscriber emits as the final BEP events so the
        // file output matches what BuildBuddy receives over BES:
        // `BuildToolLogs` (chrome trace for the Timing tab) followed by
        // `BuildMetrics` (last=true).
        if let Some(tool_logs) = self.stream_state.build_tool_logs_event() {
            self.write_bep(&tool_logs);
        }
        self.write_bep(&self.stream_state.build_metrics_event());

        for output in &self.outputs {
            if let Err(e) = output.sink.flush() {
                tracing::warn!(
                    "BEP file sink flush failed for {}: {e}",
                    output.sink.path().display()
                );
            }
        }
        Ok(())
    }
}

fn is_command_end(event: &kuro_data::BuckEvent) -> bool {
    matches!(
        event.data.as_ref(),
        Some(kuro_data::buck_event::Data::SpanEnd(se))
            if matches!(
                se.data.as_ref(),
                Some(kuro_data::span_end_event::Data::Command(_))
            )
    )
}
