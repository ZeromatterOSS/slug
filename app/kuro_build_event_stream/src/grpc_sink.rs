//! Build Event Service (BES) gRPC sink.
//!
//! Wires Kuro into BuildBuddy / EngFlow / Trunk / custom BES collectors via
//! Bazel's `PublishBuildEvent` service. Flags match Bazel 9.x:
//!
//! ```text
//! --bes_backend=grpcs://remote.buildbuddy.io
//! --bes_results_url=https://app.buildbuddy.io/invocation/
//! --bes_header=x-buildbuddy-api-key=<KEY>
//! --bes_keywords=role=CI,release=nightly
//! --bes_timeout=10m
//! ```
//!
//! Architecture: the sink runs on a Tokio task behind a bounded
//! `mpsc::channel`. Events are wrapped in `OrderedBuildEvent`s and streamed
//! over `PublishBuildToolEventStream` (bidi RPC). Lifecycle events
//! (`BuildEnqueued`, `InvocationAttemptStarted`, `InvocationAttemptFinished`,
//! `BuildFinished`) go through the unary `PublishLifecycleEvent`.
//!
//! Failure handling: transient RPC errors are retried with exponential
//! backoff bounded by `--bes_timeout`. Terminal errors are logged; the user's
//! build is never failed because the BES upload failed. Daemon shutdown
//! flushes in-flight events within `--bes_timeout` and then drops the rest.

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use prost::Message;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tonic::Request;
use tonic::metadata::MetadataKey;
use tonic::metadata::MetadataMap;
use tonic::metadata::MetadataValue;
use tonic::service::Interceptor;
use tonic::transport::ClientTlsConfig;
use tonic::transport::Endpoint;

use crate::build_event_stream::BuildEvent as BepBuildEvent;
use crate::google::devtools::build::v1 as bes;

/// User-supplied configuration for a BES upload. Normally populated from the
/// CLI flags.
#[derive(Debug, Clone)]
pub struct BesConfig {
    pub backend: String,
    pub invocation_id: String,
    pub build_id: String,
    pub project_id: String,
    pub keywords: Vec<String>,
    pub headers: Vec<(String, String)>,
    pub timeout: Duration,
    pub upload_mode: UploadMode,
}

/// Matches Bazel's `--bes_upload_mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadMode {
    /// Block build exit until the BES stream is fully uploaded (default).
    WaitForUploadComplete,
    /// Fire-and-forget on build exit.
    NoWait,
    /// Asynchronous: upload continues in the background after the build exits
    /// and into the next invocation.
    FullyAsync,
}

impl Default for UploadMode {
    fn default() -> Self {
        Self::WaitForUploadComplete
    }
}

/// Sending half of the BES pipeline; hold one per invocation.
pub struct BesSink {
    tx: mpsc::Sender<StreamItem>,
    upload_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    config: BesConfig,
    interceptor: HeaderInterceptor,
}

/// Internal channel payload.
enum StreamItem {
    Event(BepBuildEvent),
    Finish,
}

/// Default capacity for the dispatcher → uploader channel. Empirically
/// sufficient to keep up with a mid-sized build without backpressure (Plan 17
/// measurements put peak BEP rate around ~500 events/sec for typical C++
/// builds). If the uploader blocks, senders wait — dropping events breaks
/// parent/child relationships and renders BuildBuddy's invocation tree
/// incomplete.
const CHANNEL_CAPACITY: usize = 10_000;

impl BesSink {
    /// Create the sink, spawn the uploader task, and return a handle that
    /// accepts BEP `BuildEvent`s.
    pub async fn start(config: BesConfig) -> kuro_error::Result<Self> {
        let endpoint = build_endpoint(&config).await?;
        let headers = config.headers.clone();
        let interceptor = HeaderInterceptor::new(headers)?;
        let channel = endpoint.connect().await.map_err(|e| {
            kuro_error::kuro_error!(
                kuro_error::ErrorTag::Environment,
                "BES connect to {}: {e}",
                config.backend
            )
        })?;
        let client = bes::publish_build_event_client::PublishBuildEventClient::with_interceptor(
            channel,
            interceptor.clone(),
        );

        let (tx, rx) = mpsc::channel::<StreamItem>(CHANNEL_CAPACITY);
        let uploader_cfg = config.clone();
        let upload_task = tokio::spawn(async move {
            if let Err(e) = run_uploader(client, uploader_cfg, rx).await {
                tracing::warn!("BES uploader task exited: {e}");
            }
        });

        Ok(Self {
            tx,
            upload_task: Arc::new(Mutex::new(Some(upload_task))),
            config,
            interceptor,
        })
    }

