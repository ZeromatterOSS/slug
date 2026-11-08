//! Blocking reads own the ENTIRE native reader, not a naked Tokio File. Dropping
//! an RPC/adapter cannot retire a root/FD/chunk guard while blocking IO is alive.
use std::future::Future;
use std::io::Read;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use tokio::io::AsyncRead;
use tokio::io::ReadBuf;
use tokio::task::JoinHandle;

use super::*;

const CHUNK: usize = 64 * 1024;
struct Chunk {
    reader: NativeSourceReader,
    bytes: Vec<u8>,
    result: io::Result<usize>,
    position: usize,
    _credit: NativeReaderCredit,
}
enum State {
    Idle(Option<NativeSourceReader>),
    Reading(JoinHandle<Chunk>),
    Ready(Chunk),
    Closed,
}
pub struct NativeAsyncSourceReader {
    state: State,
}
impl NativeAsyncSourceReader {
    pub(super) fn new(reader: NativeSourceReader) -> Self {
        Self {
            state: State::Idle(Some(reader)),
        }
    }
}
impl AsyncRead for NativeAsyncSourceReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        out: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if out.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        loop {
            match std::mem::replace(&mut self.state, State::Closed) {
                State::Closed => {
                    return Poll::Ready(Err(io::Error::other("native source reader is closed")));
                }
                State::Idle(mut reader) => {
                    let reader = reader.take().expect("owned idle reader");
                    if let Err(error) = reader.check() {
                        return Poll::Ready(Err(error));
                    }
                    let credit = match reader.resources.reserve(
                        CHUNK + std::mem::size_of::<Chunk>() + 128,
                        0,
                        0,
                    ) {
                        Ok(c) => c,
                        Err(e) => {
                            return Poll::Ready(Err(io::Error::other(format!(
                                "native read chunk admission: {e:?}"
                            ))));
                        }
                    };
                    let mut bytes = Vec::new();
                    if bytes.try_reserve_exact(CHUNK).is_err() || bytes.capacity() > CHUNK {
                        return Poll::Ready(Err(io::Error::other("native read chunk allocation")));
                    }
                    bytes.resize(CHUNK, 0);
                    // Move one pre-armed owner into the queued job. Even a
                    // closure that never starts drops reader/bytes before credit.
                    let mut chunk = Chunk {
                        reader,
                        bytes,
                        result: Ok(0),
                        position: 0,
                        _credit: credit,
                    };
                    self.state = State::Reading(tokio::task::spawn_blocking(move || {
                        chunk.result = chunk.reader.read(&mut chunk.bytes);
                        chunk
                    }));
                }
                State::Reading(mut task) => match Pin::new(&mut task).poll(cx) {
                    Poll::Pending => {
                        self.state = State::Reading(task);
                        return Poll::Pending;
                    }
                    Poll::Ready(Ok(chunk)) => self.state = State::Ready(chunk),
                    Poll::Ready(Err(error)) => {
                        return Poll::Ready(Err(io::Error::other(format!(
                            "native read worker: {error}"
                        ))));
                    }
                },
                State::Ready(mut chunk) => {
                    if let Err(error) = chunk.reader.check() {
                        return Poll::Ready(Err(error));
                    }
                    let count = match &chunk.result {
                        Ok(count) => *count,
                        Err(_) => {
                            let Chunk { reader, result, .. } = chunk;
                            self.state = State::Idle(Some(reader));
                            return Poll::Ready(Err(result.err().unwrap()));
                        }
                    };
                    let available = count - chunk.position;
                    let copied = available.min(out.remaining());
                    out.put_slice(&chunk.bytes[chunk.position..chunk.position + copied]);
                    chunk.position += copied;
                    if chunk.position == count {
                        self.state = State::Idle(Some(chunk.reader));
                    } else {
                        self.state = State::Ready(chunk);
                    }
                    return Poll::Ready(Ok(()));
                }
            }
        }
    }
}
