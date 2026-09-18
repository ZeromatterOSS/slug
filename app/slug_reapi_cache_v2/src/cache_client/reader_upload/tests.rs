use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;

use tokio::io::ReadBuf;

use super::*;

#[derive(Default)]
struct State {
    bytes: AtomicUsize,
    largest_read: AtomicUsize,
    dropped: AtomicBool,
}

struct Reader {
    data: &'static [u8],
    state: Arc<State>,
    step: usize,
    interrupted: bool,
    error_at_end: bool,
    pending: bool,
}

impl Reader {
    fn new(data: &'static [u8]) -> Self {
        Self {
            data,
            state: Arc::default(),
            step: usize::MAX,
            interrupted: false,
            error_at_end: false,
            pending: false,
        }
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        self.state.dropped.store(true, Ordering::SeqCst);
    }
}

impl AsyncRead for Reader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.state
            .largest_read
            .fetch_max(buf.remaining(), Ordering::SeqCst);
        if self.pending {
            return Poll::Pending;
        }
        if std::mem::take(&mut self.interrupted) {
            return Poll::Ready(Err(io::ErrorKind::Interrupted.into()));
        }
        if self.data.is_empty() && self.error_at_end {
            return Poll::Ready(Err(io::Error::other("reader failed")));
        }
        let len = self.data.len().min(buf.remaining()).min(self.step);
        buf.put_slice(&self.data[..len]);
        self.data = &self.data[len..];
        self.state.bytes.fetch_add(len, Ordering::SeqCst);
        Poll::Ready(Ok(()))
    }
}

async fn record(
    reader: Reader,
    expected: &ReapiDigest,
    committed: i64,
) -> (Result<(), CacheError>, Vec<bytestream::WriteRequest>) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = requests.clone();
    let result = upload(
        reader,
        expected,
        "instance/uploads/id/blobs/digest".into(),
        2,
        |mut rx| async move {
            while let Some(request) = rx.recv().await {
                recorded.lock().unwrap().push(request);
            }
            Ok(bytestream::WriteResponse {
                committed_size: committed,
            })
        },
    )
    .await;
    let requests = Arc::try_unwrap(requests).unwrap().into_inner().unwrap();
    (result, requests)
}

#[tokio::test]
async fn verified_reader_offsets_eof_and_empty_finish_match_wire_contract() {
    for data in [b"abcde".as_slice(), b""] {
        let mut reader = Reader::new(data);
        reader.step = 1;
        reader.interrupted = true;
        let state = reader.state.clone();
        let (result, requests) =
            record(reader, &ReapiDigest::of_bytes(data), data.len() as i64).await;
        result.unwrap();
        assert!(state.dropped.load(Ordering::SeqCst));
        assert!(state.largest_read.load(Ordering::SeqCst) <= 2);
        let mut offset = 0;
        let mut received = Vec::new();
        for (index, request) in requests.iter().enumerate() {
            assert_eq!(request.write_offset, offset);
            assert_eq!(request.resource_name.is_empty(), index != 0);
            assert_eq!(request.finish_write, index + 1 == requests.len());
            assert!(request.data.len() <= 2);
            offset += request.data.len() as i64;
            received.extend_from_slice(&request.data);
        }
        assert_eq!(received, data);
        assert!(requests.last().unwrap().data.is_empty());
        assert_eq!(requests.last().unwrap().write_offset, data.len() as i64);
        assert_eq!(requests.len(), data.len().div_ceil(2) + 1);
    }
}

#[tokio::test]
async fn source_corruption_truncation_growth_and_read_failure_never_finish() {
    let expected = ReapiDigest::of_bytes(b"abcde");
    for (data, fail, message) in [
        (b"abcdf".as_slice(), false, "digest mismatch"),
        (b"abc", false, "digest mismatch"),
        (b"abcdefEXTRA", false, "exceeds declared"),
        (b"abcde", true, "reader failed"),
    ] {
        let mut reader = Reader::new(data);
        reader.error_at_end = fail;
        let state = reader.state.clone();
        let (result, requests) = record(reader, &expected, 5).await;
        assert!(result.unwrap_err().to_string().contains(message));
        assert!(requests.iter().all(|request| !request.finish_write));
        assert!(state.bytes.load(Ordering::SeqCst) <= 6);
        assert!(state.dropped.load(Ordering::SeqCst));
    }
}

