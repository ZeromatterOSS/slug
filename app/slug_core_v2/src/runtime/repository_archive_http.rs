use std::io;
use std::io::Write;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;

use http_body_util::BodyExt;
use http_body_util::Empty;
use hyper::Request;
use hyper::body::Bytes;
use hyper::body::Incoming;
use hyper::client::conn::http1;
use hyper_util::rt::TokioIo;
use sha2::Digest;
use sha2::Sha256;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::ReadBuf;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::TlsConnector;

use super::repository_archive::SelectedBcrTarGz;
use super::repository_io::ArchiveMaterializationError;

const CAPTURE_LIMIT: u64 = 128 * 1024 * 1024;
const MODULE_CAPTURE_LIMIT: u64 = 1024 * 1024;
const MAX_ADDRESSES: usize = 64;
const MAX_REDIRECTS: usize = 39;

#[derive(Clone, Copy)]
struct Limits {
    capture_bytes: u64,
    connect: Duration,
    header: Duration,
    frame: Duration,
    disposal: Duration,
}

impl Limits {
    const NATIVE: Self = Self {
        capture_bytes: CAPTURE_LIMIT,
        connect: Duration::from_secs(15),
        header: Duration::from_secs(30),
        frame: Duration::from_secs(30),
        disposal: Duration::from_secs(5),
    };
}

enum ArchiveStream {
    Tls(tokio_rustls::client::TlsStream<TcpStream>),
    #[cfg(test)]
    Plain(TcpStream),
    #[cfg(test)]
    StalledShutdown(StalledShutdown),
}

impl AsyncRead for ArchiveStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Tls(stream) => Pin::new(stream).poll_read(cx, buffer),
            #[cfg(test)]
            Self::Plain(stream) => Pin::new(stream).poll_read(cx, buffer),
            #[cfg(test)]
            Self::StalledShutdown(stream) => Pin::new(stream).poll_read(cx, buffer),
        }
    }
}

impl AsyncWrite for ArchiveStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Self::Tls(stream) => Pin::new(stream).poll_write(cx, bytes),
            #[cfg(test)]
            Self::Plain(stream) => Pin::new(stream).poll_write(cx, bytes),
            #[cfg(test)]
            Self::StalledShutdown(stream) => Pin::new(stream).poll_write(cx, bytes),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Tls(stream) => Pin::new(stream).poll_flush(cx),
            #[cfg(test)]
            Self::Plain(stream) => Pin::new(stream).poll_flush(cx),
            #[cfg(test)]
            Self::StalledShutdown(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Tls(stream) => Pin::new(stream).poll_shutdown(cx),
            #[cfg(test)]
            Self::Plain(stream) => Pin::new(stream).poll_shutdown(cx),
            #[cfg(test)]
            Self::StalledShutdown(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}

#[cfg(test)]
struct StalledShutdown(TcpStream);

#[cfg(test)]
impl AsyncRead for StalledShutdown {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_read(cx, buffer)
    }
}

#[cfg(test)]
impl AsyncWrite for StalledShutdown {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().0).poll_write(cx, bytes)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Pending
    }
}

type HttpIo = TokioIo<ArchiveStream>;
type HttpConnection = http1::Connection<HttpIo, Empty<Bytes>>;
type HttpSender = http1::SendRequest<Empty<Bytes>>;

trait Environment {
    fn limits(&self) -> Limits;
    fn resolve(&self, url: &url::Url) -> Result<Vec<SocketAddr>, String>;
    fn connect(
        &self,
        runtime: &tokio::runtime::Runtime,
        url: &url::Url,
        address: SocketAddr,
    ) -> Result<ArchiveStream, String>;
    fn capture(&self) -> Result<tempfile::NamedTempFile, String>;
}

struct NativeEnvironment {
    tls: Arc<rustls::ClientConfig>,
}

impl NativeEnvironment {
    fn new() -> Result<Self, ArchiveMaterializationError> {
        let loaded = rustls_native_certs::load_native_certs();
        let mut roots = rustls::RootCertStore::empty();
        for certificate in loaded.certs {
            roots.add(certificate).map_err(|error| {
                ArchiveMaterializationError::transport(format!(
                    "loading native TLS certificate: {error}"
                ))
            })?;
        }
        if roots.is_empty() {
            return Err(ArchiveMaterializationError::transport(format!(
                "loading native TLS roots produced no certificates{}",
                loaded
                    .errors
                    .last()
                    .map(|error| format!(": {error}"))
                    .unwrap_or_default()
            )));
        }
        let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|error| {
            ArchiveMaterializationError::transport(format!("configuring TLS: {error}"))
        })?
        .with_root_certificates(roots)
        .with_no_client_auth();
        Ok(Self { tls: Arc::new(tls) })
    }
}

