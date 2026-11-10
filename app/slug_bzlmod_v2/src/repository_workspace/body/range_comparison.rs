//! Bounded operation-local range comparison; no decoded answer is retained.
use super::*;

impl RepositoryWorkspaceBody {
    /// Run synchronous range comparisons against this immutable logical body.
    /// Every call independently checks bounds and compares the requested bytes.
    pub fn with_range_comparison<R>(
        &self,
        callback: impl FnOnce(&mut dyn FnMut(usize, &[u8]) -> bool) -> R,
    ) -> R {
        match &self.0 {
            Repr::Packed(data) if data.logical != 0 => {
                prepared_packed_range_comparison(data, callback)
            }
            _ => forward_range_comparison(self, callback),
        }
    }
}

// Keep callback frames out of the public dispatcher, including Raw/empty
// branches. Empty Packed retains the ordinary single-range behavior.
#[inline(never)]
fn forward_range_comparison<R>(
    body: &RepositoryWorkspaceBody,
    callback: impl FnOnce(&mut dyn FnMut(usize, &[u8]) -> bool) -> R,
) -> R {
    let mut compare = |offset: usize, bytes: &[u8]| body.range_eq(offset, bytes);
    callback(&mut compare)
}

// Exactly one operation-owned block replaces repeated single-range blocks.
// Keep this frame out of the dispatcher and Raw/empty fallback. The closure
// borrows scratch in place; it never owns a moved/copied array or decoded tag.
#[inline(never)]
fn prepared_packed_range_comparison<R>(
    data: &PackedData,
    callback: impl FnOnce(&mut dyn FnMut(usize, &[u8]) -> bool) -> R,
) -> R {
    #[cfg(test)]
    super::range_comparison_work_for_test::initialize(CHUNK);
    let mut scratch = [0u8; CHUNK];
    let mut compare = |offset: usize, bytes: &[u8]| {
        if offset
            .checked_add(bytes.len())
            .is_none_or(|end| end > data.logical)
        {
            return false;
        }
        packed_range_eq(data, offset, bytes, &mut scratch)
    };
    callback(&mut compare)
}

// Called only after ordinary range_eq has checked bounds. Its array frame must
// not be inlined into Raw dispatch or a caller that owns another read block.
#[inline(never)]
pub(super) fn single_packed_range_eq(data: &PackedData, offset: usize, bytes: &[u8]) -> bool {
    #[cfg(test)]
    super::range_comparison_work_for_test::initialize(CHUNK);
    let mut scratch = [0u8; CHUNK];
    packed_range_eq(data, offset, bytes, &mut scratch)
}

// The caller has checked offset+bytes.len against data.logical. This kernel
// owns no array; both the single-range and future operation workspace use it.
pub(super) fn packed_range_eq(
    data: &PackedData,
    offset: usize,
    bytes: &[u8],
    scratch: &mut [u8; CHUNK],
) -> bool {
    #[cfg(test)]
    super::range_comparison_work_for_test::range(bytes.len());
    let mut position = offset;
    let mut compared = 0;
    while compared < bytes.len() {
        let index = position / CHUNK;
        let within = position % CHUNK;
        let block = &data.blocks[index];
        let length = (data.logical - index * CHUNK).min(CHUNK);
        let decoded = if block.compressed {
            #[cfg(test)]
            super::range_comparison_work_for_test::decode(length);
            codec::decode(&block.bytes, &mut scratch[..length])
                .expect("closed workspace body invariant");
            &scratch[..length]
        } else {
            &block.bytes
        };
        let count = (length - within).min(bytes.len() - compared);
        if decoded[within..within + count] != bytes[compared..compared + count] {
            return false;
        }
        position += count;
        compared += count;
    }
    true
}