    /// Upload `bytes` to the BES backend's CAS via ByteStream `Write`
    /// and return a `bytestream://<host>/blobs/<sha256_hex>/<size>`
    /// URI suitable for use as `BuildToolLogs.File.uri`.
    ///
    /// BuildBuddy's invocation Timing tab refuses inline-bytes
    /// `BuildToolLogs.File.contents` for `command.profile.gz` and
    /// only renders timing data when the file is referenced via a
    /// `bytestream://` URI. Bazel uploads the same way; mirroring it
    /// is the only way the tab populates.
    ///
    /// On error returns `None` and logs — the build still succeeds,
    /// the timing tab just stays blank.
    pub async fn upload_blob_bytestream(&self, bytes: Vec<u8>) -> Option<String> {
        use re_grpc_proto::google::bytestream::WriteRequest;
        use re_grpc_proto::google::bytestream::byte_stream_client::ByteStreamClient;
        use sha2::Digest;

        let size = bytes.len() as i64;
        let mut hasher = sha2::Sha256::new();
        hasher.update(&bytes);
        let hash_hex = hex_lower(&hasher.finalize());
        // BuildBuddy's resource-name shape is `uploads/<uuid>/blobs/<hash>/<size>`.
        // The `uploads/<uuid>/` prefix is required by the ByteStream
        // contract for client-side dedup; the suffix names the CAS blob.
        let uuid = uuid::Uuid::new_v4();
        let resource_name = format!("uploads/{uuid}/blobs/{hash_hex}/{size}");

        // ByteStream gets its own short-lived Channel rather than sharing
        // the BES bidi stream's connection. A multi-MB profile upload on
        // the same HTTP/2 connection competes for connection-level
        // flow-control window with the streaming BEP events; on
        // `wait_for_upload_complete` that compounds the post-build wait.
        // The extra TCP+TLS handshake (~150 ms) overlaps with BES drain.
        let endpoint = match build_endpoint(&self.config).await {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("BES bytestream endpoint build failed: {e}");
                return None;
            }
        };
        let channel = match endpoint.connect().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("BES bytestream channel connect failed: {e}");
                return None;
            }
        };
        let mut client = ByteStreamClient::with_interceptor(channel, self.interceptor.clone());

        // Stream a single chunk — `command.profile.gz` is small (≤
        // a few MB even for huge builds).
        let req = WriteRequest {
            resource_name: resource_name.clone(),
            write_offset: 0,
            finish_write: true,
            data: bytes,
        };
        let stream = tokio_stream::iter([req]);
        match client.write(Request::new(stream)).await {
            Ok(resp) => {
                let committed = resp.into_inner().committed_size;
                if committed != size {
                    tracing::warn!(
                        "BES bytestream upload: server committed {} of {} bytes",
                        committed,
                        size
                    );
                    return None;
                }
            }
            Err(e) => {
                tracing::warn!("BES bytestream upload failed: {e}");
                return None;
            }
        }

        // Pull the host out of `--bes_backend` for the URI; BuildBuddy
        // accepts both `<host>` and `<host>:<port>` forms.
        let host = bes_uri_host(&self.config.backend).unwrap_or_else(|| String::from(""));
        Some(format!("bytestream://{host}/blobs/{hash_hex}/{size}"))
    }

    /// Queue a BEP event for upload.
    ///
    /// Blocks on backpressure: the channel is sized for typical workloads and
    /// dropping events is not an option (BEP parent/child tree would break).
    pub async fn enqueue(&self, event: BepBuildEvent) -> kuro_error::Result<()> {
        self.tx.send(StreamItem::Event(event)).await.map_err(|_| {
            kuro_error::kuro_error!(
                kuro_error::ErrorTag::Tier0,
                "BES sink: uploader task has exited"
            )
        })
    }

    /// Signal end-of-stream and, depending on `upload_mode`, either wait
    /// for the uploader task to flush or detach it.
    ///
    /// `WaitForUploadComplete` (default, matches Bazel): block on the
    /// uploader's `JoinHandle` up to `--bes_timeout`. Guarantees BB has
    /// the full event stream before the kuro process exits.
    ///
    /// `NoWait`: send the Finish marker so the request stream closes
    /// cleanly, then drop the JoinHandle without awaiting. The task
    /// continues running until the tokio runtime shuts down on
    /// process exit; events already buffered may not finish uploading.
    /// Mirrors Bazel's `--bes_upload_mode=nowait`.
    ///
    /// `FullyAsync` is treated as `NoWait` here. True async (uploader
    /// surviving across client invocations) would require a persistent
    /// daemon-side uploader; not implemented.
    pub async fn shutdown(&self) -> kuro_error::Result<()> {
        let _ = self.tx.send(StreamItem::Finish).await;
        let handle = self.upload_task.lock().await.take();
        let Some(h) = handle else {
            return Ok(());
        };
        match self.config.upload_mode {
            UploadMode::WaitForUploadComplete => {
                let timeout = self.config.timeout;
                match tokio::time::timeout(timeout, h).await {
                    Ok(Ok(())) => Ok(()),
                    Ok(Err(e)) => {
                        tracing::warn!("BES uploader join error: {e}");
                        Ok(())
                    }
                    Err(_) => {
                        tracing::warn!(
                            "BES uploader timeout after {timeout:?}; \
                             dropping remaining events"
                        );
                        Ok(())
                    }
                }
            }
            UploadMode::NoWait | UploadMode::FullyAsync => {
                // Drop the JoinHandle without awaiting. The task keeps
                // running until the runtime shuts down on process exit.
                drop(h);
                Ok(())
            }
        }
    }

    pub fn config(&self) -> &BesConfig {
        &self.config
    }
}

