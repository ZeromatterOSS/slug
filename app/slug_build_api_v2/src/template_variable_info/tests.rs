use super::*;
use crate::AnalysisDepset;
use crate::AnalysisValue;
use crate::DepsetOrder;
use crate::ProviderCollection;
use crate::ProviderIdentity;
use crate::ProviderOccurrence;
use crate::ProviderValue;

fn info(entries: &[(&str, &str)]) -> TemplateVariableInfo {
    TemplateVariableInfo::new(
        entries
            .iter()
            .map(|(key, value)| (CompactString::new(key), CompactString::new(value)))
            .collect(),
    )
}

#[test]
fn native_identity_uses_one_shared_allocation_even_for_empty_maps() {
    assert_eq!(
        std::mem::size_of::<TemplateVariableInfo>(),
        std::mem::size_of::<usize>()
    );
    let first = info(&[]);
    let alias = first.dupe();
    let fresh = info(&[]);
    assert_eq!(first, alias);
    assert_ne!(first, fresh);
    assert_eq!(fxhash::hash64(&first), fxhash::hash64(&alias));
    let keys: SmallMap<_, _> = [(first, 1), (alias, 2), (fresh, 3)].into_iter().collect();
    assert_eq!(keys.len(), 2);
    assert_eq!(keys.values().copied().collect::<Vec<_>>(), [2, 3]);
}

#[test]
fn publication_compares_ordered_full_payload_not_native_identity_or_map_eq() {
    let a = info(&[("z", "first"), ("é", "second")]);
    let restored = info(&[("z", "first"), ("é", "second")]);
    let reversed = info(&[("é", "second"), ("z", "first")]);
    assert_eq!(a.variables(), reversed.variables());
    assert_ne!(a, restored);
    let a = AnalysisValue::template_variable_info(a);
    assert!(a.publication_eq(&AnalysisValue::template_variable_info(restored)));
    assert!(!a.publication_eq(&AnalysisValue::template_variable_info(reversed)));
    assert!(
        !a.publication_eq(&AnalysisValue::template_variable_info(info(&[
            ("z", "first"),
            ("é", "changed")
        ])))
    );
    assert!(a.is_starlark_hashable());
    assert!(a.is_starlark_immutable());
    assert!(a.is_valid_depset_leaf());
}

fn providers(direct: TemplateVariableInfo, nested: TemplateVariableInfo) -> ProviderCollection {
    ProviderCollection::from_values(
        vec![
            ProviderValue::TemplateVariableInfo(direct),
            ProviderValue::Occurrence(ProviderOccurrence::new(
                ProviderIdentity::builtin("ToolchainInfo"),
                [("nested", AnalysisValue::template_variable_info(nested))],
            )),
        ],
        false,
    )
    .unwrap()
}

#[test]
fn publication_bijection_spans_direct_and_nested_provider_roots_both_directions() {
    let first = info(&[("A", "1")]);
    let second = info(&[("A", "1")]);
    let third = info(&[("A", "1")]);
    let shared = providers(first.clone(), first);
    let restored = providers(second.clone(), second.clone());
    let split = providers(second, third);
    assert_eq!(shared, restored);
    assert_ne!(shared, split);
    assert_ne!(split, shared);
    assert_eq!(
        shared
            .template_variable_info()
            .unwrap()
            .variables()
            .get("A")
            .unwrap(),
        "1"
    );
}

#[test]
fn native_dictionary_and_depset_identity_do_not_weaken_nested_publication() {
    let a = AnalysisValue::template_variable_info(info(&[("A", "1")]));
    let b = AnalysisValue::template_variable_info(info(&[("A", "1")]));
    let c = AnalysisValue::template_variable_info(info(&[("A", "1")]));
    let graph = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![a.clone(), a.clone(), b.clone()],
        vec![],
    )
    .unwrap();
    assert_eq!(graph.to_list().len(), 2);
    let map = AnalysisValue::dictionary([
        (a.clone(), AnalysisValue::none()),
        (b.clone(), AnalysisValue::none()),
    ])
    .unwrap();
    assert!(
        matches!(map.kind(), crate::AnalysisValueKind::Dictionary(entries) if entries.len() == 2)
    );

    let same = AnalysisValue::dictionary([(a.clone(), a.clone())]).unwrap();
    let restored = AnalysisValue::dictionary([(b.clone(), b.clone())]).unwrap();
    let split = AnalysisValue::dictionary([(b.clone(), c.clone())]).unwrap();
    assert!(same.publication_eq(&restored));
    assert!(!same.publication_eq(&split));
    assert!(!split.publication_eq(&same));

    let nested = |first: AnalysisValue, second: AnalysisValue| {
        AnalysisValue::tuple(vec![
            first,
            AnalysisValue::depset(
                AnalysisDepset::new(DepsetOrder::Default, vec![second], vec![]).unwrap(),
            ),
        ])
    };
    assert!(nested(a.clone(), a.clone()).publication_eq(&nested(b.clone(), b.clone())));
    assert!(!nested(a.clone(), a).publication_eq(&nested(b, c)));
}
