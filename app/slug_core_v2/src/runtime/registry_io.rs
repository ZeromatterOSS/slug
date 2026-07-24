/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use dice::DiceDataBuilder;
use http_body_util::BodyExt;
use http_body_util::Empty;
use hyper::Uri;
use hyper_rustls::HttpsConnector;
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use slug_bzlmod_v2::RegistryFileUrl;
use slug_bzlmod_v2::RegistryIo;
use slug_bzlmod_v2::RegistryIoOutcome;
use slug_bzlmod_v2::RegistryTransportError;
use slug_bzlmod_v2::install_registry_io;

type HyperClient = Client<HttpsConnector<HttpConnector>, Empty<Bytes>>;

enum HyperRegistryIo {
    Ready(HyperClient),
    InitializationFailed(Arc<str>),
}

impl HyperRegistryIo {
    fn new() -> Self {
        let connector = match HttpsConnectorBuilder::new().with_native_roots() {
            Ok(builder) => builder
                .https_or_http()
                .enable_http1()
                .enable_http2()
                .build(),
            Err(error) => {
                return Self::InitializationFailed(Arc::from(format!(
                    "loading native TLS roots for registry HTTP client: {error}"
                )));
            }
        };
        Self::Ready(Client::builder(TokioExecutor::new()).build(connector))
    }
}

#[async_trait]
impl RegistryIo for HyperRegistryIo {
    async fn read_exact(
        &self,
        url: &RegistryFileUrl,
    ) -> Result<RegistryIoOutcome, RegistryTransportError> {
        let client = match self {
            Self::Ready(client) => client,
            Self::InitializationFailed(message) => {
                return Err(RegistryTransportError {
                    message: message.as_ref().into(),
                });
            }
        };
        let uri = url
            .as_str()
            .parse::<Uri>()
            .map_err(|error| RegistryTransportError {
                message: format!("invalid registry URL {}: {error}", url.as_str()).into(),
            })?;
        let response = client
            .get(uri)
            .await
            .map_err(|error| RegistryTransportError {
                message: format!("registry GET {} failed: {error}", url.as_str()).into(),
            })?;
        let status = response.status();
        if status == hyper::StatusCode::NOT_FOUND {
            return Ok(RegistryIoOutcome::NotFound);
        }
        if !status.is_success() {
            return Err(RegistryTransportError {
                message: format!("registry GET {} returned HTTP {status}", url.as_str()).into(),
            });
        }
        let body = response
            .into_body()
            .collect()
            .await
            .map_err(|error| RegistryTransportError {
                message: format!(
                    "reading registry response body from {}: {error}",
                    url.as_str()
                )
                .into(),
            })?
            .to_bytes();
        Ok(RegistryIoOutcome::Found(Arc::from(body.as_ref())))
    }
}

pub(crate) fn install(builder: &mut DiceDataBuilder) {
    install_registry_io(builder, Arc::new(HyperRegistryIo::new()));
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    use super::*;

    async fn serve_once(
        response: &'static [u8],
    ) -> (RegistryFileUrl, tokio::task::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let request = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0; 4096];
            let length = stream.read(&mut request).await.unwrap();
            request.truncate(length);
            stream.write_all(response).await.unwrap();
            request
        });
        (
            RegistryFileUrl::new(format!("http://{address}/exact/path?query=yes")),
            request,
        )
    }

    #[tokio::test]
    async fn exact_http_adapter_distinguishes_success_not_found_and_other_statuses() {
        let io = HyperRegistryIo::new();

        let (url, request) = serve_once(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nbody").await;
        assert_eq!(
            io.read_exact(&url).await.unwrap(),
            RegistryIoOutcome::Found(Arc::from(&b"body"[..]))
        );
        let request = String::from_utf8(request.await.unwrap()).unwrap();
        assert!(request.starts_with("GET /exact/path?query=yes HTTP/1.1\r\n"));

        let (url, _) = serve_once(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n").await;
        assert_eq!(
            io.read_exact(&url).await.unwrap(),
            RegistryIoOutcome::NotFound
        );

        let (url, _) =
            serve_once(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n").await;
        let error = io.read_exact(&url).await.unwrap_err();
        assert!(error.message.contains("503 Service Unavailable"));

        let unavailable = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = unavailable.local_addr().unwrap();
        drop(unavailable);
        let error = io
            .read_exact(&RegistryFileUrl::new(format!(
                "http://{address}/connection-refused"
            )))
            .await
            .unwrap_err();
        assert!(error.message.contains("registry GET"));
    }
}