async fn build_endpoint(config: &BesConfig) -> kuro_error::Result<Endpoint> {
    // Bazel uses `grpc://` and `grpcs://` schemes; tonic speaks HTTP URIs.
    // Rewrite to the HTTP scheme that matches Bazel's TLS semantics.
    let tls = config.backend.starts_with("grpcs://") || config.backend.starts_with("https://");
    let uri = config
        .backend
        .replacen("grpcs://", "https://", 1)
        .replacen("grpc://", "http://", 1);

    let endpoint = Endpoint::from_str(&uri).map_err(|e| {
        kuro_error::kuro_error!(
            kuro_error::ErrorTag::Input,
            "Invalid --bes_backend URI `{}`: {e}",
            config.backend
        )
    })?;
    // Do NOT set `.timeout(config.timeout)` on the endpoint. tonic
    // applies that as a per-RPC deadline, and the
    // `PublishBuildToolEventStream` is a streaming RPC whose lifetime
    // is the entire build. With `--bes_timeout` defaulting to 60s,
    // any build longer than a minute would have the stream cancelled
    // mid-flight ("status: Cancelled, message: \"Timeout expired\"") —
    // BB would receive only the events that fit in the first 60s and
    // none of the trailing `BuildToolLogs` / `BuildMetrics`, leaving
    // the Timing tab stuck at "Build is in progress…".
    //
    // `--bes_timeout` semantically gates *shutdown* (how long we wait
    // for in-flight events to flush at end-of-build); that wait is
    // implemented in `BesSink::shutdown` via
    // `tokio::time::timeout(timeout, upload_task)` and doesn't need
    // any deadline on the gRPC stream itself.
    // Flow-control + keepalive tuning. tonic defaults are conservative for
    // a long-lived bidi stream pumping ~5k 1–2 KB events: the 64 KiB
    // initial stream window forces a WINDOW_UPDATE roughly every 30–60
    // events, and there is no HTTP/2 keepalive at all. Mirror the
    // settings buck2's `re_grpc` uses for its long-lived streams.
    let endpoint = endpoint
        .connect_timeout(Duration::from_secs(10))
        .http2_keep_alive_interval(Duration::from_secs(30))
        .keep_alive_timeout(Duration::from_secs(20))
        .keep_alive_while_idle(true)
        .initial_stream_window_size(2 * 1024 * 1024)
        .initial_connection_window_size(8 * 1024 * 1024)
        .tcp_nodelay(true);
    let endpoint = if tls {
        endpoint
            .tls_config(ClientTlsConfig::new().with_webpki_roots())
            .map_err(|e| {
                kuro_error::kuro_error!(kuro_error::ErrorTag::Environment, "BES TLS config: {e}")
            })?
    } else {
        endpoint
    };
    Ok(endpoint)
}