impl Environment for NativeEnvironment {
    fn limits(&self) -> Limits {
        Limits::NATIVE
    }

    fn resolve(&self, url: &url::Url) -> Result<Vec<SocketAddr>, String> {
        let host = url.host_str().ok_or("BCR archive URL has no host")?;
        let port = url
            .port_or_known_default()
            .ok_or("BCR archive URL has no port")?;
        let addresses = (host, port)
            .to_socket_addrs()
            .map_err(|error| format!("resolving {host}: {error}"))?
            .take(MAX_ADDRESSES + 1)
            .collect::<Vec<_>>();
        if addresses.is_empty() {
            return Err(format!("resolving {host}: no addresses"));
        }
        if addresses.len() > MAX_ADDRESSES {
            return Err(format!(
                "resolving {host}: more than {MAX_ADDRESSES} addresses"
            ));
        }
        Ok(addresses)
    }

    fn connect(
        &self,
        runtime: &tokio::runtime::Runtime,
        url: &url::Url,
        address: SocketAddr,
    ) -> Result<ArchiveStream, String> {
        let stream = runtime
            .block_on(async { timeout(self.limits().connect, TcpStream::connect(address)).await })
            .map_err(|_| "TCP connect timed out".to_owned())?
            .map_err(|error| format!("TCP connect: {error}"))?;
        let host = url.host_str().ok_or("BCR archive URL has no host")?;
        let server_name = rustls::pki_types::ServerName::try_from(host.to_owned())
            .map_err(|error| format!("TLS server name {host}: {error}"))?;
        let stream = runtime
            .block_on(async {
                timeout(
                    self.limits().header,
                    TlsConnector::from(self.tls.clone()).connect(server_name, stream),
                )
                .await
            })
            .map_err(|_| "TLS handshake timed out".to_owned())?
            .map_err(|error| format!("TLS handshake: {error}"))?;
        Ok(ArchiveStream::Tls(stream))
    }

    fn capture(&self) -> Result<tempfile::NamedTempFile, String> {
        tempfile::NamedTempFile::new().map_err(|error| format!("creating capture: {error}"))
    }
}

pub(super) fn capture_selected_bcr(
    plan: &SelectedBcrTarGz,
    runtime: &tokio::runtime::Runtime,
    active: &dyn Fn() -> bool,
) -> Result<tempfile::NamedTempFile, ArchiveMaterializationError> {
    let environment = NativeEnvironment::new()?;
    capture_selected_bcr_with(plan, runtime, active, &environment)
}

fn capture_selected_bcr_with(
    plan: &SelectedBcrTarGz,
    runtime: &tokio::runtime::Runtime,
    active: &dyn Fn() -> bool,
    environment: &impl Environment,
) -> Result<tempfile::NamedTempFile, ArchiveMaterializationError> {
    capture_urls(
        &plan.urls,
        plan.integrity,
        environment.limits().capture_bytes,
        "archive",
        runtime,
        active,
        environment,
    )
}

pub(super) fn capture_selected_bcr_module(
    plan: &SelectedBcrTarGz,
    runtime: &tokio::runtime::Runtime,
    active: &dyn Fn() -> bool,
) -> Result<tempfile::NamedTempFile, ArchiveMaterializationError> {
    let environment = NativeEnvironment::new()?;
    capture_urls(
        std::slice::from_ref(&plan.module_url),
        plan.module_integrity,
        MODULE_CAPTURE_LIMIT,
        "MODULE",
        runtime,
        active,
        &environment,
    )
}

fn capture_urls(
    urls: &[String],
    integrity: [u8; 32],
    capture_limit: u64,
    subject: &'static str,
    runtime: &tokio::runtime::Runtime,
    active: &dyn Fn() -> bool,
    environment: &impl Environment,
) -> Result<tempfile::NamedTempFile, ArchiveMaterializationError> {
    let mut last = None;
    for url in urls {
        if !active() {
            return Err(ArchiveMaterializationError::transport(
                "repository session is no longer active",
            ));
        }
        match capture_one(
            url,
            integrity,
            capture_limit,
            subject,
            runtime,
            active,
            environment,
        ) {
            Ok(capture) => return Ok(capture),
            Err(error) => last = Some(error),
        }
    }
    Err(ArchiveMaterializationError::transport(format!(
        "selected-registry BCR {subject} capture failed: {}",
        last.unwrap_or_else(|| "no archive URL".into())
    )))
}

