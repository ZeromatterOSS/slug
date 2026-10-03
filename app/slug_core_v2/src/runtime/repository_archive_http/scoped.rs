//! Owned diagnostic archive transport. Baseline capture remains separate.
use super::*;
use crate::runtime::dice::source_service::SourceBufferSite;
use crate::runtime::dice::source_service::SourceBufferUnit;
use crate::runtime::dice::source_service::SourceHttpAdmission;
use crate::runtime::dice::source_service::request_scoped;

pub(super) fn transfer_selected_capture(
    mut capture: tempfile::NamedTempFile,
    size: usize,
    sha256: [u8; 32],
    active: &dyn Fn() -> bool,
    scope: &Arc<SourceServiceScope>,
) -> Result<Arc<[u8]>, String> {
    let _arc_credit = scope
        .reserve_buffer_at(
            size.checked_add(64).ok_or("capture allocation overflow")?,
            SourceBufferSite::SelectedPayload,
        )
        .map_err(|e| e.to_string())?;
    let _scratch_credit = scope
        .reserve_buffer_at(64 * 1024, SourceBufferSite::SelectedScratch)
        .map_err(|e| e.to_string())?;
    // The final Arc allocation was already infallible in the Vec conversion.
    // Partial failures keep MaybeUninit storage and never expose its bytes.
    let mut bytes = Arc::<[u8]>::new_uninit_slice(size);
    let mut buffer = [0u8; 64 * 1024];
    let mut hasher = Sha256::new();
    let mut initialized = 0usize;
    loop {
        source_service::check(Some(scope))?;
        if !active() {
            return Err("selected extension artifact capture cancelled".into());
        }
        let count = match capture.as_file_mut().read(&mut buffer) {
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err("selected extension artifact capture unreadable".into()),
        };
        if count == 0 {
            break;
        }
        let next = initialized
            .checked_add(count)
            .filter(|n| *n as u64 <= SELECTED_EXTENSION_CAPTURE_LIMIT)
            .ok_or("selected extension artifact exceeds 33554432 byte capture limit")?;
        if next > size {
            return Err(scope
                .poison_buffer_limit_at(
                    SourceBufferSite::SelectedPayloadGrowth,
                    next,
                    size,
                    SourceBufferUnit::Bytes,
                )
                .to_string());
        }
        let slots = Arc::get_mut(&mut bytes).expect("unpublished capture Arc is unique");
        for (slot, value) in slots[initialized..next].iter_mut().zip(&buffer[..count]) {
            slot.write(*value);
        }
        initialized = next;
        hasher.update(&buffer[..count]);
    }
    if !active() {
        return Err("selected extension artifact capture cancelled".into());
    }
    if hasher.finalize().as_slice() != sha256 {
        return Err("selected extension artifact SHA-256 mismatch".into());
    }
    source_service::check(Some(scope))?;
    if initialized != size {
        return Err("selected extension artifact capture changed length".into());
    }
    source_service::check(Some(scope))?;
    if !active() {
        return Err("selected extension artifact capture cancelled".into());
    }
    // SAFETY: every slot in 0..size was written exactly through the bounded
    // initialized prefix; EOF, exact size and digest were verified above. No
    // Arc clone or initialized slice escaped while any slot was uninitialized.
    Ok(unsafe { bytes.assume_init() })
}

pub(super) fn capture_urls(
    urls: &[String],
    integrity: [u8; 32],
    limit: u64,
    subject: &'static str,
    runtime: &tokio::runtime::Runtime,
    active: &dyn Fn() -> bool,
    scope: &Arc<SourceServiceScope>,
) -> Result<tempfile::NamedTempFile, ArchiveMaterializationError> {
    let mut last = None;
    for url in urls {
        scope
            .check()
            .map_err(|e| ArchiveMaterializationError::transport(e.to_string()))?;
        match capture_one(url, integrity, limit, subject, runtime, active, scope) {
            Ok(capture) => return Ok(capture),
            Err(error) => {
                scope
                    .check()
                    .map_err(|e| ArchiveMaterializationError::transport(e.to_string()))?;
                last = Some(error);
            }
        }
    }
    Err(ArchiveMaterializationError::transport(
        last.unwrap_or_else(|| "no archive URL".into()),
    ))
}

