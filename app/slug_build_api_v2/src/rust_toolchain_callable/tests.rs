use super::*;
use crate::AnalysisArtifact;
use crate::AnalysisConfiguredTargetKey;
use crate::AnalysisDepset;
use crate::CcExecOs;
use crate::CcToolchainFeatures;
use crate::ConfiguredTargetValue;
use crate::DefaultInfo;
use crate::DepsetOrder;
use crate::ProviderCollection;
use crate::ProviderIdentity;
use crate::ProviderOccurrence;
use crate::ProviderValue;

fn module(name: &str) -> RustBzlModuleIdentity {
    RustBzlModuleIdentity {
        label: CanonicalLabel::parse(&format!("@@rules_rust+//rust/private:{name}.bzl")).unwrap(),
        workspace_path: PathBuf::from(format!("/workspace/rules_rust/rust/private/{name}.bzl")),
        repository_mapping: Arc::from([(
            ApparentRepoName::new("rules_cc").unwrap(),
            CanonicalRepoName::new("rules_cc+").unwrap(),
        )]),
    }
}
fn authority() -> Arc<RustToolchainCallableSourceAuthority> {
    let children = [module("common"), module("providers")];
    Arc::new(RustToolchainCallableSourceAuthority {
        caller: RustBzlModuleSource {
            identity: module("toolchain"),
            digest: [1; 32],
        },
        helper: RustToolchainHelperManifest {
            root: module("rust_allocator_libraries"),
            direct_children: children.clone().into(),
            reachable: children
                .map(|identity| RustBzlModuleSource {
                    identity,
                    digest: [2; 32],
                })
                .into(),
            fingerprint: [3; 32],
        },
    })
}
fn files(name: &str) -> AnalysisValue {
    AnalysisValue::depset(
        AnalysisDepset::new(
            DepsetOrder::Topological,
            vec![AnalysisValue::artifact(AnalysisArtifact::Source(
                CanonicalLabel::parse(&format!("@@rust_std+//lib:{name}.rlib")).unwrap(),
            ))],
            vec![],
        )
        .unwrap(),
    )
}
fn provider(value: AnalysisValue) -> AnalysisValue {
    AnalysisValue::provider(ProviderOccurrence::new(
        ProviderIdentity::builtin("ToolchainInfo"),
        [("capture", value)],
    ))
}
fn standard(configuration: u8, name: &str) -> AnalysisValue {
    AnalysisValue::configured_target(ConfiguredTargetValue::new(
        AnalysisConfiguredTargetKey::new(
            CanonicalLabel::parse("@@rust_std+//:stdlib").unwrap(),
            vec![configuration],
        ),
        ProviderCollection::new(vec![
            ProviderValue::DefaultInfo(DefaultInfo::empty()),
            ProviderValue::Occurrence(ProviderOccurrence::new(
                ProviderIdentity::builtin("ToolchainInfo"),
                [("files", files(name))],
            )),
        ])
        .unwrap(),
    ))
}
fn feature(name: &str) -> AnalysisValue {
    AnalysisValue::cc_feature_configuration(
        CcToolchainFeatures::new(vec![], vec![], vec![], "", CcExecOs::Linux)
            .unwrap()
            .configure_features(vec![name.into()])
            .unwrap(),
    )
}
fn model() -> RustToolchainCallableModel {
    RustToolchainCallableModel {
        source: authority(),
        identity: Arc::new(()),
        name_hash: 47,
        cc_toolchain: provider(files("runtime")),
        feature_configuration: feature("A"),
        experimental_link_std_dylib: AnalysisValue::boolean(false),
        rust_std: standard(1, "std"),
    }
}
fn value(model: RustToolchainCallableModel) -> AnalysisValue {
    AnalysisValue::rust_toolchain_callable(RustToolchainCallable::new(model).unwrap())
}
fn change_identity(identity: &mut RustBzlModuleIdentity, component: usize) {
    match component {
        0 => identity.label = module("changed").label,
        1 => identity.workspace_path.push("changed"),
        2 => {
            identity.repository_mapping = Arc::from([(
                ApparentRepoName::new("other").unwrap(),
                CanonicalRepoName::new("rules_cc+").unwrap(),
            )])
        }
        3 => {
            identity.repository_mapping = Arc::from([(
                ApparentRepoName::new("rules_cc").unwrap(),
                CanonicalRepoName::new("other+").unwrap(),
            )])
        }
        _ => unreachable!(),
    }
}

#[test]
fn authority_compares_full_manifest_even_when_fingerprint_is_unchanged() {
    let first = value(model());
    for owner in 0..4 {
        for component in 0..4 {
            let mut changed = model();
            let source = Arc::make_mut(&mut changed.source);
            let identity = match owner {
                0 => &mut source.caller.identity,
                1 => &mut source.helper.root,
                2 => &mut Arc::make_mut(&mut source.helper.direct_children)[0],
                3 => &mut Arc::make_mut(&mut source.helper.reachable)[0].identity,
                _ => unreachable!(),
            };
            change_identity(identity, component);
            assert!(
                !first.publication_eq(&value(changed)),
                "owner={owner} component={component}"
            );
        }
    }
    for component in 0..7 {
        let mut changed = model();
        let source = Arc::make_mut(&mut changed.source);
        match component {
            0 => source.caller.digest[31] ^= 1,
            1 => Arc::make_mut(&mut source.helper.reachable)[0].digest[31] ^= 1,
            2 => source.helper.fingerprint[31] ^= 1,
            3 => Arc::make_mut(&mut source.helper.direct_children).reverse(),
            4 => Arc::make_mut(&mut source.helper.reachable).reverse(),
            5 => source.helper.direct_children = Arc::from([]),
            6 => source.helper.reachable = Arc::from([]),
            _ => unreachable!(),
        }
        assert!(
            !first.publication_eq(&value(changed)),
            "component={component}"
        );
    }
    assert!(first.publication_eq(&value(model()))); // source A/B/A
}

