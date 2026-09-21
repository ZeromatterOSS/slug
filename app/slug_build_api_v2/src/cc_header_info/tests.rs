use slug_identity_v2::CanonicalLabel;

use super::*;
use crate::ActionOutput;
use crate::ActionOutputKind;
use crate::AnalysisConfiguredTargetKey;
use crate::AnalysisValue;
use crate::artifact_root::test_support as root_support;

fn source(name: &str) -> AnalysisArtifact {
    AnalysisArtifact::Source(CanonicalLabel::parse(&format!("@@//:{name}")).unwrap())
}

fn files(names: &[&str]) -> Box<[AnalysisArtifact]> {
    names.iter().map(|name| source(name)).collect()
}

fn node(names: &[&str], deps: Vec<CcHeaderInfo>) -> CcHeaderInfo {
    CcHeaderInfo::create(
        CcHeaderInfoInput {
            modular_public_headers: files(names),
            deps: deps.into(),
            ..Default::default()
        },
        false,
        &[],
    )
    .unwrap()
}

fn derived(owner: &str, kind: ActionOutputKind) -> AnalysisArtifact {
    AnalysisArtifact::Derived {
        owner: AnalysisConfiguredTargetKey::new(CanonicalLabel::parse(owner).unwrap(), vec![1]),
        output: ActionOutput::new(root_support::root(), "same.pcm", kind),
    }
}

#[test]
fn normalization_preserves_independent_order_and_host_suffix_semantics() {
    for windows in [false, true] {
        let info = CcHeaderInfo::create(
            CcHeaderInfoInput {
                modular_public_headers: files(&["a.h", "move.inc", "a.h", "upper.INC", "shared.h"]),
                modular_private_headers: files(&["shared.h", "private.inc", "a.h", "move.inc"]),
                textual_headers: files(&["explicit.inc", "explicit.inc", "a.h"]),
                separate_module_headers: files(&["s.inc", "s.inc", "a.h"]),
                ..Default::default()
            },
            windows,
            &[],
        )
        .unwrap();
        assert_eq!(
            info.model().modular_public_headers,
            if windows {
                files(&["a.h", "shared.h"])
            } else {
                files(&["a.h", "upper.INC", "shared.h"])
            }
        );
        assert_eq!(
            info.model().modular_private_headers,
            files(&["shared.h", "a.h"])
        );
        assert_eq!(
            info.model().textual_headers,
            if windows {
                files(&[
                    "explicit.inc",
                    "a.h",
                    "move.inc",
                    "upper.INC",
                    "private.inc",
                ])
            } else {
                files(&["explicit.inc", "a.h", "move.inc", "private.inc"])
            }
        );
        assert_eq!(
            info.model().separate_module_headers,
            files(&["s.inc", "s.inc", "a.h"])
        );
    }
}

#[test]
fn merge_replaces_deps_and_only_unions_three_header_lists() {
    let old = node(&["old.h"], vec![]);
    let base = node(&["base.h"], vec![old]);
    let exported = CcHeaderInfo::create(
        CcHeaderInfoInput {
            modular_public_headers: files(&["export.h", "base.h"]),
            modular_private_headers: files(&["private.h"]),
            textual_headers: files(&["text.inc"]),
            separate_module_headers: files(&["not-merged.h"]),
            ..Default::default()
        },
        false,
        &[],
    )
    .unwrap();
    let ordinary = node(&["ordinary.h"], vec![]);
    let result = base
        .with_deps(
            vec![ordinary.clone(), ordinary.clone()].into(),
            &[exported],
            false,
        )
        .unwrap();
    assert_ne!(result, base);
    assert_eq!(result.model().deps.as_ref(), &[ordinary.clone(), ordinary]);
    assert_eq!(
        result.model().modular_public_headers,
        files(&["base.h", "export.h"])
    );
    assert_eq!(
        result.model().modular_private_headers,
        files(&["private.h"])
    );
    assert_eq!(result.model().textual_headers, files(&["text.inc"]));
    assert!(result.model().separate_module_headers.is_empty());
}

#[test]
fn module_slots_require_derived_and_matching_main_but_allow_trees() {
    for slot in 0..4 {
        let mut input = CcHeaderInfoInput::default();
        let slots = [
            &mut input.header_module,
            &mut input.pic_header_module,
            &mut input.separate_module,
            &mut input.separate_pic_module,
        ];
        *slots.into_iter().nth(slot).unwrap() = Some(source("source.pcm"));
        assert!(
            CcHeaderInfo::create(input, false, &[])
                .unwrap_err()
                .contains("derived File")
        );
    }
    for pic in [false, true] {
        let mut input = CcHeaderInfoInput::default();
        let module = Some(derived("@@//:owner", ActionOutputKind::Directory));
        if pic {
            input.separate_pic_module = module;
        } else {
            input.separate_module = module;
        }
        assert!(
            CcHeaderInfo::create(input, false, &[])
                .unwrap_err()
                .contains("without main module")
        );
    }
    let module = Some(derived("@@//:owner", ActionOutputKind::Directory));
    let info = CcHeaderInfo::create(
        CcHeaderInfoInput {
            header_module: module.clone(),
            pic_header_module: module.clone(),
            separate_module: module.clone(),
            separate_pic_module: module,
            ..Default::default()
        },
        false,
        &[],
    )
    .unwrap();
    assert!(info.model().separate_pic_module.is_some());
}