pub(super) fn capture_one(
    original: &str,
    integrity: [u8; 32],
    limit: u64,
    subject: &str,
    runtime: &tokio::runtime::Runtime,
    active: &dyn Fn() -> bool,
    scope: &Arc<SourceServiceScope>,
) -> Result<tempfile::NamedTempFile, String> {
    let live = || {
        scope.check().map_err(|e| e.to_string())?;
        if !active() {
            return Err("repository session is no longer active".into());
        }
        Ok(())
    };
    live()?;
    check_address(scope, original)?;
    // Covers the bounded parsed URL plus old/new redirect spellings concurrently.
    let _address_credit = scope
        .reserve_buffer_at(64 * 1024, SourceBufferSite::ArchiveAddress)
        .map_err(|e| e.to_string())?;
    if original
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file:"))
    {
        scope
            .charge(SourceCounter::SourceAttempts, 1)
            .map_err(|e| e.to_string())?;
        let capture =
            tempfile::NamedTempFile::new().map_err(|_| "creating source capture failed")?;
        let capture = super::super::repository_archive_file::capture_file(
            original,
            capture,
            integrity,
            limit,
            subject,
            active,
            Some(scope),
        )?;
        live()?;
        return Ok(capture);
    }
    let mut url = url::Url::parse(original).map_err(|_| "invalid source URL")?;
    validate_https(&url)?;
    scope
        .charge(SourceCounter::SourceAttempts, 1)
        .map_err(|e| e.to_string())?;
    let mut capture =
        tempfile::NamedTempFile::new().map_err(|_| "creating source capture failed")?;
    for redirect in 0..=MAX_REDIRECTS {
        live()?;
        validate_https(&url)?;
        if redirect != 0 {
            scope
                .charge(SourceCounter::SourceAttempts, 1)
                .map_err(|e| e.to_string())?;
        }
        let mut response =
            runtime.block_on(request_scoped(scope, &url, SourceHttpAdmission::Archive))?;
        let status = response.status();
        if redirect_status(status) {
            let location = response
                .headers()
                .get(hyper::header::LOCATION)
                .map(|v| {
                    if v.as_bytes().len() > 8192 {
                        return Err(scope
                            .poison_buffer_limit_at(
                                SourceBufferSite::RedirectLength,
                                v.as_bytes().len(),
                                8192,
                                SourceBufferUnit::Bytes,
                            )
                            .to_string());
                    }
                    v.to_str()
                        .map(str::to_owned)
                        .map_err(|_| "invalid redirect Location".to_owned())
                })
                .transpose()
                .map_err(|_| "invalid redirect Location");
            let disposal = runtime.block_on(response.dispose());
            let location = location?.ok_or("redirect response missing Location")?;
            disposal?;
            live()?;
            if redirect == MAX_REDIRECTS {
                return Err("BCR archive rejected the 40th redirect".into());
            }
            url = url.join(&location).map_err(|_| "invalid redirect URL")?;
            check_address(scope, url.as_str())?;
            continue;
        }
        if status != hyper::StatusCode::OK && status != hyper::StatusCode::PARTIAL_CONTENT {
            let _ = runtime.block_on(response.dispose());
            live()?;
            return Err(format!("HTTP status {status}"));
        }
        let length = response
            .headers()
            .get(hyper::header::CONTENT_LENGTH)
            .map(|v| {
                v.to_str()
                    .ok()
                    .and_then(|s| s.parse::<u64>().ok())
                    .ok_or("invalid Content-Length")
            })
            .transpose();
        let body_result = (|| {
            if length?.is_some_and(|n| n > limit) {
                return Err(scope
                    .poison(source_service::SourceServiceError::Limit(
                        SourceCounter::TransferBytes,
                    ))
                    .to_string());
            }
            let mut written = 0u64;
            let mut hasher = Sha256::new();
            loop {
                live()?;
                let Some(bytes) = runtime.block_on(response.next_data())? else {
                    break;
                };
                scope
                    .charge(SourceCounter::TransferBytes, bytes.len() as u64)
                    .map_err(|e| e.to_string())?;
                written = written
                    .checked_add(bytes.len() as u64)
                    .filter(|n| *n <= limit)
                    .ok_or_else(|| {
                        scope
                            .poison(source_service::SourceServiceError::Limit(
                                SourceCounter::TransferBytes,
                            ))
                            .to_string()
                    })?;
                source_service::write_counted(&mut capture, &bytes, active, Some(scope))
                    .map_err(|e| e.to_string())?;
                hasher.update(&bytes);
            }
            capture
                .flush()
                .map_err(|_| "flushing source capture failed")?;
            if hasher.finalize().as_slice() != integrity {
                return Err(format!("BCR {subject} SHA-256 SRI mismatch"));
            }
            live()
        })();
        let disposal = runtime.block_on(response.dispose());
        body_result?;
        disposal?;
        live()?;
        return Ok(capture);
    }
    Err("BCR archive rejected the 40th redirect".into())
}

fn check_address(scope: &Arc<SourceServiceScope>, text: &str) -> Result<(), String> {
    scope.check().map_err(|e| e.to_string())?;
    if text.len() > 8192 {
        return Err(scope
            .poison_buffer_limit_at(
                SourceBufferSite::ArchiveUrlLength,
                text.len(),
                8192,
                SourceBufferUnit::Bytes,
            )
            .to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use slug_workspace_v2::NormalizedAbsolutePath;

    use super::*;
    use crate::runtime::dice::source_service::SourceServiceError;
    use crate::runtime::dice::source_service::SourceServiceLimits;
    fn scope() -> Arc<SourceServiceScope> {
        let work = Instant::now() + Duration::from_secs(2);
        SourceServiceScope::new_for_test(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            work,
            work + Duration::from_secs(1),
            SourceServiceLimits::default(),
        )
        .unwrap()
    }
    #[test]
    fn diagnostic_address_bound_refuses_before_capture_or_source_attempt() {
        let own = scope();
        check_address(&own, &"x".repeat(8192)).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        assert!(
            capture_one(
                &"x".repeat(8193),
                [0; 32],
                1,
                "test",
                &runtime,
                &|| true,
                &own
            )
            .is_err()
        );
        assert_eq!(own.check(), Err(SourceServiceError::BufferLimit));
        assert_eq!(own.snapshot().source_attempts, 0);
        assert_eq!(own.snapshot().transferred_bytes, 0);
        assert_eq!(own.snapshot().written_bytes, 0);
        assert_eq!(own.snapshot().live_buffer_bytes, 0);
    }
}
