use slug_identity_v2::ApparentLabel;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;

use super::*;

fn mapped_label(id: &str) -> slug_identity_v2::CanonicalLabel {
    let mapping = RepositoryMapping::new(RepositoryMappingId::new(id).unwrap());
    ApparentLabel::parse("//pkg:target")
        .unwrap()
        .resolve(&mapping)
}

#[test]
fn configured_owner_label_audit_covers_complete_mapping_id_before_retention() {
    let short = mapped_label("short");
    let long_id = "mapping-provenance".repeat(512);
    let long = mapped_label(&long_id);
    let mut small = Audit::default();
    small.label(&short).unwrap();
    let mut large = Audit::default();
    large.label(&long).unwrap();
    let before = small.native.intrinsic_shape();
    let after = large.native.intrinsic_shape();
    assert_eq!(after.bytes - before.bytes, long_id.len() - "short".len());
    assert_eq!(after.visits, before.visits);

    let mut bounded = Audit::default();
    bounded
        .native
        .charge_bytes(slug_build_api_v2::published_value::BYTE_LIMIT - after.bytes + 1)
        .unwrap();
    assert!(bounded.label(&long).unwrap_err().contains("bytes"));
    assert!(bounded.native.charge_visit().is_err());
}