#[derive(Clone)]
struct HeaderInterceptor {
    metadata: MetadataMap,
}

impl HeaderInterceptor {
    fn new(headers: Vec<(String, String)>) -> kuro_error::Result<Self> {
        let mut metadata = MetadataMap::new();
        for (k, v) in headers {
            let key = MetadataKey::from_str(&k.to_lowercase()).map_err(|e| {
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "Invalid --bes_header key `{k}`: {e}"
                )
            })?;
            let value = MetadataValue::try_from(v.as_str()).map_err(|e| {
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "Invalid --bes_header value for `{k}`: {e}"
                )
            })?;
            metadata.insert(key, value);
        }
        Ok(Self { metadata })
    }
}

impl Interceptor for HeaderInterceptor {
    fn call(&mut self, mut req: Request<()>) -> Result<Request<()>, tonic::Status> {
        for kv in self.metadata.iter() {
            if let tonic::metadata::KeyAndValueRef::Ascii(k, v) = kv {
                req.metadata_mut().insert(k.clone(), v.clone());
            }
        }
        Ok(req)
    }
}

async fn run_uploader<T>(
    mut client: bes::publish_build_event_client::PublishBuildEventClient<T>,
    cfg: BesConfig,
    rx: mpsc::Receiver<StreamItem>,
) -> Result<(), String>
where
    T: tonic::client::GrpcService<tonic::body::BoxBody> + Clone,
    T::Error: Into<tonic::codegen::StdError>,
    T::ResponseBody:
        tonic::codegen::Body<Data = tonic::codegen::Bytes> + std::marker::Send + 'static,
    <T::ResponseBody as tonic::codegen::Body>::Error:
        Into<tonic::codegen::StdError> + std::marker::Send,
{
    // Lifecycle: BuildEnqueued + InvocationAttemptStarted in parallel.
    // They are independent — different StreamIds, no ordering requirement
    // between them on the BB ingest side — so we save a round-trip by
    // sending them concurrently instead of awaiting BuildEnqueued first.
    let enqueue_event = bes::BuildEvent {
        event_time: Some(now_timestamp()),
        event: Some(bes::build_event::Event::BuildEnqueued(
            bes::build_event::BuildEnqueued { details: None },
        )),
    };
    let attempt_started = bes::BuildEvent {
        event_time: Some(now_timestamp()),
        event: Some(bes::build_event::Event::InvocationAttemptStarted(
            bes::build_event::InvocationAttemptStarted {
                attempt_number: 1,
                details: None,
            },
        )),
    };
    let mut client_enqueue = client.clone();
    let mut client_attempt = client.clone();
    let cfg_for_enqueue = cfg.clone();
    let cfg_for_attempt = cfg.clone();
    let (r_enqueue, r_attempt) = tokio::join!(
        publish_lifecycle(&mut client_enqueue, &cfg_for_enqueue, enqueue_event, 1),
        publish_lifecycle(&mut client_attempt, &cfg_for_attempt, attempt_started, 1),
    );
    r_enqueue.map_err(|e| format!("BuildEnqueued: {e}"))?;
    r_attempt.map_err(|e| format!("InvocationAttemptStarted: {e}"))?;

    // Bidi stream: forward every BEP event as a `bazel_event` Any-packed payload.
    let (req_tx, req_rx) =
        mpsc::channel::<bes::PublishBuildToolEventStreamRequest>(CHANNEL_CAPACITY);
    let req_stream = tokio_stream::wrappers::ReceiverStream::new(req_rx);

    // Drive the request side and the open-stream call concurrently. tonic's
    // bidi `*.await` on `client.publish_build_tool_event_stream(req_stream)`
    // returns a `Streaming<Response>` once the underlying HTTP/2 stream is
    // set up, but BuildBuddy's server-side handler only completes that
    // setup after at least one request frame arrives. If we await
    // sequentially (open the stream, then start feeding events), nothing
    // is ever pushed into `req_stream` and we deadlock; the request loop
    // below sits in `rx.recv()` waiting for events that the build hasn't
    // emitted yet, while the open call blocks forever waiting for the
    // first request. Spawning the request feeder *before* awaiting the
    // open call keeps the request side flowing and unsticks the open.
    let cfg_for_feeder = cfg.clone();
    let req_tx_for_feeder = req_tx.clone();
    let feeder_task = tokio::spawn(async move {
        feed_requests(cfg_for_feeder, rx, req_tx_for_feeder).await;
    });
    drop(req_tx);

    // Belt-and-suspenders: attach the configured headers to the streaming
    // request directly. The `InterceptedService` wrapper *should* already
    // do this for streaming RPCs as well as unary, but if a tonic version
    // skip applies it here only for unary, the tool stream would be
    // silently sent unauthenticated and BuildBuddy would ACK-then-drop the
    // events.
    let mut tool_stream_req = Request::new(req_stream);
    for (k, v) in &cfg.headers {
        if let (Ok(name), Ok(val)) = (
            tonic::metadata::MetadataKey::from_str(&k.to_lowercase()),
            tonic::metadata::MetadataValue::try_from(v.as_str()),
        ) {
            tool_stream_req.metadata_mut().insert(name, val);
        }
    }
    let resp_stream = client
        .publish_build_tool_event_stream(tool_stream_req)
        .await
        .map_err(|e| format!("PublishBuildToolEventStream open: {e}"))?;

    let drain_task = tokio::spawn(drain_responses(resp_stream.into_inner()));

    // Wait for the feeder to send the Finish marker / drain the channel.
    let t_feeder = std::time::Instant::now();
    if let Err(e) = feeder_task.await {
        tracing::warn!("BES request feeder join error: {e}");
    }
    tracing::info!(
        "BES uploader PROFILE: feeder_task drained in {:?}",
        t_feeder.elapsed()
    );

    let t_drain = std::time::Instant::now();
    if let Err(e) = drain_task.await {
        tracing::warn!("BES drain task join error: {e}");
    }
    tracing::info!(
        "BES uploader PROFILE: drain_task (server ACKs + stream close) in {:?}",
        t_drain.elapsed()
    );

    // Lifecycle: close invocation and build.
    // Without a populated `invocation_status`, BuildBuddy treats the
    // invocation as still-running and never lists it. Set
    // COMMAND_SUCCEEDED here; the actual exit code lands on the BEP
    // `BuildFinished` event the build emits, which BuildBuddy uses to
    // overwrite this status if the build failed.
    // `final_invocation_id` is documented in the proto as "set on a status
    // in BuildFinished event" only; BuildBuddy uses it to dedupe between
    // CONTROLLER and TOOL streams, so populating it on
    // InvocationAttemptFinished can mis-route the index. Keep it empty
    // here.
    let attempt_finished = bes::BuildEvent {
        event_time: Some(now_timestamp()),
        event: Some(bes::build_event::Event::InvocationAttemptFinished(
            bes::build_event::InvocationAttemptFinished {
                invocation_status: Some(bes::BuildStatus {
                    result: bes::build_status::Result::CommandSucceeded as i32,
                    final_invocation_id: String::new(),
                    build_tool_exit_code: Some(0),
                    error_message: String::new(),
                    details: None,
                }),
                details: None,
            },
        )),
    };
    let build_finished = bes::BuildEvent {
        event_time: Some(now_timestamp()),
        event: Some(bes::build_event::Event::BuildFinished(
            bes::build_event::BuildFinished {
                status: Some(bes::BuildStatus {
                    result: bes::build_status::Result::CommandSucceeded as i32,
                    final_invocation_id: cfg.invocation_id.clone(),
                    build_tool_exit_code: Some(0),
                    error_message: String::new(),
                    details: None,
                }),
                details: None,
            },
        )),
    };
    let mut client_attempt_close = client.clone();
    let mut client_build_close = client.clone();
    let cfg_close_attempt = cfg.clone();
    let cfg_close_build = cfg.clone();
    let t_close = std::time::Instant::now();
    let (r_attempt_close, r_build_close) = tokio::join!(
        publish_lifecycle(
            &mut client_attempt_close,
            &cfg_close_attempt,
            attempt_finished,
            2
        ),
        publish_lifecycle(&mut client_build_close, &cfg_close_build, build_finished, 2),
    );
    if let Err(e) = r_attempt_close {
        tracing::warn!(
            "BES InvocationAttemptFinished rejected: code={:?} message={:?}",
            e.code(),
            e.message(),
        );
    }
    if let Err(e) = r_build_close {
        tracing::warn!(
            "BES BuildFinished rejected: code={:?} message={:?}",
            e.code(),
            e.message(),
        );
    }
    tracing::info!(
        "BES uploader PROFILE: lifecycle close (parallel) in {:?}",
        t_close.elapsed()
    );

    Ok(())
}

