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
use slug_bzlmod_v2::RegistryRefusalCause;
use slug_bzlmod_v2::RegistryRefusalSite;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use super::SourceBufferReservation;
use super::SourceServiceError;
use super::SourceServiceScope;
use super::SourceTransportFailure;
use super::failure::hyper_detail;

trait SourceIo: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> SourceIo for T {}
type OwnedIo = Box<dyn SourceIo>;
type Connection = http1::Connection<TokioIo<OwnedIo>, Empty<Bytes>>;
type Sender = http1::SendRequest<Empty<Bytes>>;

fn http_error(
    scope: &SourceServiceScope,
    error: hyper::Error,
    site: RegistryRefusalSite,
    ordinary: &str,
) -> SourceTransportFailure {
    let detail = hyper_detail(&error);
    // Preserve the existing parse poison and message. A prior sticky refusal
    // remains the selected Scope failure rather than being relabelled as parse.
    if error.is_parse() {
        let poison = scope.poison(SourceServiceError::UnsupportedSourceOwner);
        if poison != SourceServiceError::UnsupportedSourceOwner {
            return SourceTransportFailure::scope(poison);
        }
        SourceTransportFailure::new(poison.to_string(), RegistryRefusalCause { site, detail })
    } else {
        SourceTransportFailure::new(ordinary, RegistryRefusalCause { site, detail })
    }
}

fn bounded_addresses_typed(
    scope: &SourceServiceScope,
    addresses: impl Iterator<Item = std::net::SocketAddr>,
) -> Result<Vec<std::net::SocketAddr>, SourceTransportFailure> {
    let mut bounded = Vec::with_capacity(64);
    for address in addresses.take(65) {
        if bounded.len() == 64 {
            return Err(SourceTransportFailure::scope(
                scope.poison(SourceServiceError::BufferLimit),
            ));
        }
        bounded.push(address);
    }
    Ok(bounded)
}

