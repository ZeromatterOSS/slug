//! Bounded input ownership and cancellation for verified ByteStream writes.

use std::future::Future;
use std::io::ErrorKind;

use sha2::Digest as _;
use sha2::Sha256;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;

use super::CacheError;
use super::ReapiDigest;
use super::bytestream;

pub(super) async fn upload<R, F, Fut>(
    reader: R,
    expected: &ReapiDigest,
    resource: String,
    chunk_bytes: usize,
    write: F,
) -> Result<(), CacheError>
where
    R: AsyncRead + Unpin,
    F: FnOnce(mpsc::Receiver<bytestream::WriteRequest>) -> Fut,
    Fut: Future<Output = Result<bytestream::WriteResponse, CacheError>>,
{
    let (sender, receiver) = mpsc::channel(1);
    let producer = produce(reader, expected, resource, chunk_bytes, sender);
    let rpc = write(receiver);
    tokio::pin!(producer, rpc);
    let response = tokio::select! {
        biased;
        produced = &mut producer => {
            produced?;
            rpc.await?
        }
        response = &mut rpc => {
            response?;
            return Err(CacheError::Protocol(
                "ByteStream responded before local upload verification completed".to_owned(),
            ));
        }
    };
    if response.committed_size != expected.size_bytes() as i64 {
        return Err(CacheError::Protocol(format!(
            "ByteStream committed {} of {} bytes",
            response.committed_size,
            expected.size_bytes(),
        )));
    }
    Ok(())
}

async fn produce<R: AsyncRead + Unpin>(
    mut reader: R,
    expected: &ReapiDigest,
    mut resource: String,
    chunk_bytes: usize,
    sender: mpsc::Sender<bytestream::WriteRequest>,
) -> Result<(), CacheError> {
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    loop {
        // Backpressure applies before reading or allocating the next payload.
        let permit = sender.reserve().await.map_err(|_| {
            CacheError::Protocol("ByteStream stopped consuming upload input".to_owned())
        })?;
        let remaining = expected.size_bytes() - size;
        let limit = (remaining + 1).min(chunk_bytes as u64) as usize;
        let mut data = vec![0; limit];
        let mut filled = 0;
        while filled < data.len() {
            match reader.read(&mut data[filled..]).await {
                Ok(0) => break,
                Ok(count) => filled += count,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) => return Err(CacheError::Source(format!("CAS upload read: {error}"))),
            }
        }
        if filled == 0 {
            let actual_hash = format!("{:x}", hasher.finalize());
            if size != expected.size_bytes() || actual_hash != expected.hash() {
                return Err(CacheError::Protocol(format!(
                    "upload digest mismatch: expected {expected}, got {actual_hash}/{size}"
                )));
            }
            permit.send(bytestream::WriteRequest {
                resource_name: resource,
                write_offset: size as i64,
                finish_write: true,
                data: Vec::new(),
            });
            return Ok(());
        }
        if filled as u64 > remaining {
            return Err(CacheError::Protocol(
                "upload exceeds declared digest size".to_owned(),
            ));
        }
        data.truncate(filled);
        hasher.update(&data);
        permit.send(bytestream::WriteRequest {
            resource_name: std::mem::take(&mut resource),
            write_offset: size as i64,
            finish_write: false,
            data,
        });
        size += filled as u64;
    }
}

#[cfg(test)]
mod tests;
