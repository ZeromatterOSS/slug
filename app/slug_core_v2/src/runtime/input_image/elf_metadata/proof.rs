//! Every parser read is saved before its bytes can influence a fact.
use std::fs::File;
use std::io;
use std::os::unix::fs::FileExt;

use sha2::Digest;
use sha2::Sha256;
use slug_workspace_v2::ElfMetadataLimit;
use slug_workspace_v2::ElfMetadataPolicy;
use slug_workspace_v2::ElfSourceDigestBinding;
use slug_workspace_v2::ImageDescriptorBudget;
use slug_workspace_v2::ImageDescriptorReservation;

use super::super::InputImageCaptureError;

#[derive(Debug)]
pub(super) enum ReadError {
    OutsideFile,
    Limit(ElfMetadataLimit),
    Operational(InputImageCaptureError),
}
impl From<InputImageCaptureError> for ReadError {
    fn from(error: InputImageCaptureError) -> Self {
        Self::Operational(error)
    }
}

struct Span {
    offset: u64,
    bytes: Vec<u8>,
}
struct NormalizedSpan {
    offset: u64,
    start: usize,
    len: usize,
}

pub(super) trait ReadAt {
    fn read_at(&self, bytes: &mut [u8], offset: u64) -> io::Result<usize>;
}
impl ReadAt for File {
    fn read_at(&self, bytes: &mut [u8], offset: u64) -> io::Result<usize> {
        FileExt::read_at(self, bytes, offset)
    }
}

pub(super) struct ProofReader<'a> {
    file: &'a dyn ReadAt,
    size: u64,
    policy: ElfMetadataPolicy,
    budget: &'a ImageDescriptorBudget,
    active: &'a dyn Fn() -> bool,
    spans: Vec<Span>,
    bytes: usize,
    credit: ImageDescriptorReservation,
}

fn check_active(active: &dyn Fn() -> bool) -> Result<(), InputImageCaptureError> {
    if active() {
        Ok(())
    } else {
        Err(InputImageCaptureError::Cancelled)
    }
}

pub(super) fn reserve_rows<T>(
    rows: &mut Vec<T>,
    credit: &mut ImageDescriptorReservation,
    additional: usize,
) -> Result<(), InputImageCaptureError> {
    let needed = rows
        .len()
        .checked_add(additional)
        .ok_or(InputImageCaptureError::LimitExceeded)?;
    if needed > rows.capacity() {
        let capacity = needed.max(
            rows.capacity()
                .max(4)
                .checked_mul(2)
                .ok_or(InputImageCaptureError::LimitExceeded)?,
        );
        // Charge replacement capacity while the old allocation remains live.
        credit.try_grow(
            capacity
                .checked_mul(std::mem::size_of::<T>())
                .ok_or(InputImageCaptureError::LimitExceeded)?,
        )?;
        rows.try_reserve_exact(capacity - rows.len())
            .map_err(|_| InputImageCaptureError::LimitExceeded)?;
    }
    Ok(())
}

impl<'a> ProofReader<'a> {
    pub(super) fn new(
        file: &'a dyn ReadAt,
        size: u64,
        policy: ElfMetadataPolicy,
        budget: &'a ImageDescriptorBudget,
        active: &'a dyn Fn() -> bool,
    ) -> Result<Self, InputImageCaptureError> {
        check_active(active)?;
        Ok(Self {
            file,
            size,
            policy,
            budget,
            active,
            spans: Vec::new(),
            bytes: 0,
            credit: budget.try_reserve(256)?,
        })
    }

    pub(super) fn size(&self) -> u64 {
        self.size
    }

