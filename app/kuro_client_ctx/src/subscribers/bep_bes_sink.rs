//! Event subscriber that uploads Build Event Protocol (BEP) events to a
//! Build Event Service (BES) backend via gRPC. Backs the `--bes_backend`,
//! `--bes_results_url`, `--bes_header`, `--bes_keywords`, `--bes_timeout`,
//! `--bes_upload_mode`, and `--bes_instance_name` CLI flags.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use humantime::parse_duration;
use kuro_build_event_stream::build_event_stream as bep;
use kuro_build_event_stream::grpc_sink::BesConfig;
use kuro_build_event_stream::grpc_sink::BesSink;
use kuro_build_event_stream::grpc_sink::UploadMode;
use kuro_build_event_stream::translate::BepStreamState;
use kuro_build_event_stream::translate::BuildEventContext;
use kuro_build_event_stream::translate::make_aborted;
use kuro_build_event_stream::translate::make_progress;
use kuro_build_event_stream::translate::translate_buck_event;
use kuro_events::BuckEvent;

use crate::common::CommonBuildConfigurationOptions;
use crate::subscribers::subscriber::EventSubscriber;

/// BES subscriber. Connects lazily on the first event so we can stay out of
/// the synchronous `update_events_ctx` setup path; if the connection fails,
/// the subscriber logs once and subsequent events become no-ops (BES upload
/// failure never fails the build).
pub struct BesSubscriber {
    state: State,
    ctx: BuildEventContext,
    stream_state: BepStreamState,
    saw_command_end: bool,
    progress_seq: i32,
}

enum State {
    Pending(BesConfig),
    Connected(Arc<BesSink>),
    Failed,
}

impl BesSubscriber {
    /// Construct the subscriber from CLI flags without opening the
    /// connection. Returns `None` when `--bes_backend` is not set.
    pub fn maybe_new(
        opts: &CommonBuildConfigurationOptions,
        ctx: BuildEventContext,
    ) -> kuro_error::Result<Option<Self>> {
        let Some(backend) = opts.bes_backend.as_ref() else {
            return Ok(None);
        };

        let timeout = match opts.bes_timeout.as_deref() {
            Some(s) => parse_duration(s).map_err(|e| {
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "Invalid --bes_timeout `{s}`: {e}"
                )
            })?,
            None => Duration::from_secs(60),
        };
        let upload_mode = match opts.bes_upload_mode.as_deref() {
            Some("nowait") => UploadMode::NoWait,
            Some("fully_async") => UploadMode::FullyAsync,
            Some("wait_for_upload_complete") | None => UploadMode::WaitForUploadComplete,
            Some(other) => {
                return Err(kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "Invalid --bes_upload_mode `{other}`"
                ));
            }
        };

        // Bazel routes `--remote_header=KEY=VALUE` (set in `common --…`
        // lines of `.bazelrc`) to both the RE client *and* the BES
        // upload — that's how a single `x-buildbuddy-api-key=…`
        // declaration covers everything BuildBuddy needs. Mirror that
        // here so a vanilla `kuro build … --config=remote` running in a
        // workspace with a Bazel `.bazelrc` authenticates without the
        // user having to set `--bes_header` separately. `--bes_header`
        // (when explicitly passed) takes precedence by appearing
        // second.
        let headers = opts
            .remote_header
            .iter()
            .chain(opts.bes_header.iter())
            .map(|entry| parse_kv(entry))
            .collect::<kuro_error::Result<Vec<_>>>()?;

        let config = BesConfig {
            backend: backend.clone(),
            invocation_id: ctx.invocation_id.clone(),
            // BES distinguishes the *build* (potentially multi-attempt
            // request) from each *invocation* attempt within it. Bazel
            // generates two independent UUIDs (`buildRequestId` and
            // `commandId`); BuildBuddy keys its CONTROLLER vs TOOL stream
            // routing tables off them being distinct, and silently drops
            // the invocation when they collide. Mint a fresh UUID here
            // until Kuro grows a real notion of retry-within-a-build.
            build_id: uuid::Uuid::new_v4().to_string(),
            // Bazel sends `project_id` only when `--bes_instance_name` is
            // explicitly set; an unset project_id is empty string.
            // BuildBuddy treats a non-empty arbitrary value (`"default"`)
            // as a routing hint to a specific project that may not exist,
            // and silently drops the invocation. Match Bazel's behavior.
            project_id: opts.bes_instance_name.clone().unwrap_or_default(),
            keywords: opts.bes_keywords.clone(),
            headers,
            timeout,
            upload_mode,
        };

        Ok(Some(Self {
            state: State::Pending(config),
            stream_state: BepStreamState::with_invocation_id(ctx.invocation_id.clone()),
            ctx,
            saw_command_end: false,
            progress_seq: 0,
        }))
    }

    async fn ensure_connected(&mut self) -> Option<Arc<BesSink>> {
        match &self.state {
            State::Connected(sink) => return Some(sink.clone()),
            State::Failed => return None,
            State::Pending(_) => {}
        }
        let State::Pending(config) = std::mem::replace(&mut self.state, State::Failed) else {
            unreachable!();
        };
        match BesSink::start(config).await {
            Ok(sink) => {
                let sink = Arc::new(sink);
                self.state = State::Connected(sink.clone());
                Some(sink)
            }
            Err(e) => {
                tracing::warn!("BES sink connect failed: {e}. Upload disabled for this build.");
                None
            }
        }
    }

    /// Emit the BuildBuddy-style `Streaming build results to:` line that CI
    /// log scrapers look for, if `--bes_results_url` is set. Written to
    /// stderr so it is visible without a specific `RUST_LOG` setting.
    pub fn log_results_url(results_url_prefix: Option<&str>, invocation_id: &str) {
        if let Some(prefix) = results_url_prefix {
            eprintln!(
                "Streaming build results to: {}/{}",
                prefix.trim_end_matches('/'),
                invocation_id
            );
        }
    }
}

