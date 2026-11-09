use miniz_oxide::deflate::core::CompressorOxide;
use miniz_oxide::deflate::core::TDEFLFlush;
use miniz_oxide::deflate::core::TDEFLStatus;
use miniz_oxide::deflate::core::compress;
use miniz_oxide::deflate::core::create_comp_flags_from_zip_params;
use miniz_oxide::inflate::TINFLStatus;
use miniz_oxide::inflate::core::DecompressorOxide;
use miniz_oxide::inflate::core::decompress;
use miniz_oxide::inflate::core::inflate_flags;

use super::*;

const OUTPUT_BOUND: usize = 2 * CHUNK + 1024;
pub(super) fn encode(raw: &[u8]) -> Result<Vec<u8>, Error> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(OUTPUT_BOUND)
        .map_err(|_| Error::Allocation)?;
    if output.capacity() > OUTPUT_BOUND {
        return Err(Error::Storage);
    }
    output.resize(OUTPUT_BOUND, 0);
    let mut compressor = CompressorOxide::new(create_comp_flags_from_zip_params(1, 0, 0));
    let (status, consumed, written) =
        compress(&mut compressor, raw, &mut output, TDEFLFlush::Finish);
    if status != TDEFLStatus::Done || consumed != raw.len() {
        return Err(Error::Codec);
    }
    output.truncate(written);
    Ok(output)
}
pub(super) fn decode(encoded: &[u8], out: &mut [u8]) -> Result<(), Error> {
    #[cfg(test)]
    super::record_work(false, out.len());
    let mut decoder = DecompressorOxide::new();
    let (status, consumed, written) = decompress(
        &mut decoder,
        encoded,
        out,
        0,
        inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF,
    );
    if status != TINFLStatus::Done || consumed != encoded.len() || written != out.len() {
        return Err(Error::Codec);
    }
    Ok(())
}
/// 2MiB prepays the codec's five boxed payloads (253614 bytes in pinned0.8.9),
/// overlapping construction copies, bounded output Vec/resize alternatives,
/// two raw/decode blocks, decompressor and compressor inline controls.
pub(super) fn scratch_bound() -> usize {
    2 * 1024 * 1024
}
#[cfg(test)]
pub(super) fn scratch_components() -> usize {
    2 * 253_614
        + 2 * size_of::<CompressorOxide>()
        + 2 * OUTPUT_BOUND
        + 4 * CHUNK
        + 2 * size_of::<DecompressorOxide>()
        + 4096
}
