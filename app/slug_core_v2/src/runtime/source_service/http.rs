//! Each response owns and drives its one HTTP1 connection. No executor tasks,
//! client pool, redirects, or body collection are installed by this leaf.
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::Empty;
use hyper::client::conn::http1;
use hyper_util::rt::TokioIo;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use super::SourceBufferReservation;
use super::SourceServiceError;
use super::SourceServiceScope;

trait SourceIo: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> SourceIo for T {}
type OwnedIo = Box<dyn SourceIo>;
type Connection = http1::Connection<TokioIo<OwnedIo>, Empty<Bytes>>;
type Sender = http1::SendRequest<Empty<Bytes>>;

fn http_error(scope: &SourceServiceScope, error: hyper::Error, ordinary: &str) -> String {
    // Client-only Hyper exposes no size-specific parse classifier. The private
    // diagnostic admits no malformed/oversized protocol head; ordinary status
    // and transport IO errors retain their existing fallback behavior.
    if error.is_parse() {
        scope
            .poison(SourceServiceError::UnsupportedSourceOwner)
            .to_string()
    } else {
        ordinary.to_owned()
    }
}

pub(super) fn bounded_addresses(
    scope: &SourceServiceScope,
    addresses: impl Iterator<Item = std::net::SocketAddr>,
) -> Result<Vec<std::net::SocketAddr>, String> {
    let mut bounded = Vec::with_capacity(64);
    for address in addresses.take(65) {
        if bounded.len() == 64 {
            return Err(scope.poison(SourceServiceError::BufferLimit).to_string());
        }
        bounded.push(address);
    }
    Ok(bounded)
}

#[derive(Clone, Copy)]
pub(in crate::runtime) enum SourceHttpAdmission {
    Registry,
    Archive,
}

pub(in crate::runtime) struct ScopedHttpResponse {
    scope: Arc<SourceServiceScope>,
    status: hyper::StatusCode,
    headers: hyper::HeaderMap,
    body: Option<hyper::body::Incoming>,
    sender: Option<Sender>,
    connection: Option<Pin<Box<Connection>>>,
    // The fixed HTTP parser/request bookkeeping is reserved before handshake.
    // TLS/library/kernel allocations are explicitly outside this buffer claim.
    _parser_credit: SourceBufferReservation,
}
impl ScopedHttpResponse {
    pub(in crate::runtime) fn status(&self) -> hyper::StatusCode {
        self.status
    }
    pub(in crate::runtime) fn headers(&self) -> &hyper::HeaderMap {
        &self.headers
    }
    pub(in crate::runtime) async fn next_data(&mut self) -> Result<Option<Bytes>, String> {
        loop {
            let body = self
                .body
                .as_mut()
                .ok_or_else(|| "source HTTP body already disposed".to_owned())?;
            let frame = body.frame();
            tokio::pin!(frame);
            let connection = &mut self.connection;
            let result=self.scope.during(Duration::from_secs(30),async {
                let result=if let Some(conn)=connection.as_mut() {
                    tokio::select! {
                        result=&mut frame=>result,
                        result=conn.as_mut()=>{
                            *connection=None;
                            match result {Ok(())=>frame.await,Err(error)=>return Ok(Err(http_error(&self.scope,error,"source HTTP connection failed")))}
                        }
                    }
                } else {frame.await};
                Ok(result.transpose().map_err(|error|http_error(&self.scope,error,"source HTTP body failed")))
            }).await.map_err(|e|e.to_string())??;
            match result {
                None => return Ok(None),
                Some(frame) => {
                    if let Ok(data) = frame.into_data() {
                        return Ok(Some(data));
                    }
                }
            }
        }
    }
    pub(in crate::runtime) async fn dispose(mut self) -> Result<(), String> {
        // No background task exists to join. Dropping the owned connection
        // closes its socket directly; polling it could drain a rejected body.
        let scope = Arc::clone(&self.scope);
        scope
            .during_cleanup(async {
                drop(self.body.take());
                drop(self.sender.take());
                drop(self.connection.take());
                Ok(())
            })
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub(super) fn validate_address(
    url: &url::Url,
    admission: SourceHttpAdmission,
) -> Result<(), String> {
    let admitted = url.as_str().len() <= 8192
        && matches!(url.scheme(), "http" | "https")
        && (matches!(admission, SourceHttpAdmission::Registry) || url.scheme() == "https")
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none();
    if admitted {
        Ok(())
    } else {
        Err("unsupported source HTTP address".to_owned())
    }
}

pub(in crate::runtime) async fn request_scoped(
    scope: &Arc<SourceServiceScope>,
    url: &url::Url,
    admission: SourceHttpAdmission,
) -> Result<ScopedHttpResponse, String> {
    scope.check().map_err(|e| e.to_string())?;
    if url.as_str().len() > 8192 {
        return Err(scope.poison(SourceServiceError::BufferLimit).to_string());
    }
    validate_address(url, admission)?;
    let host = url
        .host_str()
        .ok_or_else(|| "source HTTP host missing".to_owned())?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| "source HTTP port missing".to_owned())?;
    let credit = scope
        .reserve_buffer(64 * 1024 + 128 * 256 + 32 * 1024)
        .map_err(|e| e.to_string())?;
    let addresses = scope
        .during(Duration::from_secs(15), async {
            Ok(tokio::net::lookup_host((host, port))
                .await
                .map_err(|_| "source DNS lookup failed".to_owned()))
        })
        .await
        .map_err(|e| e.to_string())??;
    // No unbounded collect. DNS implementation itself is inherited and is
    // supervised externally; cancellation cannot guarantee OS resolver stop.
    let bounded = bounded_addresses(scope, addresses)?;
    let mut connected = None;
    for address in bounded {
        let result = scope
            .during(Duration::from_secs(15), async {
                Ok(TcpStream::connect(address).await)
            })
            .await
            .map_err(|e| e.to_string())?;
        if let Ok(stream) = result {
            connected = Some(stream);
            break;
        }
    }
    let stream = connected.ok_or_else(|| "source TCP connection failed".to_owned())?;
    let io: OwnedIo = if url.scheme() == "https" {
        scope.check().map_err(|e| e.to_string())?;
        // Native root discovery is an inherited synchronous asset boundary.
        let roots = rustls_native_certs::load_native_certs();
        let mut store = rustls::RootCertStore::empty();
        for certificate in roots.certs {
            store
                .add(certificate)
                .map_err(|_| "source TLS root rejected".to_owned())?;
        }
        if store.is_empty() {
            return Err("source TLS roots unavailable".to_owned());
        }
        let config = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|_| "source TLS protocol refused".to_owned())?
        .with_root_certificates(store)
        .with_no_client_auth();
        scope.check().map_err(|e| e.to_string())?;
        let name = rustls::pki_types::ServerName::try_from(host.to_owned())
            .map_err(|_| "source TLS name refused".to_owned())?;
        let stream = scope
            .during(Duration::from_secs(15), async {
                Ok(TlsConnector::from(Arc::new(config))
                    .connect(name, stream)
                    .await
                    .map_err(|_| "source TLS handshake failed".to_owned()))
            })
            .await
            .map_err(|e| e.to_string())??;
        Box::new(stream)
    } else {
        Box::new(stream)
    };
    request_on_io(scope, url, io, credit).await
}