#[async_trait]
impl EventSubscriber for BesSubscriber {
    fn name(&self) -> &'static str {
        "bep_bes_sink"
    }

    async fn handle_output(&mut self, raw_output: &[u8]) -> kuro_error::Result<()> {
        if raw_output.is_empty() {
            return Ok(());
        }
        let Some(sink) = self.ensure_connected().await else {
            return Ok(());
        };
        self.progress_seq += 1;
        let stdout = String::from_utf8_lossy(raw_output).into_owned();
        let progress = make_progress(self.progress_seq, stdout, String::new(), Vec::new());
        let _ = sink.enqueue(progress).await;
        Ok(())
    }

    async fn handle_tailer_stderr(&mut self, stderr: &str) -> kuro_error::Result<()> {
        if stderr.is_empty() {
            return Ok(());
        }
        let Some(sink) = self.ensure_connected().await else {
            return Ok(());
        };
        self.progress_seq += 1;
        let progress = make_progress(
            self.progress_seq,
            String::new(),
            stderr.to_owned(),
            Vec::new(),
        );
        let _ = sink.enqueue(progress).await;
        Ok(())
    }

    async fn handle_events(&mut self, events: &[Arc<BuckEvent>]) -> kuro_error::Result<()> {
        let Some(sink) = self.ensure_connected().await else {
            return Ok(());
        };
        for event in events {
            if is_command_end(event.event()) {
                self.saw_command_end = true;
            }
            let buck_event = event.event();
            // Pull kuro-side data (BuildGraphInfo critical-path stats,
            // Command span timestamps) before translating — those
            // signals don't have BEP analogues but feed BuildMetrics.
            self.stream_state.observe_kuro_event(buck_event);
            let bep_events = translate_buck_event(&self.ctx, buck_event);
            for bep_event in bep_events {
                self.stream_state.observe(Some(buck_event), &bep_event);
                if let Err(e) = sink.enqueue(bep_event).await {
                    tracing::warn!("BES sink enqueue failed: {e}");
                    self.state = State::Failed;
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    async fn finalize(&mut self) -> kuro_error::Result<()> {
        // No CommandEnd observed → invocation ended prematurely (Ctrl+C,
        // daemon crash, etc.). Synthesize an `Aborted` event so BuildBuddy
        // moves the invocation out of "still running" state.
        if !self.saw_command_end
            && let State::Connected(sink) = &self.state
        {
            let aborted = make_aborted(
                bep::BuildEventId {
                    id: Some(bep::build_event_id::Id::BuildFinished(
                        bep::build_event_id::BuildFinishedId {},
                    )),
                },
                bep::aborted::AbortReason::UserInterrupted,
                "Build interrupted before CommandEnd was emitted",
            );
            let _ = sink.enqueue(aborted).await;
        }

        // Bazel order: `BuildFinished` → `BuildToolLogs` → `BuildMetrics
        // last=true`. The Timing tab in BuildBuddy reads
        // `command.profile.gz` from BuildToolLogs; the entry must be a
        // `bytestream://` URI (BB ignores inline `contents` for this
        // file specifically). Upload the gzipped trace bytes via the
        // BES backend's ByteStream service, then attach the resulting
        // URI to the BuildToolLogs event.
        if let State::Connected(sink) = &self.state {
            if let Some(gz_bytes) = self.stream_state.build_profile_gz() {
                let gz_size = gz_bytes.len();
                tracing::debug!("BES finalize: uploading chrome trace ({gz_size} gz bytes)");
                match sink.upload_blob_bytestream(gz_bytes).await {
                    None => {
                        tracing::warn!(
                            "BES finalize: chrome trace bytestream upload failed; \
                             BB Timing tab will stay blank"
                        );
                    }
                    Some(uri) => {
                        tracing::debug!("BES finalize: trace uploaded; URI = {uri}");
                        if let Some(tool_logs) =
                            self.stream_state.build_tool_logs_event_with_uri(uri)
                            && let Err(e) = sink.enqueue(tool_logs).await
                        {
                            tracing::warn!("BES finalize: enqueue BuildToolLogs failed: {e}");
                        }
                    }
                }
            }
            let metrics = self.stream_state.build_metrics_event();
            tracing::info!("BES finalize: enqueueing BuildMetrics");
            if let Err(e) = sink.enqueue(metrics).await {
                tracing::warn!("BES finalize: enqueue BuildMetrics failed: {e}");
            }
        }

        if let State::Connected(sink) = &self.state
            && let Err(e) = sink.shutdown().await
        {
            tracing::warn!("BES sink shutdown error: {e}");
        }
        Ok(())
    }
}

/// Classify a raw `BuckEvent` as the terminal `CommandEnd` span-end that
/// marks normal invocation completion.
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

fn parse_kv(entry: &str) -> kuro_error::Result<(String, String)> {
    let (key, value) = entry.split_once('=').ok_or_else(|| {
        kuro_error::kuro_error!(
            kuro_error::ErrorTag::Input,
            "Expected --bes_header=KEY=VALUE, got `{entry}`"
        )
    })?;
    Ok((key.to_owned(), value.to_owned()))
}
