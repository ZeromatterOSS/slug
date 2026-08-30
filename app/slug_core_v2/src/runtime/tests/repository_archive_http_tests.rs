use std::collections::HashMap;
use std::future::pending;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use super::super::repository_archive::SelectedBcrArchiveFormat;
use super::super::repository_io::ArchiveFailureStage;
use super::*;

#[derive(Clone)]
struct Reply {
    head: String,
    head_delay: Duration,
    chunks: Vec<(Duration, Vec<u8>)>,
    hold: Duration,
}

impl Reply {
    fn fixed(status: u16, body: &[u8]) -> Self {
        Self {
            head: format!(
                "HTTP/1.1 {status} test\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n",
                body.len()
            ),
            head_delay: Duration::ZERO,
            chunks: vec![(Duration::ZERO, body.to_vec())],
            hold: Duration::ZERO,
        }
    }

    fn redirect(status: u16, location: &str) -> Self {
        Self {
            head: format!(
                "HTTP/1.1 {status} redirect\r\nLocation: {location}\r\nContent-Length: 0\r\n\r\n"
            ),
            head_delay: Duration::ZERO,
            chunks: Vec::new(),
            hold: Duration::ZERO,
        }
    }
}

struct Server {
    address: SocketAddr,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Server {
    fn new(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();
        let thread = thread::spawn(move || {
            for reply in replies {
                let (mut stream, _) = listener.accept().unwrap();
                if stop_thread.load(Ordering::SeqCst) {
                    break;
                }
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 512];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    let count = stream.read(&mut buffer).unwrap();
                    if count == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..count]);
                }
                recorded
                    .lock()
                    .unwrap()
                    .push(String::from_utf8(request).unwrap());
                thread::sleep(reply.head_delay);
                if stream.write_all(reply.head.as_bytes()).is_err() {
                    continue;
                }
                stream.flush().unwrap();
                for (delay, chunk) in reply.chunks {
                    thread::sleep(delay);
                    if stream.write_all(&chunk).is_err() {
                        break;
                    }
                    let _ = stream.flush();
                }
                thread::sleep(reply.hold);
            }
        });
        Self {
            address,
            requests,
            stop,
            thread: Some(thread),
        }
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            self.stop.store(true, Ordering::SeqCst);
            let _ = std::net::TcpStream::connect(self.address);
            thread.join().unwrap();
        }
    }
}

struct TestEnvironment {
    addresses: HashMap<String, Vec<SocketAddr>>,
    resolutions: Mutex<Vec<String>>,
    connections: Mutex<Vec<(String, SocketAddr)>>,
    captures: Mutex<Vec<PathBuf>>,
    connect_errors: HashMap<String, String>,
    connect_stalls: Vec<String>,
    shutdown_stalls: Vec<String>,
    limits: Limits,
}

impl TestEnvironment {
    fn new(entries: impl IntoIterator<Item = (String, Vec<SocketAddr>)>) -> Self {
        Self {
            addresses: entries.into_iter().collect(),
            resolutions: Mutex::new(Vec::new()),
            connections: Mutex::new(Vec::new()),
            captures: Mutex::new(Vec::new()),
            connect_errors: HashMap::new(),
            connect_stalls: Vec::new(),
            shutdown_stalls: Vec::new(),
            limits: Limits {
                capture_bytes: 1024,
                connect: Duration::from_millis(40),
                header: Duration::from_millis(80),
                frame: Duration::from_millis(40),
                disposal: Duration::from_millis(40),
            },
        }
    }

    fn captures(&self) -> Vec<PathBuf> {
        self.captures.lock().unwrap().clone()
    }
}

impl Environment for TestEnvironment {
    fn limits(&self) -> Limits {
        self.limits
    }

    fn resolve(&self, url: &url::Url) -> Result<Vec<SocketAddr>, String> {
        let host = url.host_str().unwrap().to_owned();
        self.resolutions.lock().unwrap().push(host.clone());
        self.addresses
            .get(&host)
            .cloned()
            .ok_or_else(|| format!("no scripted address for {host}"))
    }

