//! Bounded actual-allocation XZ reader. No native IO/source authority is minted.
use std::io::Read;
use std::io::{self};
use std::sync::Arc;

use super::SourceDecoderReservation;
use super::SourceServiceError;
use super::SourceServiceScope;
use super::xz_allocator::AllocatorOwner;
use super::xz_allocator::Failure;

pub(in crate::runtime) const XZ_DECODER_MEMORY_LIMIT: u64 = 136 * 1024 * 1024;

pub(in crate::runtime) struct SourceXzDecoder<R: Read> {
    reader: R,
    stream: lzma_sys::lzma_stream,
    allocator: AllocatorOwner,
    scope: Option<Arc<SourceServiceScope>>,
    input: [u8; 8192],
    start: usize,
    end: usize,
    eof: bool,
    finished: bool,
    // Full logical allowance retires only after lzma_end and allocator state.
    _allowance: Option<SourceDecoderReservation>,
}
impl<R: Read> SourceXzDecoder<R> {
    pub(in crate::runtime) fn new(
        reader: R,
        scope: Option<&Arc<SourceServiceScope>>,
    ) -> io::Result<Self> {
        let allowance = scope
            .map(|s| s.reserve_xz_decoder_allowance(XZ_DECODER_MEMORY_LIMIT as usize))
            .transpose()
            .map_err(io::Error::other)?;
        let resources = scope.and_then(|s| s.command_resources()).cloned();
        let allocator = AllocatorOwner::new(
            resources,
            XZ_DECODER_MEMORY_LIMIT as usize,
            size_of::<Self>(),
        )
        .map_err(|f| publish(scope.map(Arc::as_ref), f))?;
        // SAFETY: documented LZMA_STREAM_INIT, also used by pinned xz2. The
        // allocator points to separately stable storage before initialization.
        let mut stream: lzma_sys::lzma_stream = unsafe { std::mem::zeroed() };
        stream.allocator = allocator.allocator();
        let mut decoder = Self {
            reader,
            stream,
            allocator,
            scope: scope.cloned(),
            input: [0; 8192],
            start: 0,
            end: 0,
            eof: false,
            finished: false,
            _allowance: allowance,
        };
        decoder.check()?;
        // SAFETY: stream and stable allocator are initialized; no callback
        // state or Context Rust borrow spans this single-threaded C call.
        let status = unsafe {
            lzma_sys::lzma_stream_decoder(
                &mut decoder.stream,
                XZ_DECODER_MEMORY_LIMIT,
                lzma_sys::LZMA_CONCATENATED,
            )
        };
        decoder.check()?;
        status_result(status)?;
        Ok(decoder)
    }
    fn check(&self) -> io::Result<()> {
        if let Some(failure) = self.allocator.failure() {
            return Err(publish(self.scope.as_deref(), failure));
        }
        if let Some(scope) = &self.scope {
            scope.check().map_err(io::Error::other)?;
        }
        Ok(())
    }
}
impl<R: Read> Read for SourceXzDecoder<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.check()?;
        if output.is_empty() || self.finished {
            return Ok(0);
        }
        loop {
            if self.start == self.end && !self.eof {
                self.start = 0;
                self.end = 0;
                self.end = self.reader.read(&mut self.input)?;
                self.eof = self.end == 0;
            }
            self.check()?;
            let available = self.end - self.start;
            self.stream.next_in = self.input[self.start..self.end].as_ptr();
            self.stream.avail_in = available;
            self.stream.next_out = output.as_mut_ptr();
            self.stream.avail_out = output.len();
            // SAFETY: exact live input/output ranges, no overlap, stable
            // allocator. Callback state is disjoint from stream/buffers.
            let status = unsafe {
                lzma_sys::lzma_code(
                    &mut self.stream,
                    if self.eof {
                        lzma_sys::LZMA_FINISH
                    } else {
                        lzma_sys::LZMA_RUN
                    },
                )
            };
            let consumed = available.checked_sub(self.stream.avail_in);
            let produced = output.len().checked_sub(self.stream.avail_out);
            self.stream.next_in = std::ptr::null();
            self.stream.avail_in = 0;
            self.stream.next_out = std::ptr::null_mut();
            self.stream.avail_out = 0;
            self.check()?;
            let consumed = consumed.ok_or_else(|| io::Error::from(xz2::stream::Error::Program))?;
            let produced = produced.ok_or_else(|| io::Error::from(xz2::stream::Error::Program))?;
            self.start += consumed;
            status_result(status)?;
            if status == lzma_sys::LZMA_STREAM_END {
                self.finished = true;
            }
            if produced > 0 || self.eof {
                if produced == 0 && !self.finished {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "premature eof",
                    ));
                }
                return Ok(produced);
            }
            if consumed == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "corrupt xz stream",
                ));
            }
        }
    }
}
impl<R: Read> Drop for SourceXzDecoder<R> {
    fn drop(&mut self) {
        // SAFETY: end accepts the zero/partially initialized stream and frees
        // every library block while allocator/context/allowance are still live.
        unsafe { lzma_sys::lzma_end(&mut self.stream) };
    }
}

fn publish(scope: Option<&SourceServiceScope>, failure: Failure) -> io::Error {
    let error = match failure {
        Failure::DecoderLimit => SourceServiceError::DecoderLimit,
        Failure::Allocation => SourceServiceError::Allocation,
    };
    io::Error::other(scope.map_or(error, |s| s.poison(error)))
}
fn status_result(status: lzma_sys::lzma_ret) -> io::Result<()> {
    use xz2::stream::Error as E;
    let error = match status {
        lzma_sys::LZMA_OK
        | lzma_sys::LZMA_STREAM_END
        | lzma_sys::LZMA_GET_CHECK
        | lzma_sys::LZMA_BUF_ERROR => return Ok(()),
        lzma_sys::LZMA_NO_CHECK => E::NoCheck,
        lzma_sys::LZMA_UNSUPPORTED_CHECK => E::UnsupportedCheck,
        lzma_sys::LZMA_MEM_ERROR => E::Mem,
        lzma_sys::LZMA_MEMLIMIT_ERROR => E::MemLimit,
        lzma_sys::LZMA_FORMAT_ERROR => E::Format,
        lzma_sys::LZMA_OPTIONS_ERROR => E::Options,
        lzma_sys::LZMA_DATA_ERROR => E::Data,
        _ => E::Program,
    };
    Err(error.into())
}

#[cfg(test)]
#[path = "xz_decoder_tests.rs"]
mod tests;
