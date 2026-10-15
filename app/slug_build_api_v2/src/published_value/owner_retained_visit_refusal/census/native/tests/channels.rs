use super::*;

#[test]
fn actual_three_certificate_channels_conserve_native_source_other_and_unweighted_rows() {
    let (arena, native, sources, other) = channel_arena();
    let (foreign, _, _, _) = channel_arena();
    assert_ne!(arena.identity(), foreign.identity());
    let mut parent = PublishedValueArenaBuilder::new();
    parent.import(arena.clone()).unwrap();
    let parent = seal(parent, 0);
    assert!(parent.sources().is_empty());
    let mut b = draft(&[arena.clone(), arena, parent.clone(), foreign]);
    let complete = summary(&mut b);
    let out = complete.native_retention().unwrap();
    assert_eq!(out.native_visits(), 2 * native);
    assert_eq!(complete.imported_source_body_facts(), 2 * sources);
    assert_eq!(out.other_visits(), 2 * other + parent.0.local_shape.visits);
    assert_eq!(
        out.native_visits() + complete.imported_source_body_facts() + out.other_visits(),
        b.import_visits
    );
    for channel in [
        PublicationNativeChannel::Atom,
        PublicationNativeChannel::HiddenFiles,
        PublicationNativeChannel::TargetProvider,
    ] {
        assert_eq!(out.channel(channel).count(), 2);
        assert!(out.channel(channel).visits() > 0);
    }
    assert_eq!(out.family(PublicationNativeRootFamily::Scalar).count(), 2);
    assert_eq!(
        out.family(PublicationNativeRootFamily::FilesProvider)
            .count(),
        4
    );
    assert_eq!(
        out.family(PublicationNativeRootFamily::CcFeatureModel)
            .count(),
        0
    );
    for kind in [
        PublicationRowKind::Native,
        PublicationRowKind::Provider,
        PublicationRowKind::ConfiguredTarget,
        PublicationRowKind::Code,
    ] {
        assert_eq!(out.row_kind_count(kind), 2);
    }
    assert_eq!(out.row_kind_count(PublicationRowKind::Tuple), 0);
    assert_eq!((out.rows(), out.maximum_rows()), (8, 4));
    assert_eq!((out.edges(), out.maximum_edges()), (0, 0));
    assert_eq!((out.slots(), out.maximum_slots()), (8, 4));
    assert_eq!(out.maximum_arena_certificates(), 3);
    assert_eq!(out.maximum_arena_native_visits(), native);
    assert_eq!(out.maximum_local_visits(), native + sources + other);
    assert!(!out.feature_selected());
    assert_eq!(
        (
            out.feature_occurrences(),
            out.feature_arenas(),
            out.feature_wrapper_visits()
        ),
        (0, 0, 0)
    );
}