async fn request_on_io(
    scope: &Arc<SourceServiceScope>,
    url: &url::Url,
    io: OwnedIo,
    credit: SourceBufferReservation,
) -> Result<ScopedHttpResponse, String> {
    let mut builder = http1::Builder::new();
    builder.max_buf_size(64 * 1024).max_headers(128);
    let (mut sender, connection) = scope
        .during(Duration::from_secs(15), async {
            Ok(builder
                .handshake(TokioIo::new(io))
                .await
                .map_err(|error| http_error(scope, error, "source HTTP handshake failed")))
        })
        .await
        .map_err(|e| e.to_string())??;
    let uri = url
        .as_str()
        .parse::<hyper::Uri>()
        .map_err(|_| "source HTTP URI refused".to_owned())?;
    let authority = uri
        .authority()
        .ok_or_else(|| "source HTTP authority missing".to_owned())?
        .as_str();
    let request = hyper::Request::get(uri.path_and_query().map_or("/", |v| v.as_str()))
        .header(hyper::header::HOST, authority)
        .body(Empty::<Bytes>::new())
        .map_err(|_| "source HTTP request refused".to_owned())?;
    let mut connection = Some(Box::pin(connection));
    let response=scope.during(Duration::from_secs(30),async {
        let request=sender.send_request(request);tokio::pin!(request);
        let result=tokio::select! {
            result=&mut request=>result,
            result=connection.as_mut().unwrap().as_mut()=>{
                connection=None;
                match result {Ok(())=>request.await,Err(error)=>return Ok(Err(http_error(scope,error,"source HTTP connection failed")))}
            }
        };
        Ok(result.map_err(|error|http_error(scope,error,"source HTTP headers failed")))
    }).await.map_err(|e|e.to_string())??;
    let (parts, body) = response.into_parts();
    Ok(ScopedHttpResponse {
        scope: Arc::clone(scope),
        status: parts.status,
        headers: parts.headers,
        body: Some(body),
        sender: Some(sender),
        connection,
        _parser_credit: credit,
    })
}

#[cfg(test)]
pub(super) async fn request_test_io(
    scope: &Arc<SourceServiceScope>,
    url: &url::Url,
    io: tokio::io::DuplexStream,
) -> Result<ScopedHttpResponse, String> {
    let credit = scope
        .reserve_buffer(64 * 1024 + 128 * 256 + 32 * 1024)
        .map_err(|e| e.to_string())?;
    request_on_io(scope, url, Box::new(io), credit).await
}