#[tokio::test]
async fn wrong_committed_size_and_early_rpc_response_fail_closed() {
    for committed in [-1, 0, 4, 6] {
        let (result, _) = record(
            Reader::new(b"abcde"),
            &ReapiDigest::of_bytes(b"abcde"),
            committed,
        )
        .await;
        assert!(result.unwrap_err().to_string().contains("committed"));
    }
    for fail in [false, true] {
        let mut reader = Reader::new(b"abcde");
        reader.pending = true;
        let state = reader.state.clone();
        let digest = ReapiDigest::of_bytes(b"abcde");
        let mut future = Box::pin(upload(
            reader,
            &digest,
            "resource".into(),
            2,
            |rx| async move {
                let _receiver = rx;
                if fail {
                    Err(CacheError::Transport(tonic::Status::unavailable(
                        "wire failure",
                    )))
                } else {
                    Ok(bytestream::WriteResponse { committed_size: 5 })
                }
            },
        ));
        // Deterministic: an early terminal RPC must not wait on the blocked reader.
        let Poll::Ready(result) = futures::poll!(&mut future) else {
            panic!("reader was not cancelled")
        };
        assert!(result.unwrap_err().to_string().contains(if fail {
            "wire failure"
        } else {
            "before local"
        }));
        assert!(state.dropped.load(Ordering::SeqCst));
    }
}

#[tokio::test]
async fn stalled_rpc_backpressures_reads_and_cancellation_drops_reader() {
    let reader = Reader::new(b"a much longer source than the tiny chunk");
    let state = reader.state.clone();
    let digest = ReapiDigest::of_bytes(reader.data);
    let mut future = Box::pin(upload(
        reader,
        &digest,
        "resource".into(),
        2,
        |rx| async move {
            // Hold the receiver without consuming it, as a stalled transport would.
            let _receiver = rx;
            std::future::pending::<Result<bytestream::WriteResponse, CacheError>>().await
        },
    ));
    assert!(futures::poll!(&mut future).is_pending());
    assert_eq!(state.bytes.load(Ordering::SeqCst), 2);
    assert!(!state.dropped.load(Ordering::SeqCst));
    drop(future);
    assert!(state.dropped.load(Ordering::SeqCst));

    let mut reader = Reader::new(b"abc");
    reader.pending = true;
    let state = reader.state.clone();
    let mut future = Box::pin(upload(
        reader,
        &digest,
        "resource".into(),
        2,
        |rx| async move {
            let _receiver = rx;
            std::future::pending::<Result<bytestream::WriteResponse, CacheError>>().await
        },
    ));
    assert!(futures::poll!(&mut future).is_pending());
    drop(future);
    assert!(state.dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn source_failure_cancels_stalled_rpc() {
    struct DropFlag(Arc<AtomicBool>);
    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let dropped = Arc::new(AtomicBool::new(false));
    let flag = DropFlag(dropped.clone());
    let mut reader = Reader::new(b"");
    reader.error_at_end = true;
    let state = reader.state.clone();
    let result = upload(
        reader,
        &ReapiDigest::of_bytes(b""),
        "resource".into(),
        2,
        |rx| async move {
            let _flag = flag;
            let _receiver = rx;
            std::future::pending::<Result<bytestream::WriteResponse, CacheError>>().await
        },
    )
    .await;
    assert!(matches!(result, Err(CacheError::Source(_))));
    assert!(state.dropped.load(Ordering::SeqCst));
    assert!(dropped.load(Ordering::SeqCst));
}