    fn connect(
        &self,
        runtime: &tokio::runtime::Runtime,
        url: &url::Url,
        address: SocketAddr,
    ) -> Result<ArchiveStream, String> {
        let host = url.host_str().unwrap().to_owned();
        self.connections
            .lock()
            .unwrap()
            .push((host.clone(), address));
        if let Some(error) = self.connect_errors.get(&host) {
            return Err(error.clone());
        }
        if self.connect_stalls.contains(&host) {
            return runtime
                .block_on(async {
                    timeout(
                        self.limits.connect,
                        pending::<Result<ArchiveStream, String>>(),
                    )
                    .await
                })
                .map_err(|_| "TCP connect timed out".to_owned())?;
        }
        let stream = runtime
            .block_on(async { timeout(self.limits.connect, TcpStream::connect(address)).await })
            .map_err(|_| "TCP connect timed out".to_owned())?
            .map_err(|error| format!("TCP connect: {error}"))?;
        if self.shutdown_stalls.contains(&host) {
            Ok(ArchiveStream::StalledShutdown(StalledShutdown(stream)))
        } else {
            Ok(ArchiveStream::Plain(stream))
        }
    }

    fn capture(&self) -> Result<tempfile::NamedTempFile, String> {
        let capture = tempfile::NamedTempFile::new().unwrap();
        self.captures
            .lock()
            .unwrap()
            .push(capture.path().to_path_buf());
        Ok(capture)
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn plan(urls: Vec<String>, bytes: &[u8]) -> SelectedBcrArchive {
    SelectedBcrArchive {
        format: SelectedBcrArchiveFormat::TarGz,
        urls: urls.into_boxed_slice(),
        integrity: Sha256::digest(bytes).into(),
        strip_prefix: None,
        patches: Box::new([]),
        overlays: Box::new([]),
        patch_strip: 0,
        module_url: "https://registry.test/MODULE.bazel".into(),
        module_integrity: [0; 32],
    }
}

fn url(host: &str, server: &Server, path: &str) -> String {
    format!("https://{host}:{}{path}", server.address.port())
}

fn assert_captures_deleted(environment: &TestEnvironment) {
    let captures = environment.captures();
    assert!(!captures.is_empty());
    assert!(captures.iter().all(|path| !path.exists()), "{captures:?}");
}

#[test]
fn streams_host_origin_query_and_deletes_verified_capture() {
    let body = b"two streamed frames";
    let server = Server::new(vec![Reply {
        head: "HTTP/1.1 200 ok\r\nTransfer-Encoding: chunked\r\n\r\n".into(),
        head_delay: Duration::ZERO,
        chunks: vec![
            (Duration::ZERO, b"4\r\ntwo \r\n".to_vec()),
            (
                Duration::from_millis(5),
                b"F\r\nstreamed frames\r\n0\r\n\r\n".to_vec(),
            ),
        ],
        hold: Duration::ZERO,
    }]);
    let environment = TestEnvironment::new([("mirror.test".into(), vec![server.address])]);
    capture_selected_bcr_with(
        &plan(vec![url("mirror.test", &server, "/archive?q=one")], body),
        &runtime(),
        &|| true,
        &environment,
    )
    .unwrap();
    let request = server.requests().pop().unwrap();
    assert!(request.starts_with("GET /archive?q=one HTTP/1.1\r\n"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains(&format!("host: mirror.test:{}\r\n", server.address.port()))
    );
    assert_captures_deleted(&environment);
}

#[test]
fn first_verified_mirror_stops_before_later_resolution_connection_or_capture() {
    let body = b"first";
    let first = Server::new(vec![Reply::fixed(200, body)]);
    let second = Server::new(vec![Reply::fixed(200, body)]);
    let environment = TestEnvironment::new([
        ("first.test".into(), vec![first.address]),
        ("second.test".into(), vec![second.address]),
    ]);
    capture_selected_bcr_with(
        &plan(
            vec![
                url("first.test", &first, "/archive"),
                url("second.test", &second, "/archive"),
            ],
            body,
        ),
        &runtime(),
        &|| true,
        &environment,
    )
    .unwrap();
    assert_eq!(
        environment.resolutions.lock().unwrap().as_slice(),
        ["first.test"]
    );
    assert_eq!(environment.connections.lock().unwrap().len(), 1);
    assert_eq!(environment.captures().len(), 1);
    assert_eq!(first.requests().len(), 1);
    assert!(second.requests().is_empty());
    assert_captures_deleted(&environment);
}

#[test]
fn accepts_206_and_tries_addresses_in_resolver_order() {
    let body = b"partial";
    let server = Server::new(vec![Reply::fixed(206, body)]);
    let closed = TcpListener::bind("127.0.0.1:0").unwrap();
    let closed_address = closed.local_addr().unwrap();
    drop(closed);
    let environment =
        TestEnvironment::new([("mirror.test".into(), vec![closed_address, server.address])]);
    capture_selected_bcr_with(
        &plan(vec![url("mirror.test", &server, "/partial")], body),
        &runtime(),
        &|| true,
        &environment,
    )
    .unwrap();
    let connected = environment.connections.lock().unwrap();
    assert_eq!(connected[0].1, closed_address);
    assert_eq!(connected[1].1, server.address);
}

#[test]
fn exact_mirror_fallback_cases_select_the_second_source() {
    for first in ["404", "tls", "body-timeout", "sri"] {
        let expected = b"verified";
        let second = Server::new(vec![Reply::fixed(200, expected)]);
        let mut entries = vec![("second.test".into(), vec![second.address])];
        let mut first_server = None;
        let mut environment;
        let first_url = if first == "tls" {
            entries.push(("first.test".into(), vec![second.address]));
            environment = TestEnvironment::new(entries);
            environment
                .connect_errors
                .insert("first.test".into(), "TLS certificate rejected".into());
            format!("https://first.test:{}/archive", second.address.port())
        } else {
            let reply = match first {
                "404" => Reply::fixed(404, b"missing"),
                "body-timeout" => Reply {
                    head: "HTTP/1.1 200 ok\r\nContent-Length: 8\r\n\r\n".into(),
                    head_delay: Duration::ZERO,
                    chunks: Vec::new(),
                    hold: Duration::from_millis(80),
                },
                "sri" => Reply::fixed(200, b"wrong"),
                _ => unreachable!(),
            };
            first_server = Some(Server::new(vec![reply]));
            let server = first_server.as_ref().unwrap();
            entries.push(("first.test".into(), vec![server.address]));
            environment = TestEnvironment::new(entries);
            url("first.test", server, "/archive")
        };
        capture_selected_bcr_with(
            &plan(
                vec![first_url, url("second.test", &second, "/archive")],
                expected,
            ),
            &runtime(),
            &|| true,
            &environment,
        )
        .unwrap_or_else(|error| panic!("{first}: {error:?}"));
        assert_eq!(second.requests().len(), 1, "{first}");
        assert_eq!(environment.captures().len(), 2, "{first}");
        assert_captures_deleted(&environment);
        drop(first_server);
    }
}

#[test]
fn follows_only_admitted_redirects_and_resolves_each_hop() {
    for status in [301, 302, 303, 307] {
        let body = b"redirected";
        let server = Server::new(vec![
            Reply::redirect(status, "/final"),
            Reply::fixed(200, body),
        ]);
        let environment = TestEnvironment::new([("redirect.test".into(), vec![server.address])]);
        capture_selected_bcr_with(
            &plan(vec![url("redirect.test", &server, "/start")], body),
            &runtime(),
            &|| true,
            &environment,
        )
        .unwrap();
        assert_eq!(environment.resolutions.lock().unwrap().len(), 2);
        assert_eq!(server.requests().len(), 2);
    }

    let body = b"absolute";
    let destination = Server::new(vec![Reply::fixed(200, body)]);
    let source = Server::new(vec![Reply::redirect(
        301,
        &url("destination.test", &destination, "/final"),
    )]);
    let environment = TestEnvironment::new([
        ("source.test".into(), vec![source.address]),
        ("destination.test".into(), vec![destination.address]),
    ]);
    capture_selected_bcr_with(
        &plan(vec![url("source.test", &source, "/start")], body),
        &runtime(),
        &|| true,
        &environment,
    )
    .unwrap();
    assert_eq!(destination.requests().len(), 1);
}

#[test]
fn rejects_308_downgrade_and_the_fortieth_redirect() {
    for location in ["/not-followed", "http://other.test/insecure"] {
        let body = b"fallback";
        let second = Server::new(vec![Reply::fixed(200, body)]);
        let first = Server::new(vec![Reply::redirect(
            if location.starts_with("http:") {
                301
            } else {
                308
            },
            location,
        )]);
        let environment = TestEnvironment::new([
            ("first.test".into(), vec![first.address]),
            ("second.test".into(), vec![second.address]),
        ]);
        capture_selected_bcr_with(
            &plan(
                vec![
                    url("first.test", &first, "/archive"),
                    url("second.test", &second, "/archive"),
                ],
                body,
            ),
            &runtime(),
            &|| true,
            &environment,
        )
        .unwrap();
        assert_eq!(first.requests().len(), 1);
        assert_eq!(second.requests().len(), 1);
    }

    let body = b"after-loop";
    let second = Server::new(vec![Reply::fixed(200, body)]);
    let first = Server::new((0..40).map(|_| Reply::redirect(301, "/again")).collect());
    let environment = TestEnvironment::new([
        ("loop.test".into(), vec![first.address]),
        ("second.test".into(), vec![second.address]),
    ]);
    capture_selected_bcr_with(
        &plan(
            vec![
                url("loop.test", &first, "/again"),
                url("second.test", &second, "/archive"),
            ],
            body,
        ),
        &runtime(),
        &|| true,
        &environment,
    )
    .unwrap();
    assert_eq!(first.requests().len(), 40);
}

#[test]
fn enforces_declared_streamed_and_lifecycle_ceilings() {
    let environment = TestEnvironment::new([(
        "too-many.test".into(),
        vec!["127.0.0.1:1".parse().unwrap(); MAX_ADDRESSES + 1],
    )]);
    let error = capture_selected_bcr_with(
        &plan(vec!["https://too-many.test/archive".into()], b"unused"),
        &runtime(),
        &|| true,
        &environment,
    )
    .unwrap_err();
    assert!(error.message.contains("more than 64 addresses"));
    assert!(environment.connections.lock().unwrap().is_empty());
    assert_captures_deleted(&environment);

    let cases = [
        Reply {
            head: "HTTP/1.1 200 ok\r\nContent-Length: 5\r\n\r\n".into(),
            head_delay: Duration::ZERO,
            chunks: vec![(Duration::ZERO, b"12345".to_vec())],
            hold: Duration::ZERO,
        },
        Reply {
            head: "HTTP/1.1 200 ok\r\nTransfer-Encoding: chunked\r\n\r\n".into(),
            head_delay: Duration::ZERO,
            chunks: vec![(Duration::ZERO, b"5\r\n12345\r\n0\r\n\r\n".to_vec())],
            hold: Duration::ZERO,
        },
    ];
    for reply in cases {
        let server = Server::new(vec![reply]);
        let mut environment = TestEnvironment::new([("limit.test".into(), vec![server.address])]);
        environment.limits.capture_bytes = 4;
        let error = capture_selected_bcr_with(
            &plan(vec![url("limit.test", &server, "/archive")], b"12345"),
            &runtime(),
            &|| true,
            &environment,
        )
        .unwrap_err();
        assert!(error.message.contains("capture limit"));
        assert_captures_deleted(&environment);
    }

    let server = Server::new(vec![Reply::fixed(200, b"done")]);
    let mut environment = TestEnvironment::new([("stall.test".into(), vec![server.address])]);
    environment.shutdown_stalls.push("stall.test".into());
    let error = capture_selected_bcr_with(
        &plan(vec![url("stall.test", &server, "/archive")], b"done"),
        &runtime(),
        &|| true,
        &environment,
    )
    .unwrap_err();
    assert!(error.message.contains("disposal timed out"));
    assert_captures_deleted(&environment);

    let mut reply = Reply::fixed(200, b"peer-held-open");
    reply.hold = Duration::from_millis(300);
    let server = Server::new(vec![reply]);
    let mut environment = TestEnvironment::new([("held-open.test".into(), vec![server.address])]);
    environment.limits.disposal = Duration::from_millis(50);
    let started = Instant::now();
    capture_selected_bcr_with(
        &plan(
            vec![url("held-open.test", &server, "/archive")],
            b"peer-held-open",
        ),
        &runtime(),
        &|| true,
        &environment,
    )
    .unwrap();
    assert!(started.elapsed() < Duration::from_millis(200));
    assert_captures_deleted(&environment);

    let server = Server::new(vec![Reply {
        head: "HTTP/1.1 200 ok\r\nContent-Length: 0\r\n\r\n".into(),
        head_delay: Duration::from_millis(100),
        chunks: Vec::new(),
        hold: Duration::ZERO,
    }]);
    let mut environment =
        TestEnvironment::new([("header-stall.test".into(), vec![server.address])]);
    environment.limits.header = Duration::from_millis(40);
    let error = capture_selected_bcr_with(
        &plan(vec![url("header-stall.test", &server, "/archive")], b""),
        &runtime(),
        &|| true,
        &environment,
    )
    .unwrap_err();
    assert!(error.message.contains("header timed out"));
    assert_captures_deleted(&environment);

    let mut environment = TestEnvironment::new([(
        "connect-stall.test".into(),
        vec!["127.0.0.1:1".parse().unwrap()],
    )]);
    environment.connect_stalls.push("connect-stall.test".into());
    let error = capture_selected_bcr_with(
        &plan(vec!["https://connect-stall.test/archive".into()], b"unused"),
        &runtime(),
        &|| true,
        &environment,
    )
    .unwrap_err();
    assert!(error.message.contains("connect timed out"));
}

#[test]
fn inactive_session_stops_between_transport_steps_and_cleans_capture() {
    let first = Server::new(vec![Reply::fixed(200, b"body")]);
    let second = Server::new(vec![Reply::fixed(200, b"body")]);
    let environment = TestEnvironment::new([
        ("first.test".into(), vec![first.address]),
        ("second.test".into(), vec![second.address]),
    ]);
    let probes = AtomicUsize::new(0);
    let error = capture_selected_bcr_with(
        &plan(
            vec![
                url("first.test", &first, "/archive"),
                url("second.test", &second, "/archive"),
            ],
            b"body",
        ),
        &runtime(),
        &|| probes.fetch_add(1, Ordering::SeqCst) < 3,
        &environment,
    )
    .unwrap_err();
    assert!(error.message.contains("no longer active"));
    assert_eq!(
        environment.resolutions.lock().unwrap().as_slice(),
        ["first.test"]
    );
    assert_eq!(environment.connections.lock().unwrap().len(), 1);
    assert_eq!(environment.captures().len(), 1);
    assert_eq!(first.requests().len(), 1);
    assert!(second.requests().is_empty());
    assert_captures_deleted(&environment);
}

#[test]
fn production_owner_has_no_task_client_global_provider_or_full_body_path() {
    let source = include_str!("../repository_archive_http.rs");
    for forbidden in [
        "spawn(",
        "spawn_blocking",
        "GaiResolver",
        "Client::",
        "TokioExecutor",
        "install_default",
        "tokio::fs",
        "collect_body",
        "Vec<u8>",
        "create_root",
        "extract",
    ] {
        assert!(!source.contains(forbidden), "forbidden {forbidden}");
    }
    assert!(source.contains("http1::handshake"));
    assert!(source.contains("ring::default_provider"));
    assert!(source.contains("Result<tempfile::NamedTempFile"));
}

#[test]
fn module_capture_uses_its_own_sri_limit_and_transport_stage() {
    let body = b"registry module bytes";
    let server = Server::new(vec![Reply::fixed(200, body)]);
    let environment = TestEnvironment::new([("registry.test".into(), vec![server.address])]);
    let module_url = url("registry.test", &server, "/MODULE.bazel");
    let mut capture = capture_urls(
        std::slice::from_ref(&module_url),
        Sha256::digest(body).into(),
        MODULE_CAPTURE_LIMIT,
        "MODULE",
        &runtime(),
        &|| true,
        &environment,
    )
    .unwrap();
    let mut captured = Vec::new();
    capture.as_file_mut().seek(SeekFrom::Start(0)).unwrap();
    capture.as_file_mut().read_to_end(&mut captured).unwrap();
    assert_eq!(captured, body);
    drop(capture);
    assert_captures_deleted(&environment);

    let bad = Server::new(vec![Reply::fixed(200, body)]);
    let environment = TestEnvironment::new([("bad.test".into(), vec![bad.address])]);
    let error = capture_urls(
        &[url("bad.test", &bad, "/MODULE.bazel")],
        [0; 32],
        MODULE_CAPTURE_LIMIT,
        "MODULE",
        &runtime(),
        &|| true,
        &environment,
    )
    .unwrap_err();
    assert_eq!(error.stage, ArchiveFailureStage::Transport);
    assert!(error.message.contains("MODULE capture failed"));
    assert_captures_deleted(&environment);
}