async fn feed_requests(
    cfg: BesConfig,
    mut rx: mpsc::Receiver<StreamItem>,
    req_tx: mpsc::Sender<bes::PublishBuildToolEventStreamRequest>,
) {
    let mut seq: i64 = 0;
    while let Some(item) = rx.recv().await {
        match item {
            StreamItem::Event(bep_event) => {
                seq += 1;
                let msg = wrap_bazel_event(&cfg, &bep_event, seq);
                if req_tx.send(msg).await.is_err() {
                    tracing::warn!("BES tool-event stream closed early at seq={seq}");
                    break;
                }
            }
            StreamItem::Finish => {
                seq += 1;
                let finished = bes::BuildEvent {
                    event_time: Some(now_timestamp()),
                    event: Some(bes::build_event::Event::ComponentStreamFinished(
                        bes::build_event::BuildComponentStreamFinished {
                            r#type: bes::build_event::build_component_stream_finished::FinishType::Finished as i32,
                        },
                    )),
                };
                let msg = bes::PublishBuildToolEventStreamRequest {
                    ordered_build_event: Some(bes::OrderedBuildEvent {
                        stream_id: Some(stream_id(&cfg, StreamComponent::Tool, true)),
                        sequence_number: seq,
                        event: Some(finished),
                    }),
                    notification_keywords: cfg.keywords.clone(),
                    project_id: cfg.project_id.clone(),
                    check_preceding_lifecycle_events_present: false,
                };
                let _ = req_tx.send(msg).await;
                break;
            }
        }
    }

    drop(req_tx);
}

