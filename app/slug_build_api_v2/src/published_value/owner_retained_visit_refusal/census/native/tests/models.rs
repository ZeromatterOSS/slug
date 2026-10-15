use super::*;

#[test]
fn actual_inner_model_aliases_foreign_ties_and_configuration_weights_stay_advisory() {
    let model = features("feature".into());
    let configuration = model.configure_features(vec!["feature".into()]).unwrap();
    let foreign = features("feature".into());
    assert_eq!(model.model(), foreign.model());
    assert!(!Arc::ptr_eq(model.model(), foreign.model()));
    let foreign_configuration = foreign.configure_features(vec!["feature".into()]).unwrap();
    let (a, af) = native_arena(vec![
        AnalysisValue::cc_toolchain_features(model.clone()),
        AnalysisValue::cc_feature_configuration(configuration.clone()),
        AnalysisValue::cc_toolchain_features(model.clone()),
    ]);
    let (b, bf) = native_arena(vec![AnalysisValue::cc_feature_configuration(configuration)]);
    let (c, cf) = native_arena(vec![AnalysisValue::cc_toolchain_features(foreign.clone())]);
    assert_eq!(af[0], af[2], "warm hits keep complete wrapper certificates");
    assert!(
        af[1] > af[0],
        "configuration suffix contributes to FULL wrapper weight"
    );
    let expected = af.iter().sum::<usize>() + bf[0];
    let mut audit = draft(&[a.clone(), b.clone(), c.clone()]);
    let closure = summary(&mut audit);
    let out = closure.native_retention().unwrap();
    assert!(out.feature_selected() && !out.feature_ambiguous());
    assert_eq!(out.feature_maximum_wrapper_visits(), af[1]);
    assert_eq!(
        (
            out.feature_occurrences(),
            out.feature_arenas(),
            out.feature_wrapper_visits()
        ),
        (4, 2, expected)
    );
    assert_eq!(
        out.family(PublicationNativeRootFamily::CcFeatureModel)
            .count(),
        3
    );
    assert_eq!(
        out.family(PublicationNativeRootFamily::CcFeatureConfiguration)
            .count(),
        2
    );
    // Exercise reset deterministically with actual factory certificate weights.
    let mut candidate = feature::Candidate::default();
    let mut shown = PublicationNativeRetentionCensus::default();
    candidate.consider(model.model(), af[0]);
    candidate.consider(foreign.model(), cf[0]);
    candidate.publish(&mut shown);
    assert!(shown.feature_ambiguous());
    candidate.consider(model.model(), af[1]);
    candidate.publish(&mut shown);
    assert!(!shown.feature_ambiguous());
    let (d, df) = native_arena(vec![AnalysisValue::cc_feature_configuration(
        foreign_configuration,
    )]);
    assert_eq!(df[0], af[1]);
    let mut audit = draft(&[a, b, c, d]);
    let closure = summary(&mut audit);
    let out = closure.native_retention().unwrap();
    assert!(out.feature_ambiguous());
    assert_eq!(out.feature_maximum_wrapper_visits(), af[1]);
    assert_eq!(out.feature_arenas(), 2);
    let actual = (out.feature_occurrences(), out.feature_wrapper_visits());
    assert!(
        actual == (4, expected) || actual == (2, cf[0] + df[0]),
        "tied hash iteration does not select semantic identity"
    );
}