#[test]
fn publication_discriminates_hidden_payload_owner_and_global_aliases() {
    let a = node(&["a.h"], vec![]);
    let b = node(&["a.h"], vec![]);
    let changed = node(&["b.h"], vec![]);
    let wrap = |value| AnalysisValue::cc_header_info(value);
    let left = node(&[], vec![a.clone(), a.clone()]);
    let same = node(&[], vec![b.clone(), b.clone()]);
    assert!(wrap(left.clone()).publication_eq(&wrap(same.clone())));
    assert!(!wrap(left.clone()).publication_eq(&wrap(node(&[], vec![a.clone(), b.clone()]))));
    assert!(!wrap(left.clone()).publication_eq(&wrap(node(&[], vec![changed.clone(), changed]))));
    assert!(
        AnalysisValue::tuple(vec![wrap(left.clone()), wrap(a.clone())]).publication_eq(
            &AnalysisValue::tuple(vec![wrap(same.clone()), wrap(b.clone())])
        )
    );
    assert!(
        !AnalysisValue::tuple(vec![wrap(left), wrap(a.clone())])
            .publication_eq(&AnalysisValue::tuple(vec![wrap(same), wrap(a)]))
    );
    let module_node = |owner| {
        CcHeaderInfo::create(
            CcHeaderInfoInput {
                header_module: Some(derived(owner, ActionOutputKind::File)),
                ..Default::default()
            },
            false,
            &[],
        )
        .unwrap()
    };
    assert!(
        !wrap(node(&[], vec![module_node("@@//:a")]))
            .publication_eq(&wrap(node(&[], vec![module_node("@@//:b")])))
    );
}

#[test]
fn publication_memoizes_shared_diamonds_without_collapsing_fresh_nodes() {
    fn diamond() -> CcHeaderInfo {
        let mut current = node(&["leaf.h"], vec![]);
        for _ in 0..28 {
            current = node(&[], vec![current.clone(), current]);
        }
        current
    }
    let a = AnalysisValue::cc_header_info(diamond());
    let b = AnalysisValue::cc_header_info(diamond());
    assert_ne!(a, b);
    assert!(a.publication_eq(&b));
    assert!(a.is_starlark_hashable());
    assert_eq!(
        a.starlark_hash().unwrap(),
        a.clone().starlark_hash().unwrap()
    );
}

#[test]
fn publication_checks_every_module_and_header_sequence() {
    let make = |changed: Option<usize>| {
        let mut input = CcHeaderInfoInput {
            header_module: Some(derived("@@//:a", ActionOutputKind::File)),
            pic_header_module: Some(derived("@@//:a", ActionOutputKind::File)),
            separate_module: Some(derived("@@//:a", ActionOutputKind::File)),
            separate_pic_module: Some(derived("@@//:a", ActionOutputKind::File)),
            modular_public_headers: files(&["a.h", "b.h"]),
            modular_private_headers: files(&["a.h", "b.h"]),
            textual_headers: files(&["a.h", "b.h"]),
            separate_module_headers: files(&["a.h", "b.h"]),
            ..Default::default()
        };
        if let Some(slot) = changed {
            if slot < 4 {
                let slots = [
                    &mut input.header_module,
                    &mut input.pic_header_module,
                    &mut input.separate_module,
                    &mut input.separate_pic_module,
                ];
                *slots.into_iter().nth(slot).unwrap() =
                    Some(derived("@@//:b", ActionOutputKind::File));
            } else {
                let slots = [
                    &mut input.modular_public_headers,
                    &mut input.modular_private_headers,
                    &mut input.textual_headers,
                    &mut input.separate_module_headers,
                ];
                *slots.into_iter().nth(slot - 4).unwrap() = files(&["b.h", "a.h"]);
            }
        }
        AnalysisValue::cc_header_info(CcHeaderInfo::create(input, false, &[]).unwrap())
    };
    assert!(make(None).publication_eq(&make(None)));
    for slot in 0..8 {
        assert!(!make(None).publication_eq(&make(Some(slot))), "slot {slot}");
    }
}

#[test]
fn publication_uses_bounded_call_stack_for_deep_dependency_chains() {
    let mut left = vec![node(&["leaf.h"], vec![])];
    let mut right = vec![node(&["leaf.h"], vec![])];
    for _ in 0..16_384 {
        left.push(node(&[], vec![left.last().unwrap().clone()]));
        right.push(node(&[], vec![right.last().unwrap().clone()]));
    }
    assert!(
        left.last()
            .unwrap()
            .publication_eq_with(right.last().unwrap(), &mut PublicationEqState::default())
    );
    // Keep each intermediate root alive, isolating comparison from Arc teardown.
    while left.pop().is_some() {}
    while right.pop().is_some() {}
}