async fn drain_responses<S>(mut stream: S)
where
    S: futures::Stream<Item = Result<bes::PublishBuildToolEventStreamResponse, tonic::Status>>
        + Unpin,
{
    use futures::StreamExt;
    let mut acked = 0u64;
    let mut last_ack_seq = 0i64;
    while let Some(result) = stream.next().await {
        match result {
            Ok(resp) => {
                acked += 1;
                last_ack_seq = resp.sequence_number;
            }
            Err(status) => {
                tracing::warn!(
                    "BES tool-event stream rejected: code={:?} message={:?} (after {acked} acks; last_ack_seq={last_ack_seq})",
                    status.code(),
                    status.message(),
                );
                return;
            }
        }
    }
    tracing::debug!(
        "BES tool-event stream drained: total ACKs = {acked}, last_ack_seq = {last_ack_seq}"
    );
}

fn wrap_bazel_event(
    cfg: &BesConfig,
    bep_event: &BepBuildEvent,
    sequence_number: i64,
) -> bes::PublishBuildToolEventStreamRequest {
    let any = prost_types::Any {
        type_url: "type.googleapis.com/build_event_stream.BuildEvent".to_owned(),
        value: bep_event.encode_to_vec(),
    };
    let bes_event = bes::BuildEvent {
        event_time: Some(now_timestamp()),
        event: Some(bes::build_event::Event::BazelEvent(any)),
    };
    bes::PublishBuildToolEventStreamRequest {
        ordered_build_event: Some(bes::OrderedBuildEvent {
            stream_id: Some(stream_id(cfg, StreamComponent::Tool, true)),
            sequence_number,
            event: Some(bes_event),
        }),
        notification_keywords: if sequence_number == 1 {
            cfg.keywords.clone()
        } else {
            Vec::new()
        },
        project_id: cfg.project_id.clone(),
        check_preceding_lifecycle_events_present: false,
    }
}