    pub(super) fn read(&mut self, offset: u64, length: usize) -> Result<&[u8], ReadError> {
        check_active(self.active)?;
        if offset
            .checked_add(length as u64)
            .is_none_or(|end| end > self.size)
        {
            return Err(ReadError::OutsideFile);
        }
        let limits = self.policy.limits();
        let total = self
            .bytes
            .checked_add(length)
            .ok_or(ReadError::Limit(ElfMetadataLimit::ProofBytes))?;
        if total > limits.max_proof_bytes {
            return Err(ReadError::Limit(ElfMetadataLimit::ProofBytes));
        }
        if self.spans.len() >= limits.max_proof_spans {
            return Err(ReadError::Limit(ElfMetadataLimit::ProofSpans));
        }
        reserve_rows(&mut self.spans, &mut self.credit, 1)?;
        self.credit
            .try_grow(length)
            .map_err(InputImageCaptureError::from)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| InputImageCaptureError::LimitExceeded)?;
        bytes.resize(length, 0);
        let mut done = 0;
        while done < length {
            check_active(self.active)?;
            // Interrupted is operational failure, never a retry loop.
            let count = self
                .file
                .read_at(&mut bytes[done..], offset + done as u64)
                .map_err(InputImageCaptureError::from)?;
            if count == 0 {
                return Err(InputImageCaptureError::StaleSource.into());
            }
            done += count;
        }
        self.bytes = total;
        self.spans.push(Span { offset, bytes });
        Ok(&self.spans.last().expect("inserted proof span").bytes)
    }

    pub(super) fn verify(
        mut self,
        expected: ElfSourceDigestBinding,
    ) -> Result<(), InputImageCaptureError> {
        check_active(self.active)?;
        if self.size != expected.size_bytes() {
            return Err(InputImageCaptureError::StaleSource);
        }
        self.spans.sort_unstable_by_key(|span| span.offset);
        // Original buffers and this complete normalization allocation overlap
        // in lifetime. Both have independent precharged workspace credit.
        let normalized_bytes = self.bytes;
        let row_bytes = self
            .spans
            .len()
            .checked_mul(std::mem::size_of::<NormalizedSpan>())
            .ok_or(InputImageCaptureError::LimitExceeded)?;
        let _normalized_credit = self.budget.try_reserve(
            normalized_bytes
                .checked_add(row_bytes)
                .and_then(|n| n.checked_add(256))
                .ok_or(InputImageCaptureError::LimitExceeded)?,
        )?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(normalized_bytes)
            .map_err(|_| InputImageCaptureError::LimitExceeded)?;
        let mut spans: Vec<NormalizedSpan> = Vec::new();
        spans
            .try_reserve_exact(self.spans.len())
            .map_err(|_| InputImageCaptureError::LimitExceeded)?;
        for span in &self.spans {
            check_active(self.active)?;
            if let Some(last) = spans.last_mut() {
                let end = last.offset + last.len as u64;
                if span.offset <= end {
                    let relative = (span.offset - last.offset) as usize;
                    let overlap = span.bytes.len().min(last.len - relative);
                    if bytes[last.start + relative..last.start + relative + overlap]
                        != span.bytes[..overlap]
                    {
                        return Err(InputImageCaptureError::StaleSource);
                    }
                    bytes.extend_from_slice(&span.bytes[overlap..]);
                    last.len += span.bytes.len() - overlap;
                    continue;
                }
            }
            spans.push(NormalizedSpan {
                offset: span.offset,
                start: bytes.len(),
                len: span.bytes.len(),
            });
            bytes.extend_from_slice(&span.bytes);
        }
        // Saved originals are no longer needed after every overlap agrees.
        drop(self.spans);
        drop(self.credit);
        let mut hash = Sha256::new();
        let mut offset = 0u64;
        let mut cursor = 0usize;
        let mut covered = 0usize;
        let mut buffer = [0u8; 64 * 1024];
        while offset < self.size {
            check_active(self.active)?;
            let wanted = (self.size - offset).min(buffer.len() as u64) as usize;
            let count = self.file.read_at(&mut buffer[..wanted], offset)?;
            if count == 0 {
                return Err(InputImageCaptureError::StaleSource);
            }
            let end = offset + count as u64;
            while let Some(span) = spans.get(cursor) {
                if span.offset >= end {
                    break;
                }
                let begin = offset.max(span.offset);
                let span_end = span.offset + span.len as u64;
                let until = end.min(span_end);
                if until > begin {
                    let length = (until - begin) as usize;
                    let saved = span.start + (begin - span.offset) as usize;
                    let streamed = (begin - offset) as usize;
                    if bytes[saved..saved + length] != buffer[streamed..streamed + length] {
                        return Err(InputImageCaptureError::StaleSource);
                    }
                    covered += length;
                }
                if span_end <= end {
                    cursor += 1;
                } else {
                    break;
                }
            }
            hash.update(&buffer[..count]);
            offset = end;
        }
        check_active(self.active)?;
        if self.file.read_at(&mut buffer[..1], offset)? != 0
            || cursor != spans.len()
            || covered != bytes.len()
            || hash.finalize().as_slice() != expected.sha256()
        {
            return Err(InputImageCaptureError::StaleSource);
        }
        check_active(self.active)?;
        Ok(())
    }
}
