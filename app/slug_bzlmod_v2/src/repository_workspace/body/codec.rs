use super::*;

// lz4_flex 0.14.0 block::get_maximum_output_size(65536): 20 + 110*n/100.
const OUTPUT_BOUND: usize = 72_109;
/// Working buffers belong only to one Builder and its prepaid construction
/// allowance. Neither buffer is retained by a closed Body.
#[derive(Default)]
pub(super) struct Workspace {
    output: Vec<u8>,
    validation: Vec<u8>,
}
impl Workspace {
    pub(super) fn encode_and_validate(&mut self, raw: &[u8]) -> Result<&[u8], Error> {
        let written = encode_into(raw, &mut self.output)?;
        // Preserve first-append order: encode before validation allocation.
        if self.validation.capacity() == 0 {
            #[cfg(test)]
            super::construction_work_for_test::validation_reserve(CHUNK);
            self.validation
                .try_reserve_exact(CHUNK)
                .map_err(|_| Error::Allocation)?;
        }
        if self.validation.capacity() > CHUNK {
            return Err(Error::Storage);
        }
        if self.validation.len() < raw.len() {
            #[cfg(test)]
            super::construction_work_for_test::validation_init(raw.len() - self.validation.len());
            self.validation.resize(raw.len(), 0);
        }
        #[cfg(test)]
        super::construction_work_for_test::decode(raw.len());
        decode(&self.output[..written], &mut self.validation[..raw.len()])?;
        #[cfg(test)]
        super::construction_work_for_test::compare(raw.len());
        if &self.validation[..raw.len()] != raw {
            return Err(Error::Codec);
        }
        Ok(&self.output[..written])
    }
}
fn encode_into(raw: &[u8], output: &mut Vec<u8>) -> Result<usize, Error> {
    if raw.len() > CHUNK {
        return Err(Error::Length);
    }
    if output.capacity() == 0 {
        #[cfg(test)]
        super::construction_work_for_test::output_reserve(OUTPUT_BOUND);
        output
            .try_reserve_exact(OUTPUT_BOUND)
            .map_err(|_| Error::Allocation)?;
    }
    if output.capacity() > OUTPUT_BOUND {
        return Err(Error::Storage);
    }
    if output.len() < OUTPUT_BOUND {
        #[cfg(test)]
        super::construction_work_for_test::output_init(OUTPUT_BOUND - output.len());
        output.resize(OUTPUT_BOUND, 0);
    }
    #[cfg(test)]
    super::construction_work_for_test::encode(raw.len());
    // Keep the original fresh-table choice on EVERY independent block. The
    // reusable CompressTable API would retain a different table for short tails.
    let written = lz4_flex::block::compress_into(raw, output).map_err(|_| Error::Codec)?;
    if written > output.len() {
        return Err(Error::Codec);
    }
    Ok(written)
}
#[cfg(test)]
pub(super) fn encode(raw: &[u8]) -> Result<Vec<u8>, Error> {
    // Existing grammar tests keep a fresh owned result via the same compressor.
    let mut output = Vec::new();
    let written = encode_into(raw, &mut output)?;
    output.truncate(written);
    Ok(output)
}
pub(super) fn decode(encoded: &[u8], out: &mut [u8]) -> Result<(), Error> {
    if encoded.len() > OUTPUT_BOUND || out.len() > CHUNK {
        return Err(Error::Length);
    }
    #[cfg(test)]
    super::record_work(false, out.len());
    // Pinned 0.14.0 block/decompress_safe.rs has one successful loop exit:
    // input_pos >= input.len(), after checked literal consumption. Its safe
    // public API returns only bytes written, NOT a consumed-input counter.
    // Closed publication additionally requires the builder's full-byte check.
    let written = lz4_flex::block::decompress_into(encoded, out).map_err(|_| Error::Codec)?;
    if written != out.len() {
        return Err(Error::Codec);
    }
    Ok(())
}
/// Preserve the admitted 2MiB construction workspace. The pinned safe encoder
/// has a fresh 4096-entry u16 table below 65535 input bytes, otherwise u32;
/// no alloc/std/frame features are enabled. Output and validation Vecs are
/// fallible, bounded and construction-local; all initialized storage is
/// discarded before Body publication. No headroom refund.
pub(super) fn scratch_bound() -> usize {
    2 * 1024 * 1024
}
#[cfg(test)]
pub(super) fn scratch_components() -> usize {
    // Conservatively include both table regimes and construction/transfer
    // copies, output Vec overlap, validation/raw/selected blocks and controls.
    // hashtable.rs: 4096 entries; u16 table repr(align(64)), size 8192;
    // u32 table size 16384. Decoder is scalar/slice-only (no table or heap).
    2 * (4096 * size_of::<u16>() + 4096 * size_of::<u32>()) + 2 * OUTPUT_BOUND + 4 * CHUNK + 8192
}
