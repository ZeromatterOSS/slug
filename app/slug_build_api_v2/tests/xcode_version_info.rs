//! The finite Xcode provider has a typed, publication-stable builtin identity.

use slug_build_api_v2::DefaultInfo;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::ProviderError;
use slug_build_api_v2::ProviderId;
use slug_build_api_v2::ProviderIdentity;
use slug_build_api_v2::ProviderOccurrence;
use slug_build_api_v2::ProviderValue;
use slug_build_api_v2::XcodeVersionInfo;

#[test]
fn default_xcode_version_info_is_a_compact_typed_publication() {
    assert_eq!(std::mem::size_of::<XcodeVersionInfo>(), 0);
    let builtin = ProviderIdentity::builtin("XcodeVersionInfo");
    let first = ProviderCollection::from_values(
        vec![ProviderValue::XcodeVersionInfo(
            XcodeVersionInfo::default_empty_version(),
        )],
        false,
    )
    .unwrap();
    let same = ProviderCollection::from_values(
        vec![ProviderValue::XcodeVersionInfo(
            XcodeVersionInfo::default_empty_version(),
        )],
        false,
    )
    .unwrap();
    let absent = ProviderCollection::from_values(Vec::new(), false).unwrap();

    assert!(first.contains(&builtin));
    assert_eq!(first.names().next().unwrap().as_str(), "XcodeVersionInfo");
    assert!(matches!(
        first.get(&builtin),
        Some(ProviderValue::XcodeVersionInfo(_))
    ));
    assert_eq!(
        first.xcode_version_info(),
        Some(&XcodeVersionInfo::default_empty_version())
    );
    assert_eq!(first, same);
    assert_ne!(first, absent);

    let published = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(DefaultInfo::empty()),
        ProviderValue::XcodeVersionInfo(XcodeVersionInfo::default_empty_version()),
    ])
    .unwrap();
    assert!(published.default_info().is_some());
    assert_eq!(published.xcode_version_info(), first.xcode_version_info());
}

#[test]
fn generic_xcode_occurrence_cannot_impersonate_the_typed_provider() {
    let forged = ProviderCollection::from_values(
        vec![ProviderValue::Occurrence(ProviderOccurrence::empty(
            ProviderIdentity::builtin("XcodeVersionInfo"),
        ))],
        false,
    )
    .unwrap_err();
    assert_eq!(forged, ProviderError::InvalidXcodeVersionInfoOccurrence);

    let duplicate = ProviderCollection::from_values(
        vec![
            ProviderValue::XcodeVersionInfo(XcodeVersionInfo::default_empty_version()),
            ProviderValue::XcodeVersionInfo(XcodeVersionInfo::default_empty_version()),
        ],
        false,
    )
    .unwrap_err();
    assert!(matches!(duplicate, ProviderError::DuplicateProvider { .. }));

    let user_identity =
        ProviderIdentity::user(ProviderId::new("@@//:defs.bzl", "XcodeVersionInfo").unwrap());
    let unrelated_user = ProviderCollection::from_values(
        vec![ProviderValue::Occurrence(ProviderOccurrence::empty(
            user_identity.clone(),
        ))],
        false,
    )
    .unwrap();
    assert!(unrelated_user.contains(&user_identity));
    assert!(!unrelated_user.contains(&ProviderIdentity::builtin("XcodeVersionInfo")));
    assert!(unrelated_user.xcode_version_info().is_none());
}