#[test]
fn captures_configuration_and_name_hash_restore_without_occurrence_equality() {
    let first = value(model());
    for component in 0..8 {
        let mut changed = model();
        match component {
            0 => changed.cc_toolchain = AnalysisValue::none(),
            1 => changed.cc_toolchain = provider(files("other_runtime")),
            2 => changed.feature_configuration = AnalysisValue::none(),
            3 => changed.feature_configuration = feature("B"),
            4 => changed.experimental_link_std_dylib = AnalysisValue::boolean(true),
            5 => changed.rust_std = standard(2, "std"),
            6 => changed.rust_std = standard(1, "other_std"),
            7 => changed.name_hash += 1,
            _ => unreachable!(),
        }
        assert!(
            !first.publication_eq(&value(changed)),
            "component={component}"
        );
    }
    let restored = value(model());
    assert_ne!(first, restored);
    assert!(first.publication_eq(&restored)); // capture/configuration A/B/A
    assert_eq!(first, first.dupe());
    assert!(first.is_starlark_hashable());
    assert_eq!(first.value_type().as_str(), "function");
    assert_eq!(
        first.starlark_hash().unwrap(),
        first.dupe().starlark_hash().unwrap()
    );
    let AnalysisValueKind::RustToolchainCallable(callable) = first.kind() else {
        unreachable!()
    };
    let mut same_token = model();
    same_token.identity = callable.model().identity.clone();
    let same_token = value(same_token);
    assert_eq!(first, same_token);
    assert_eq!(
        first.starlark_hash().unwrap(),
        same_token.starlark_hash().unwrap()
    );
}

#[test]
fn callable_and_capture_aliases_share_one_complete_publication_bijection() {
    let a = value(model());
    let b = value(model());
    let shared = AnalysisValue::tuple(vec![a.dupe(), a]);
    let rebuilt = AnalysisValue::tuple(vec![b.dupe(), b]);
    let split = AnalysisValue::tuple(vec![value(model()), value(model())]);
    assert!(shared.publication_eq(&rebuilt));
    assert!(!shared.publication_eq(&split));
    assert!(!split.publication_eq(&shared));
    fn graph(shared_files: bool, shared_feature: bool, reverse: bool) -> AnalysisValue {
        let capture = files("runtime");
        let configuration = feature("A");
        let mut callable = model();
        callable.cc_toolchain = provider(AnalysisValue::tuple(vec![
            capture.dupe(),
            configuration.dupe(),
        ]));
        callable.feature_configuration = if shared_feature {
            configuration.dupe()
        } else {
            feature("A")
        };
        let outside = AnalysisValue::tuple(vec![
            if shared_files {
                capture
            } else {
                files("runtime")
            },
            configuration,
        ]);
        let callable = value(callable);
        let (first, second) = if reverse {
            (outside, callable)
        } else {
            (callable, outside)
        };
        provider(AnalysisValue::tuple(vec![first, second]))
    }
    for reverse in [false, true] {
        let shared = graph(true, true, reverse);
        let rebuilt = graph(true, true, reverse);
        assert!(shared.publication_eq(&rebuilt));
        for split in [graph(false, true, reverse), graph(true, false, reverse)] {
            assert!(!shared.publication_eq(&split));
            assert!(!split.publication_eq(&shared));
        }
    }
}

#[test]
fn constructor_checks_only_outer_shapes_preserving_helper_validation() {
    for field in 0..4 {
        for invalid in [
            AnalysisValue::none(),
            AnalysisValue::boolean(false),
            AnalysisValue::string("spoof"),
            AnalysisValue::list(vec![]),
            AnalysisValue::strukt([("type", AnalysisValue::string("Target"))]),
        ] {
            let expected = (field < 2 && matches!(invalid.kind(), AnalysisValueKind::None))
                || (field == 2 && matches!(invalid.kind(), AnalysisValueKind::Boolean(_)));
            let mut changed = model();
            match field {
                0 => changed.cc_toolchain = invalid,
                1 => changed.feature_configuration = invalid,
                2 => changed.experimental_link_std_dylib = invalid,
                3 => changed.rust_std = invalid,
                _ => unreachable!(),
            }
            assert_eq!(RustToolchainCallable::new(changed).is_ok(), expected);
        }
    }
    // Membership belongs to the original helper, not this finite data owner.
    let mut absent = model();
    absent.cc_toolchain = AnalysisValue::none();
    absent.feature_configuration = AnalysisValue::none();
    absent.rust_std = AnalysisValue::configured_target(ConfiguredTargetValue::new(
        AnalysisConfiguredTargetKey::new(CanonicalLabel::parse("@@//:empty").unwrap(), vec![1]),
        ProviderCollection::new(vec![ProviderValue::DefaultInfo(DefaultInfo::empty())]).unwrap(),
    ));
    assert!(RustToolchainCallable::new(absent).is_ok());
}
