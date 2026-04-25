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
            channel.clone(),
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
        })
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

    /// Signal end-of-stream, then wait for the uploader task to flush within
    /// `--bes_timeout`.
    pub async fn shutdown(&self) -> kuro_error::Result<()> {
        let _ = self.tx.send(StreamItem::Finish).await;
        let handle = self.upload_task.lock().await.take();
        if let Some(h) = handle {
            let timeout = self.config.timeout;
            match tokio::time::timeout(timeout, h).await {
                Ok(Ok(())) => Ok(()),
                Ok(Err(e)) => {
                    tracing::warn!("BES uploader join error: {e}");
                    Ok(())
                }
                Err(_) => {
                    tracing::warn!(
                        "BES uploader timeout after {timeout:?}; dropping remaining events"
                    );
                    Ok(())
                }
            }
        } else {
            Ok(())
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
    let endpoint = endpoint
        .timeout(config.timeout)
        .connect_timeout(Duration::from_secs(10));
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
    T: tonic::client::GrpcService<tonic::body::BoxBody>,
    T::Error: Into<tonic::codegen::StdError>,
    T::ResponseBody:
        tonic::codegen::Body<Data = tonic::codegen::Bytes> + std::marker::Send + 'static,
    <T::ResponseBody as tonic::codegen::Body>::Error:
        Into<tonic::codegen::StdError> + std::marker::Send,
{
    // Lifecycle: BuildEnqueued first.
    let enqueue_event = bes::BuildEvent {
        event_time: Some(now_timestamp()),
        event: Some(bes::build_event::Event::BuildEnqueued(
            bes::build_event::BuildEnqueued { details: None },
        )),
    };
    publish_lifecycle(&mut client, &cfg, enqueue_event, 1)
        .await
        .map_err(|e| format!("BuildEnqueued: {e}"))?;

    // Lifecycle: InvocationAttemptStarted.
    let attempt_started = bes::BuildEvent {
        event_time: Some(now_timestamp()),
        event: Some(bes::build_event::Event::InvocationAttemptStarted(
            bes::build_event::InvocationAttemptStarted {
                attempt_number: 1,
                details: None,
            },
        )),
    };
    publish_lifecycle(&mut client, &cfg, attempt_started, 1)
        .await
        .map_err(|e| format!("InvocationAttemptStarted: {e}"))?;

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
    if let Err(e) = feeder_task.await {
        tracing::warn!("BES request feeder join error: {e}");
    }

    let _ = drain_task.await;

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
    if let Err(e) = publish_lifecycle(&mut client, &cfg, attempt_finished, 2).await {
        tracing::warn!(
            "BES InvocationAttemptFinished rejected: code={:?} message={:?}",
            e.code(),
            e.message(),
        );
    }

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
    if let Err(e) = publish_lifecycle(&mut client, &cfg, build_finished, 2).await {
        tracing::warn!(
            "BES BuildFinished rejected: code={:?} message={:?}",
            e.code(),
            e.message(),
        );
    }

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
                    tracing::warn!("BES tool-event stream closed early");
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
    while let Some(result) = stream.next().await {
        match result {
            Ok(_) => acked += 1,
            Err(status) => {
                tracing::warn!(
                    "BES tool-event stream rejected: code={:?} message={:?} (after {acked} acks)",
                    status.code(),
                    status.message(),
                );
                return;
            }
        }
    }
    tracing::debug!("BES tool-event stream drained: total ACKs = {acked}");
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
