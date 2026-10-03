//! Fixed numeric projection of the actual returned owner error; no error text.
use slug_bzlmod_v2::RegistryRefusalCause;
use slug_bzlmod_v2::RegistryRefusalDetail;
use slug_bzlmod_v2::SelectedSourceRefusalTermination;
use slug_loading_v2::SelectedPreparationOwnerError;
use slug_loading_v2::SelectedPreparationOwnerRefusalSite;

use super::NativeDemandSessionError;
use super::Receipt;

pub(super) fn emit(
    error: &NativeDemandSessionError,
    receipt: &mut Receipt,
) -> Result<(), &'static str> {
    // Existing plain anyhow::Error::new has no contexts. Bound future context
    // traversal too; unknown errors never supply a guessed semantic/native cause.
    let owner = match error {
        NativeDemandSessionError::Computation(error) => error
            .chain()
            .take(16)
            .find_map(|error| error.downcast_ref::<SelectedPreparationOwnerError>()),
        _ => None,
    };
    receipt.number("nested_owner_error_observed", u64::from(owner.is_some()))?;
    let mut source = None;
    let category = match owner {
        None => 0,
        Some(SelectedPreparationOwnerError::InvalidSelector) => 1,
        Some(SelectedPreparationOwnerError::MissingImport) => 2,
        Some(SelectedPreparationOwnerError::NotGenerated) => 3,
        Some(SelectedPreparationOwnerError::WrongOwnerFamily) => 4,
        Some(SelectedPreparationOwnerError::SourceSelection {
            site,
            classification,
        }) => {
            source = Some((*site, *classification));
            5
        }
        Some(SelectedPreparationOwnerError::Operational) => 6,
    };
    receipt.number("nested_owner_error_category", category)?;
    let mut missing = owner.is_none();
    let mut native = None;
    if let Some((site, classification)) = source {
        let site = match site {
            SelectedPreparationOwnerRefusalSite::MappingObservation => 1,
            SelectedPreparationOwnerRefusalSite::MappingSemantic => 2,
            SelectedPreparationOwnerRefusalSite::MappingView => 3,
            SelectedPreparationOwnerRefusalSite::DemandObservation => 4,
            SelectedPreparationOwnerRefusalSite::DemandSemantic => 5,
        };
        let termination = match classification.termination() {
            SelectedSourceRefusalTermination::Classified => 1,
            SelectedSourceRefusalTermination::OpaqueLeaf => 2,
            SelectedSourceRefusalTermination::Unknown => 3,
            SelectedSourceRefusalTermination::DepthLimit => 4,
            SelectedSourceRefusalTermination::UnexpectedSuccess => 5,
        };
        receipt.number("nested_owner_refusal_site", site)?;
        receipt.number("nested_refusal_termination", termination)?;
        receipt.number(
            "nested_variant_path_complete",
            u64::from(classification.variant_path_complete()),
        )?;
        receipt.number(
            "nested_variant_code_count",
            classification.codes().len() as u64,
        )?;
        // Repeated records are an ordered path, never a last-value dictionary.
        for code in classification.codes() {
            receipt.number("nested_variant_code", u64::from(code.code()))?;
        }
        native = classification.native_cause();
        receipt.number(
            "nested_native_detail_observed",
            u64::from(classification.native_detail_observed()),
        )?;
        missing = classification.termination() != SelectedSourceRefusalTermination::Classified;
    } else {
        receipt.number("nested_owner_refusal_site", 0)?;
        receipt.number("nested_refusal_termination", 0)?;
        receipt.number("nested_variant_path_complete", 0)?;
        receipt.number("nested_variant_code_count", 0)?;
        receipt.number("nested_native_detail_observed", 0)?;
    }
    emit_native(native, receipt)?;
    receipt.number("nested_frontier_proof_missing", u64::from(missing))?;
    Ok(())
}

