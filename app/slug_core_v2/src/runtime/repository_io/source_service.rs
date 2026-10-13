//! Diagnostic source accounting; none of these helpers mints source authority.
use std::io::Read;
use std::io::Write;
use std::io::{self};
use std::sync::Arc;

use crate::runtime::dice::source_service::SourceBufferReservation;
pub(in crate::runtime) use crate::runtime::dice::source_service::SourceBufferSite;
pub(in crate::runtime) use crate::runtime::dice::source_service::SourceBufferUnit;
pub(in crate::runtime) use crate::runtime::dice::source_service::SourceCounter;
use crate::runtime::dice::source_service::SourceDecoderReservation;
pub(in crate::runtime) use crate::runtime::dice::source_service::SourceServiceError;
pub(in crate::runtime) use crate::runtime::dice::source_service::SourceServiceScope;

pub(in crate::runtime) fn check_identity(
    installed: Option<&Arc<SourceServiceScope>>,
    supplied: Option<&Arc<SourceServiceScope>>,
) -> Result<(), SourceServiceError> {
    match (installed, supplied) {
        (None, None) => Ok(()),
        (Some(a), Some(b)) if Arc::ptr_eq(a, b) => a.check(),
        _ => Err(SourceServiceError::ForeignScope),
    }
}

pub(in crate::runtime) fn check(scope: Option<&Arc<SourceServiceScope>>) -> Result<(), String> {
    scope.map_or(Ok(()), |s| s.check().map_err(|e| e.to_string()))
}

pub(in crate::runtime) fn charge(
    scope: Option<&Arc<SourceServiceScope>>,
    counter: SourceCounter,
    bytes: u64,
) -> Result<(), String> {
    scope.map_or(Ok(()), |s| {
        s.charge(counter, bytes).map_err(|e| e.to_string())
    })
}

pub(in crate::runtime) fn buffer(
    scope: Option<&Arc<SourceServiceScope>>,
    bytes: usize,
) -> Result<Option<SourceBufferReservation>, String> {
    scope
        .map(|s| {
            s.reserve_buffer_at(bytes, SourceBufferSite::SharedIoBuffer)
                .map_err(|e| e.to_string())
        })
        .transpose()
}

/// Reserve codec allocation before constructing a decoder. The token owns the
/// scope and stays live until the decoder has been dropped on every exit.
pub(in crate::runtime) fn decoder(
    scope: Option<&Arc<SourceServiceScope>>,
    bytes: usize,
) -> Result<Option<SourceDecoderReservation>, String> {
    scope
        .map(|s| s.reserve_decoder(bytes).map_err(|e| e.to_string()))
        .transpose()
}

/// Bound the decoder request BEFORE read. At the exact service cap only a
/// fixed one-byte EOF probe is allowed; overflow poisons and is never output.
pub(in crate::runtime) fn read_counted(
    reader: &mut impl Read,
    bytes: &mut [u8],
    active: &dyn Fn() -> bool,
    scope: Option<&Arc<SourceServiceScope>>,
    counter: SourceCounter,
) -> io::Result<usize> {
    if scope.is_none() {
        return reader.read(bytes);
    }
    let operational = |e: String| io::Error::other(e);
    if !active() {
        return Err(operational("repository session is no longer active".into()));
    }
    check(scope).map_err(operational)?;
    if bytes.is_empty() {
        return Ok(0);
    }
    let allowed = match scope {
        Some(s) => bytes.len().min(
            usize::try_from(
                s.remaining(counter)
                    .map_err(|e| operational(e.to_string()))?,
            )
            .unwrap_or(usize::MAX),
        ),
        None => bytes.len(),
    };
    if allowed == 0 {
        let mut probe = [0; 1];
        let n = reader.read(&mut probe)?;
        if n != 0 {
            let s = scope.expect("only a scoped counter can exhaust here");
            return Err(operational(
                s.poison(SourceServiceError::Limit(counter)).to_string(),
            ));
        }
        check(scope).map_err(operational)?;
        if !active() {
            return Err(operational("repository session is no longer active".into()));
        }
        return Ok(0);
    }
    let count = reader.read(&mut bytes[..allowed])?;
    charge(scope, counter, count as u64).map_err(operational)?;
    if !active() {
        return Err(operational("repository session is no longer active".into()));
    }
    Ok(count)
}

pub(in crate::runtime) fn write_counted(
    output: &mut impl Write,
    bytes: &[u8],
    active: &dyn Fn() -> bool,
    scope: Option<&Arc<SourceServiceScope>>,
) -> io::Result<()> {
    if scope.is_none() {
        return output.write_all(bytes);
    }
    if !active() {
        return Err(io::Error::other("repository session is no longer active"));
    }
    charge(scope, SourceCounter::WrittenBytes, bytes.len() as u64).map_err(io::Error::other)?;
    output.write_all(bytes)?;
    check(scope).map_err(io::Error::other)?;
    if !active() {
        return Err(io::Error::other("repository session is no longer active"));
    }
    Ok(())
}

pub(in crate::runtime) fn copy_counted(
    input: &mut impl Read,
    output: &mut impl Write,
    limit: u64,
    active: &dyn Fn() -> bool,
    scope: Option<&Arc<SourceServiceScope>>,
) -> io::Result<u64> {
    let _credit = buffer(scope, 64 * 1024).map_err(io::Error::other)?;
    let mut buffer = [0; 64 * 1024];
    let mut total = 0u64;
    loop {
        if !active() {
            return Err(io::Error::other("repository session is no longer active"));
        }
        check(scope).map_err(io::Error::other)?;
        let allowed = buffer
            .len()
            .min(usize::try_from(limit - total).unwrap_or(usize::MAX));
        if allowed == 0 {
            let mut probe = [0; 1];
            return match input.read(&mut probe)? {
                0 => Ok(total),
                _ => Err(io::Error::other("verified copy exceeds size limit")),
            };
        }
        let n = input.read(&mut buffer[..allowed])?;
        if n == 0 {
            check(scope).map_err(io::Error::other)?;
            return Ok(total);
        }
        write_counted(output, &buffer[..n], active, scope)?;
        total += n as u64;
    }
}