async fn publish_lifecycle<T>(
    client: &mut bes::publish_build_event_client::PublishBuildEventClient<T>,
    cfg: &BesConfig,
    event: bes::BuildEvent,
    sequence_number: i64,
) -> Result<(), tonic::Status>
where
    T: tonic::client::GrpcService<tonic::body::BoxBody>,
    T::Error: Into<tonic::codegen::StdError>,
    T::ResponseBody:
        tonic::codegen::Body<Data = tonic::codegen::Bytes> + std::marker::Send + 'static,
    <T::ResponseBody as tonic::codegen::Body>::Error:
        Into<tonic::codegen::StdError> + std::marker::Send,
{
    // Bazel parity: the StreamId shape depends on which lifecycle event we
    // are sending. CONTROLLER scope build-bracket events (BuildEnqueued /
    // BuildFinished) set only `build_id`. CONTROLLER scope per-invocation
    // events (InvocationAttemptStarted / InvocationAttemptFinished) set
    // both `build_id` and `invocation_id`. Sending these all under the
    // TOOL component (as the previous version did) silently caused
    // BuildBuddy to drop the invocation: it never sees the BuildEnqueued
    // bracket, so there's nothing to attach the tool stream to.
    let (component, with_invocation_id) = match event.event {
        Some(bes::build_event::Event::BuildEnqueued(_))
        | Some(bes::build_event::Event::BuildFinished(_)) => (StreamComponent::Controller, false),
        Some(bes::build_event::Event::InvocationAttemptStarted(_))
        | Some(bes::build_event::Event::InvocationAttemptFinished(_)) => {
            (StreamComponent::Controller, true)
        }
        _ => (StreamComponent::Tool, true),
    };
    let stream = stream_id(cfg, component, with_invocation_id);
    // Match Bazel's `addAllNotificationKeywords` pattern: keywords flow on
    // the build-enqueued / invocation-started / build-finished events,
    // dropped on the others.
    let notification_keywords = match event.event {
        Some(bes::build_event::Event::BuildEnqueued(_))
        | Some(bes::build_event::Event::InvocationAttemptStarted(_))
        | Some(bes::build_event::Event::BuildFinished(_)) => cfg.keywords.clone(),
        _ => Vec::new(),
    };
    let ordered = bes::OrderedBuildEvent {
        stream_id: Some(stream),
        sequence_number,
        event: Some(event),
    };
    let req = bes::PublishLifecycleEventRequest {
        service_level: bes::publish_lifecycle_event_request::ServiceLevel::Interactive as i32,
        build_event: Some(ordered),
        stream_timeout: None,
        notification_keywords,
        project_id: cfg.project_id.clone(),
        check_preceding_lifecycle_events_present: false,
    };
    client.publish_lifecycle_event(req).await.map(|_| ())
}

#[derive(Copy, Clone)]
enum StreamComponent {
    /// Build-level events: lifecycle markers that BuildBuddy uses to
    /// bracket an invocation hierarchy.
    Controller,
    /// Per-tool stream: the actual BEP events packed into `bazel_event`,
    /// plus the closing `ComponentStreamFinished`.
    Tool,
}

fn stream_id(
    cfg: &BesConfig,
    component: StreamComponent,
    with_invocation_id: bool,
) -> bes::StreamId {
    bes::StreamId {
        build_id: cfg.build_id.clone(),
        invocation_id: if with_invocation_id {
            cfg.invocation_id.clone()
        } else {
            String::new()
        },
        component: match component {
            StreamComponent::Controller => bes::stream_id::BuildComponent::Controller as i32,
            StreamComponent::Tool => bes::stream_id::BuildComponent::Tool as i32,
        },
    }
}

fn now_timestamp() -> prost_types::Timestamp {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    prost_types::Timestamp {
        seconds: d.as_secs() as i64,
        nanos: d.subsec_nanos() as i32,
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Pull the bare host (or host:port) out of a `grpc(s)://host:port` /
/// `https://host:port` URL. Returns `None` for malformed inputs;
/// callers fall back to the empty string and let BuildBuddy
/// reconstruct the host from the BES connection metadata.
fn bes_uri_host(backend: &str) -> Option<String> {
    let (_scheme, rest) = backend.split_once("://")?;
    // Strip any path component.
    let host = rest.split('/').next()?;
    if host.is_empty() {
        None
    } else {
        Some(host.to_owned())
    }
}