fn emit_native(
    cause: Option<RegistryRefusalCause>,
    receipt: &mut Receipt,
) -> Result<(), &'static str> {
    let mut kind = 0;
    let mut io = 0;
    let mut errno = None;
    let mut status = None;
    let mut protocol = 0;
    let mut scope = 0;
    let mut counter = None;
    if let Some(cause) = cause {
        match cause.detail {
            RegistryRefusalDetail::Io {
                kind: observed,
                errno: observed_errno,
            } => {
                kind = 1;
                io = u64::from(observed.code());
                errno = observed_errno;
            }
            RegistryRefusalDetail::Status(observed) => {
                kind = 2;
                status = Some(observed);
            }
            RegistryRefusalDetail::Protocol(observed) => {
                kind = 3;
                protocol = u64::from(observed.code());
            }
            RegistryRefusalDetail::Scope(observed) => {
                kind = 4;
                scope = u64::from(observed.code());
                counter = observed.counter();
            }
            RegistryRefusalDetail::Opaque => kind = 5,
        }
    }
    for (key, value) in [
        ("nested_native_cause_present", u64::from(cause.is_some())),
        (
            "nested_native_site",
            cause.map_or(0, |cause| u64::from(cause.site.code())),
        ),
        ("nested_native_detail_kind", kind),
        ("nested_native_io_kind", io),
        ("nested_native_errno_present", u64::from(errno.is_some())),
        (
            "nested_native_errno_negative",
            u64::from(errno.is_some_and(|errno| errno < 0)),
        ),
        (
            "nested_native_errno_magnitude",
            errno.map_or(0, |errno| i64::from(errno).unsigned_abs()),
        ),
        ("nested_native_status_present", u64::from(status.is_some())),
        ("nested_native_status", status.map_or(0, u64::from)),
        ("nested_native_protocol", protocol),
        ("nested_native_scope", scope),
        (
            "nested_native_scope_counter_present",
            u64::from(counter.is_some()),
        ),
        (
            "nested_native_scope_counter",
            counter.map_or(0, |counter| u64::from(counter.code())),
        ),
    ] {
        receipt.number(key, value)?;
    }
    Ok(())
}

#[test]
fn unknown_error_never_infers_owner_or_native_cause_from_message() {
    let budget = slug_workspace_v2::ImageDescriptorBudget::new(128 * 1024);
    let mut receipt = Receipt::new(&budget).unwrap();
    let error = NativeDemandSessionError::Computation(anyhow::anyhow!(
        "secret-url-password source DNS lookup failed SourceSelection RegistryFile Transport"
    ));
    emit(&error, &mut receipt).unwrap();
    assert!(receipt.text.contains("nested_owner_error_observed=0\n"));
    assert!(receipt.text.contains("nested_native_cause_present=0\n"));
    assert!(receipt.text.contains("nested_frontier_proof_missing=1\n"));
    assert!(!receipt.text.contains("secret"));
    assert!(!receipt.text.contains("DNS"));
}

#[test]
fn native_numeric_schema_keeps_signed_errno_status_and_absence_distinct() {
    use slug_bzlmod_v2::RegistryRefusalIoKind;
    use slug_bzlmod_v2::RegistryRefusalSite;

    let budget = slug_workspace_v2::ImageDescriptorBudget::new(128 * 1024);
    let mut receipt = Receipt::new(&budget).unwrap();
    emit_native(
        Some(RegistryRefusalCause {
            site: RegistryRefusalSite::LocalPin,
            detail: RegistryRefusalDetail::Io {
                kind: RegistryRefusalIoKind::PermissionDenied,
                errno: Some(i32::MIN),
            },
        }),
        &mut receipt,
    )
    .unwrap();
    assert!(receipt.text.contains("nested_native_errno_present=1\n"));
    assert!(receipt.text.contains("nested_native_errno_negative=1\n"));
    assert!(
        receipt
            .text
            .contains("nested_native_errno_magnitude=2147483648\n")
    );
    receipt.text.clear();
    emit_native(None, &mut receipt).unwrap();
    assert!(receipt.text.contains("nested_native_cause_present=0\n"));
    assert!(receipt.text.contains("nested_native_errno_present=0\n"));
    receipt.text.clear();
    emit_native(
        Some(RegistryRefusalCause {
            site: RegistryRefusalSite::HttpStatus,
            detail: RegistryRefusalDetail::Status(503),
        }),
        &mut receipt,
    )
    .unwrap();
    assert!(receipt.text.contains("nested_native_status_present=1\n"));
    assert!(receipt.text.contains("nested_native_status=503\n"));
    assert!(receipt.text.contains("nested_native_errno_present=0\n"));
}