#[cfg(test)]
pub(super) fn bounded_addresses(
    scope: &SourceServiceScope,
    addresses: impl Iterator<Item = std::net::SocketAddr>,
) -> Result<Vec<std::net::SocketAddr>, String> {
    bounded_addresses_typed(scope, addresses).map_err(SourceTransportFailure::into_message)
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
    pub(in crate::runtime) async fn next_data_typed(
        &mut self,
    ) -> Result<Option<Bytes>, SourceTransportFailure> {
        loop {
            let body = self.body.as_mut().ok_or_else(|| {
                SourceTransportFailure::opaque(
                    RegistryRefusalSite::InternalState,
                    "source HTTP body already disposed",
                )
            })?;
            let frame = body.frame();
            tokio::pin!(frame);
            let connection = &mut self.connection;
            let result=self.scope.during(Duration::from_secs(30),async {
                let result=if let Some(conn)=connection.as_mut() {
                    tokio::select! {
                        result=&mut frame=>result,
                        result=conn.as_mut()=>{
                            *connection=None;
                            match result {Ok(())=>frame.await,Err(error)=>return Ok(Err(http_error(&self.scope,error,RegistryRefusalSite::HttpConnection,"source HTTP connection failed")))}
                        }
                    }
                } else {frame.await};
                Ok(result.transpose().map_err(|error|http_error(&self.scope,error,RegistryRefusalSite::HttpBody,"source HTTP body failed")))
            }).await.map_err(SourceTransportFailure::scope)??;
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
    pub(in crate::runtime) async fn dispose_typed(mut self) -> Result<(), SourceTransportFailure> {
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
            .map_err(SourceTransportFailure::scope)?;
        Ok(())
    }
    pub(in crate::runtime) async fn next_data(&mut self) -> Result<Option<Bytes>, String> {
        self.next_data_typed()
            .await
            .map_err(SourceTransportFailure::into_message)
    }
    pub(in crate::runtime) async fn dispose(self) -> Result<(), String> {
        self.dispose_typed()
            .await
            .map_err(SourceTransportFailure::into_message)
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

pub(in crate::runtime) async fn request_scoped_typed(
    scope: &Arc<SourceServiceScope>,
    url: &url::Url,
    admission: SourceHttpAdmission,
) -> Result<ScopedHttpResponse, SourceTransportFailure> {
    scope.check().map_err(SourceTransportFailure::scope)?;
    if url.as_str().len() > 8192 {
        return Err(SourceTransportFailure::scope(
            scope.poison(SourceServiceError::BufferLimit),
        ));
    }
    validate_address(url, admission).map_err(|message| {
        SourceTransportFailure::opaque(RegistryRefusalSite::AddressValidation, message)
    })?;
    let host = url.host_str().ok_or_else(|| {
        SourceTransportFailure::opaque(
            RegistryRefusalSite::AddressValidation,
            "source HTTP host missing",
        )
    })?;
    let port = url.port_or_known_default().ok_or_else(|| {
        SourceTransportFailure::opaque(
            RegistryRefusalSite::AddressValidation,
            "source HTTP port missing",
        )
    })?;
    let credit = scope
        .reserve_buffer(64 * 1024 + 128 * 256 + 32 * 1024)
        .map_err(SourceTransportFailure::scope)?;
    let addresses = scope
        .during(Duration::from_secs(15), async {
            Ok(tokio::net::lookup_host((host, port)).await.map_err(|e| {
                SourceTransportFailure::io(RegistryRefusalSite::Dns, &e, "source DNS lookup failed")
            }))
        })
        .await
        .map_err(SourceTransportFailure::scope)??;
    // No unbounded collect. DNS implementation itself is inherited and is
    // supervised externally; cancellation cannot guarantee OS resolver stop.
    let bounded = bounded_addresses_typed(scope, addresses)?;
    let mut connected = None;
    let mut last_failure = None;
    for address in bounded {
        let result = scope
            .during(Duration::from_secs(15), async {
                Ok(TcpStream::connect(address).await)
            })
            .await
            .map_err(SourceTransportFailure::scope)?;
        match result {
            Ok(stream) => {
                connected = Some(stream);
                break;
            }
            Err(error) => {
                last_failure = Some(SourceTransportFailure::io(
                    RegistryRefusalSite::ConnectExhausted,
                    &error,
                    "source TCP connection failed",
                ))
            }
        }
    }
    let stream = connected.ok_or_else(|| {
        last_failure.unwrap_or_else(|| {
            SourceTransportFailure::opaque(
                RegistryRefusalSite::ConnectExhausted,
                "source TCP connection failed",
            )
        })
    })?;
    let io: OwnedIo = if url.scheme() == "https" {
        scope.check().map_err(SourceTransportFailure::scope)?;
        // Native root discovery is an inherited synchronous asset boundary.
        let roots = rustls_native_certs::load_native_certs();
        let mut store = rustls::RootCertStore::empty();
        for certificate in roots.certs {
            store.add(certificate).map_err(|_| {
                SourceTransportFailure::opaque(
                    RegistryRefusalSite::TlsRootRejected,
                    "source TLS root rejected",
                )
            })?;
        }
        if store.is_empty() {
            return Err(SourceTransportFailure::opaque(
                RegistryRefusalSite::TlsRootsUnavailable,
                "source TLS roots unavailable",
            ));
        }
        let config = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|_| {
            SourceTransportFailure::opaque(
                RegistryRefusalSite::TlsProtocol,
                "source TLS protocol refused",
            )
        })?
        .with_root_certificates(store)
        .with_no_client_auth();
        scope.check().map_err(SourceTransportFailure::scope)?;
        let name = rustls::pki_types::ServerName::try_from(host.to_owned()).map_err(|_| {
            SourceTransportFailure::opaque(RegistryRefusalSite::TlsName, "source TLS name refused")
        })?;
        let stream = scope
            .during(Duration::from_secs(15), async {
                Ok(TlsConnector::from(Arc::new(config))
                    .connect(name, stream)
                    .await
                    .map_err(|e| {
                        SourceTransportFailure::io(
                            RegistryRefusalSite::TlsHandshake,
                            &e,
                            "source TLS handshake failed",
                        )
                    }))
            })
            .await
            .map_err(SourceTransportFailure::scope)??;
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
) -> Result<ScopedHttpResponse, SourceTransportFailure> {
    let mut builder = http1::Builder::new();
    builder.max_buf_size(64 * 1024).max_headers(128);
    let (mut sender, connection) = scope
        .during(Duration::from_secs(15), async {
            Ok(builder.handshake(TokioIo::new(io)).await.map_err(|error| {
                http_error(
                    scope,
                    error,
                    RegistryRefusalSite::HttpHandshake,
                    "source HTTP handshake failed",
                )
            }))
        })
        .await
        .map_err(SourceTransportFailure::scope)??;
    let uri = url.as_str().parse::<hyper::Uri>().map_err(|_| {
        SourceTransportFailure::opaque(RegistryRefusalSite::HttpRequest, "source HTTP URI refused")
    })?;
    let authority = uri
        .authority()
        .ok_or_else(|| {
            SourceTransportFailure::opaque(
                RegistryRefusalSite::HttpRequest,
                "source HTTP authority missing",
            )
        })?
        .as_str();
    let request = hyper::Request::get(uri.path_and_query().map_or("/", |v| v.as_str()))
        .header(hyper::header::HOST, authority)
        .body(Empty::<Bytes>::new())
        .map_err(|_| {
            SourceTransportFailure::opaque(
                RegistryRefusalSite::HttpRequest,
                "source HTTP request refused",
            )
        })?;
    let mut connection = Some(Box::pin(connection));
    let response=scope.during(Duration::from_secs(30),async {
        let request=sender.send_request(request);tokio::pin!(request);
        let result=tokio::select! {
            result=&mut request=>result,
            result=connection.as_mut().unwrap().as_mut()=>{
                connection=None;
                match result {Ok(())=>request.await,Err(error)=>return Ok(Err(http_error(scope,error,RegistryRefusalSite::HttpConnection,"source HTTP connection failed")))}
            }
        };
        Ok(result.map_err(|error|http_error(scope,error,RegistryRefusalSite::HttpHeaders,"source HTTP headers failed")))
    }).await.map_err(SourceTransportFailure::scope)??;
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
pub(super) async fn request_test_io_typed(
    scope: &Arc<SourceServiceScope>,
    url: &url::Url,
    io: tokio::io::DuplexStream,
) -> Result<ScopedHttpResponse, SourceTransportFailure> {
    let credit = scope
        .reserve_buffer(64 * 1024 + 128 * 256 + 32 * 1024)
        .map_err(SourceTransportFailure::scope)?;
    request_on_io(scope, url, Box::new(io), credit).await
}

pub(in crate::runtime) async fn request_scoped(
    scope: &Arc<SourceServiceScope>,
    url: &url::Url,
    admission: SourceHttpAdmission,
) -> Result<ScopedHttpResponse, String> {
    request_scoped_typed(scope, url, admission)
        .await
        .map_err(SourceTransportFailure::into_message)
}
#[cfg(test)]
pub(super) async fn request_test_io(
    scope: &Arc<SourceServiceScope>,
    url: &url::Url,
    io: tokio::io::DuplexStream,
) -> Result<ScopedHttpResponse, String> {
    request_test_io_typed(scope, url, io)
        .await
        .map_err(SourceTransportFailure::into_message)
}

#[cfg(test)]
mod refusal_tests {
    use slug_bzlmod_v2::RegistryRefusalDetail;
    use slug_bzlmod_v2::RegistryRefusalProtocolFailure;
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    use super::*;

    #[tokio::test]
    async fn parse_projection_is_bounded_before_scope_postcheck_selects_poison() {
        let work = std::time::Instant::now() + Duration::from_secs(2);
        let scope = SourceServiceScope::new(
            slug_workspace_v2::NormalizedAbsolutePath::new("/parse-projection").unwrap(),
            work,
            work + Duration::from_secs(1),
            super::super::SourceServiceLimits::default(),
        )
        .unwrap();
        let (client, mut server) = tokio::io::duplex(1024);
        let consumer = async {
            let (mut sender, connection) =
                http1::handshake::<_, Empty<Bytes>>(TokioIo::new(client))
                    .await
                    .unwrap();
            let request = sender.send_request(
                hyper::Request::get("/")
                    .body(Empty::<Bytes>::new())
                    .unwrap(),
            );
            tokio::pin!(request);
            tokio::pin!(connection);
            tokio::select! {
                result = &mut request => result.unwrap_err(),
                result = &mut connection => {
                    match result { Err(error) => error, Ok(()) => request.await.unwrap_err() }
                }
            }
        };
        let writer = async {
            let mut request = [0; 1024];
            server.read(&mut request).await.unwrap();
            let _ = server.write_all(b"NOT-HTTP\r\n\r\n").await;
        };
        let error = tokio::join!(consumer, writer).0;
        let failure = http_error(
            &scope,
            error,
            RegistryRefusalSite::HttpHeaders,
            "source HTTP headers failed",
        );
        assert_eq!(failure.cause().site, RegistryRefusalSite::HttpHeaders);
        assert_eq!(
            failure.cause().detail,
            RegistryRefusalDetail::Protocol(RegistryRefusalProtocolFailure::Parse)
        );
        assert_eq!(
            failure.into_message(),
            SourceServiceError::UnsupportedSourceOwner.to_string()
        );
        assert_eq!(
            scope.check(),
            Err(SourceServiceError::UnsupportedSourceOwner)
        );
    }
}
