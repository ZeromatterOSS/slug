use std::mem::size_of;

use slug_bzlmod_v2::HostSelectedExtensionOwnerKind;

#[test]
fn innate_owner_kind_is_an_explicit_cross_crate_projection() {
    assert_ne!(
        HostSelectedExtensionOwnerKind::ModuleExtension,
        HostSelectedExtensionOwnerKind::InnateRepositoryRule
    );
    assert_eq!(size_of::<HostSelectedExtensionOwnerKind>(), 1);
}

#[test]
fn innate_loading_reuses_canonical_owners_and_stays_split_from_regular_evaluation() {
    let innate = include_str!("../src/module_extension_innate_repository.rs");
    let ordinary = include_str!("../src/module_extension.rs");
    let instantiation = include_str!("../src/module_extension_repository_instantiation.rs");
    assert!(innate.contains("HostCanonicalRepositoryLoadRouteObservationKey"));
    assert!(innate.contains("ExternalBzlModuleObservationKey::new_canonical"));
    assert!(innate.contains("FrozenRepositoryRuleDefinition"));
    assert!(instantiation.contains("label_conversion_base"));
    assert!(!ordinary.contains("InnateRepository"));
}