fn capture_one(
    original: &str,
    integrity: [u8; 32],
    capture_limit: u64,
    subject: &'static str,
    runtime: &tokio::runtime::Runtime,
    active: &dyn Fn() -> bool,
    environment: &impl Environment,
) -> Result<tempfile::NamedTempFile, String> {
    let mut capture = environment.capture()?;
    let mut url = url::Url::parse(original).map_err(|error| error.to_string())?;
    for redirect in 0..=MAX_REDIRECTS {
        if !active() {
            return Err("repository session is no longer active".into());
        }
        let response = request(&url, runtime, active, environment)?;
        let status = response.status;
        let location = response.location;
        let content_length = response.content_length;
        let mut body = response.body;
        let mut connection = response.connection;
        let sender = response.sender;
        if redirect_status(status) {
            drop(body);
            finish(runtime, connection, sender, environment.limits())?;
            if redirect == MAX_REDIRECTS {
                return Err("BCR archive rejected the 40th redirect".into());
            }
            let location = location.ok_or("redirect response missing Location")?;
            url = url
                .join(&location)
                .map_err(|error| format!("redirect URL: {error}"))?;
            validate_https(&url)?;
            continue;
        }
        if status != hyper::StatusCode::OK && status != hyper::StatusCode::PARTIAL_CONTENT {
            drop(body);
            let _ = finish(runtime, connection, sender, environment.limits());
            return Err(format!("HTTP status {status}"));
        }
        let content_length = match content_length
            .map(|value| {
                value
                    .parse::<u64>()
                    .map_err(|_| "invalid Content-Length".to_owned())
            })
            .transpose()
        {
            Ok(length) => length,
            Err(error) => {
                drop(body);
                let _ = finish(runtime, connection, sender, environment.limits());
                return Err(error);
            }
        };
        if content_length.is_some_and(|length| length > capture_limit) {
            drop(body);
            let _ = finish(runtime, connection, sender, environment.limits());
            return Err(format!(
                "BCR {subject} exceeds {capture_limit} byte capture limit"
            ));
        }
        let mut hasher = Sha256::new();
        let body_result = capture_body(
            runtime,
            active,
            environment.limits(),
            capture_limit,
            subject,
            &mut body,
            &mut connection,
            &mut capture,
            &mut hasher,
        );
        drop(body);
        let disposal = finish(runtime, connection, sender, environment.limits());
        body_result?;
        disposal?;
        capture
            .flush()
            .map_err(|error| format!("flushing capture: {error}"))?;
        if hasher.finalize().as_slice() != integrity {
            return Err(format!("BCR {subject} SHA-256 SRI mismatch"));
        }
        return Ok(capture);
    }
    Err("BCR archive rejected the 40th redirect".into())
}

fn capture_body(
    runtime: &tokio::runtime::Runtime,
    active: &dyn Fn() -> bool,
    limits: Limits,
    capture_limit: u64,
    subject: &'static str,
    body: &mut Incoming,
    connection: &mut Option<Pin<Box<HttpConnection>>>,
    capture: &mut tempfile::NamedTempFile,
    hasher: &mut Sha256,
) -> Result<(), String> {
    let mut written = 0u64;
    loop {
        if !active() {
            return Err("repository session is no longer active".into());
        }
        let Some(frame) = next_frame(runtime, body, connection, limits)? else {
            return Ok(());
        };
        if let Ok(data) = frame.into_data() {
            written = written
                .checked_add(data.len() as u64)
                .filter(|size| *size <= capture_limit)
                .ok_or_else(|| {
                    format!("BCR {subject} exceeds {capture_limit} byte capture limit")
                })?;
            capture
                .write_all(&data)
                .map_err(|error| format!("writing capture: {error}"))?;
            hasher.update(&data);
        }
    }
}

struct ResponseOwner {
    status: hyper::StatusCode,
    location: Option<String>,
    content_length: Option<String>,
    body: Incoming,
    connection: Option<Pin<Box<HttpConnection>>>,
    sender: HttpSender,
}

fn request(
    url: &url::Url,
    runtime: &tokio::runtime::Runtime,
    active: &dyn Fn() -> bool,
    environment: &impl Environment,
) -> Result<ResponseOwner, String> {
    validate_https(url)?;
    let addresses = environment.resolve(url)?;
    if addresses.len() > MAX_ADDRESSES {
        return Err(format!(
            "BCR archive resolver returned more than {MAX_ADDRESSES} addresses"
        ));
    }
    let mut last = None;
    for address in addresses {
        if !active() {
            return Err("repository session is no longer active".into());
        }
        match request_address(url, address, runtime, environment) {
            Ok(response) => return Ok(response),
            Err(error) => last = Some(error),
        }
    }
    Err(last.unwrap_or_else(|| "BCR archive resolver returned no addresses".into()))
}

fn request_address(
    url: &url::Url,
    address: SocketAddr,
    runtime: &tokio::runtime::Runtime,
    environment: &impl Environment,
) -> Result<ResponseOwner, String> {
    let stream = environment.connect(runtime, url, address)?;
    let (mut sender, connection) = runtime
        .block_on(async {
            timeout(
                environment.limits().header,
                http1::handshake(TokioIo::new(stream)),
            )
            .await
        })
        .map_err(|_| "HTTP handshake timed out".to_owned())?
        .map_err(|error| format!("HTTP handshake: {error}"))?;
    let uri = url
        .as_str()
        .parse::<hyper::Uri>()
        .map_err(|error| format!("parsing request URI: {error}"))?;
    let authority = uri
        .authority()
        .ok_or("BCR archive URL has no authority")?
        .as_str();
    let target = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");
    let request = Request::builder()
        .method("GET")
        .uri(target)
        .header(hyper::header::HOST, authority)
        .body(Empty::new())
        .map_err(|error| format!("building request: {error}"))?;
    let mut connection = Box::pin(connection);
    let (response, connection_completed) = runtime
        .block_on(async {
            timeout(environment.limits().header, async {
                let mut response = Box::pin(sender.send_request(request));
                tokio::select! {
                    biased;
                    response = response.as_mut() => {
                        response
                            .map(|response| (response, false))
                            .map_err(|error| format!("HTTP response: {error}"))
                    }
                    result = connection.as_mut() => {
                        result.map_err(|error| {
                            format!("HTTP connection ended before response: {error}")
                        })?;
                        response
                            .await
                            .map(|response| (response, true))
                            .map_err(|error| format!("HTTP response: {error}"))
                    }
                }
            })
            .await
        })
        .map_err(|_| "HTTP response header timed out".to_owned())??;
    let location = response
        .headers()
        .get(hyper::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let content_length = response
        .headers()
        .get(hyper::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    Ok(ResponseOwner {
        status: response.status(),
        location,
        content_length,
        body: response.into_body(),
        connection: (!connection_completed).then_some(connection),
        sender,
    })
}

fn validate_https(url: &url::Url) -> Result<(), String> {
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("BCR archive URL must be unauthenticated HTTPS".into());
    }
    Ok(())
}

fn redirect_status(status: hyper::StatusCode) -> bool {
    matches!(
        status,
        hyper::StatusCode::MOVED_PERMANENTLY
            | hyper::StatusCode::FOUND
            | hyper::StatusCode::SEE_OTHER
            | hyper::StatusCode::TEMPORARY_REDIRECT
    )
}

fn next_frame(
    runtime: &tokio::runtime::Runtime,
    body: &mut Incoming,
    connection: &mut Option<Pin<Box<HttpConnection>>>,
    limits: Limits,
) -> Result<Option<hyper::body::Frame<Bytes>>, String> {
    if connection.is_none() {
        return runtime
            .block_on(async { timeout(limits.frame, body.frame()).await })
            .map_err(|_| "HTTP body frame timed out".to_owned())?
            .transpose()
            .map_err(|error| error.to_string());
    }
    enum FrameProgress {
        Frame(Option<Result<hyper::body::Frame<Bytes>, hyper::Error>>),
        Connection(hyper::Result<()>),
    }
    let progress = runtime
        .block_on(async {
            timeout(limits.frame, async {
                tokio::select! {
                    biased;
                    frame = body.frame() => FrameProgress::Frame(frame),
                    result = connection.as_mut().expect("checked connection").as_mut() => {
                        FrameProgress::Connection(result)
                    }
                }
            })
            .await
        })
        .map_err(|_| "HTTP body frame timed out".to_owned())?;
    match progress {
        FrameProgress::Frame(frame) => frame.transpose().map_err(|error| error.to_string()),
        FrameProgress::Connection(Ok(())) => {
            *connection = None;
            runtime
                .block_on(async { timeout(limits.frame, body.frame()).await })
                .map_err(|_| "HTTP body frame timed out".to_owned())?
                .transpose()
                .map_err(|error| error.to_string())
        }
        FrameProgress::Connection(Err(error)) => {
            Err(format!("HTTP connection ended while reading body: {error}"))
        }
    }
}

fn finish(
    runtime: &tokio::runtime::Runtime,
    mut connection: Option<Pin<Box<HttpConnection>>>,
    sender: HttpSender,
    limits: Limits,
) -> Result<(), String> {
    drop(sender);
    let Some(connection) = connection.as_mut() else {
        return Ok(());
    };
    match runtime.block_on(async { timeout(limits.disposal, connection.as_mut()).await }) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(format!("HTTP connection disposal: {error}")),
        Err(_) => Err("HTTP connection disposal timed out".into()),
    }
}

#[cfg(test)]
#[path = "tests/repository_archive_http_tests.rs"]
mod tests;
