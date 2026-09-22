use slug_configuration_v2::CommandConfigurationOccurrence;
use slug_configuration_v2::CommandConfigurationOverlay;
use slug_configuration_v2::NativeCommandOption;
use slug_identity_v2::CanonicalLabel;

use super::*;
use crate::ActionOutput;
use crate::ActionOutputKind;
use crate::AnalysisConfiguredTargetKey;
use crate::AnalysisDepset;
use crate::AnalysisValue;
use crate::ArtifactInputSource;
use crate::ArtifactInputs;
use crate::CppCompileData;
use crate::DepsetOrder;
use crate::DerivedArtifactRoot;
use crate::ProviderIdentity;
use crate::ProviderOccurrence;
use crate::artifact_root::test_support as roots;

fn source(label: &str) -> AnalysisArtifact {
    AnalysisArtifact::Source(CanonicalLabel::parse(label).unwrap())
}
fn file(name: &str) -> AnalysisArtifact {
    source(&format!("@@//pkg:{name}"))
}
fn inputs(values: &[AnalysisArtifact]) -> ArtifactInputs {
    ArtifactInputs::new(
        values
            .iter()
            .cloned()
            .map(ArtifactInputSource::Direct)
            .collect::<Vec<_>>(),
    )
}
fn directories(values: &[&str]) -> AnalysisValue {
    AnalysisValue::depset(
        AnalysisDepset::new(
            DepsetOrder::Default,
            values.iter().map(|s| AnalysisValue::string(*s)).collect(),
            vec![],
        )
        .unwrap(),
    )
}
fn context(system: &[&str], external: &[&str]) -> AnalysisValue {
    AnalysisValue::provider(ProviderOccurrence::new(
        ProviderIdentity::builtin("CcCompilationContext"),
        [
            ("system_includes", directories(system)),
            ("external_includes", directories(external)),
        ],
    ))
}
fn data() -> CppCompileData {
    let mut data = super::super::tests::data();
    data.compilation_context = context(&[], &[]);
    data
}
fn resolve(path: &str) -> Result<Option<AnalysisArtifact>, String> {
    Ok([
        file("source.cc"),
        file("header.h"),
        file("prunable.h"),
        file("cache"),
    ]
    .into_iter()
    .find(|a| a.path() == path))
}
fn paths(values: &[AnalysisArtifact]) -> Vec<String> {
    values.iter().map(|a| a.path().into_owned()).collect()
}
fn generated(owner: &str, root: DerivedArtifactRoot, kind: ActionOutputKind) -> AnalysisArtifact {
    AnalysisArtifact::Derived {
        owner: AnalysisConfiguredTargetKey::new(
            CanonicalLabel::parse(owner).unwrap(),
            b"config".as_slice(),
        ),
        output: ActionOutput::new(root, "pkg/generated.h", kind),
    }
}

#[test]
fn cpp_input_sets_preserve_roles_order_and_drop_unused_headers_after_dotd() {
    let recipe = CppCompileSpec::new(data()).unwrap();
    let before = recipe.clone();
    let sets = recipe.input_sets().unwrap();
    assert_eq!(
        paths(sets.mandatory()),
        [
            "pkg/cc",
            "pkg/builtin.h",
            "pkg/extra",
            "pkg/source.cc",
            "pkg/cache"
        ]
    );
    assert_eq!(paths(sets.additional()), ["pkg/header.h", "pkg/prunable.h"]);
    assert_eq!(
        paths(sets.spawn()),
        [
            "pkg/cc",
            "pkg/builtin.h",
            "pkg/extra",
            "pkg/source.cc",
            "pkg/header.h",
            "pkg/prunable.h"
        ]
    );
    assert_eq!(
        paths(sets.allowed_derived()),
        [
            "pkg/cc",
            "pkg/builtin.h",
            "pkg/extra",
            "pkg/source.cc",
            "pkg/cache",
            "pkg/prunable.h",
            "pkg/header.h"
        ]
    );
    for _ in 0..2 {
        let result = recipe
            .reconcile_dotd_posix(
                "/exec",
                b"ignored.o: pkg/source.cc pkg/header.h pkg/header.h\n",
                resolve,
            )
            .unwrap();
        assert_eq!(result.discovered(), &[file("header.h")]);
        assert_eq!(
            paths(result.post_discovery()),
            [
                "pkg/cc",
                "pkg/builtin.h",
                "pkg/extra",
                "pkg/source.cc",
                "pkg/cache",
                "pkg/header.h"
            ]
        );
        assert_eq!(recipe, before);
    }
}

#[test]
fn derived_lookup_preserves_first_owner_cross_root_and_repository_identity() {
    let first = generated("@@//pkg:first", roots::root(), ActionOutputKind::File);
    let second = generated("@@//pkg:second", roots::root(), ActionOutputKind::File);
    let other = generated(
        "@@repo+//pkg:other",
        roots::root_for(b"other-config"),
        ActionOutputKind::File,
    );
    let external = source("@@repo+//pkg:header.h");
    let mut data = data();
    data.construction.additional_prunable_inputs = inputs(&[first.clone(), other.clone()]);
    data.construction.declared_headers = inputs(&[second.clone(), external.clone()]);
    let recipe = CppCompileSpec::new(data).unwrap();
    let dotd = format!(
        "ignored: {} {} {} {}\n",
        first.path(),
        other.path(),
        external.path(),
        first.path()
    );
    let mut calls = Vec::new();
    let result = recipe
        .reconcile_dotd_posix("/exec", dotd.as_bytes(), |path| {
            calls.push(path.to_owned());
            Ok((path == external.path()).then(|| external.clone()))
        })
        .unwrap();
    assert_eq!(result.discovered(), &[first, other, external.clone()]);
    assert_eq!(calls, [external.path()]);
    assert!(!result.discovered().contains(&second));
}

#[test]
fn builtin_prefix_root_normalization_and_diagnostic_priority_are_explicit() {
    let recipe = CppCompileSpec::new(data()).unwrap();
    let result = recipe
        .reconcile_dotd_posix(
            "/exec/a/..",
            b"x: /include/a.h pkg/./sub/../header.h /exec/pkg/source.cc unknown.cppmap\n",
            resolve,
        )
        .unwrap();
    assert_eq!(result.discovered(), &[file("header.h")]);
    assert!(
        recipe
            .reconcile_dotd_posix("/exec", b"x: .cppmap\n", |_| Ok(None))
            .unwrap_err()
            .contains("unresolved inclusions")
    );
    assert!(
        recipe
            .reconcile_dotd_posix("/exec", b"x: pkg/.cppmap\n", |_| Ok(None))
            .unwrap()
            .discovered()
            .is_empty()
    );
    let resolved_map = source("@@//:.cppmap");
    assert!(
        recipe
            .reconcile_dotd_posix("/exec", b"x: .cppmap\n", |_| Ok(Some(resolved_map.clone())))
            .unwrap_err()
            .contains("undeclared inclusions")
    );
    assert_eq!(
        recipe
            .reconcile_dotd_posix("/", b"x: /pkg/header.h\n", resolve)
            .unwrap()
            .discovered(),
        &[file("header.h")],
    );
    assert!(
        recipe
            .reconcile_dotd_posix("/exec", b"x: ../escaped.h\n", resolve)
            .unwrap_err()
            .contains("/escaped.h")
    );
    let error = recipe
        .reconcile_dotd_posix("/exec", b"x: missing.h /include-other/no.h\n", resolve)
        .unwrap_err();
    assert!(error.contains("absolute path inclusions"), "{error}");
    assert!(error.contains("/include-other/no.h"));
    assert!(!error.contains("missing.h"));
    assert!(
        recipe
            .reconcile_dotd_posix("/exec", b"x: missing.h\n", resolve)
            .unwrap_err()
            .contains("unresolved inclusions")
    );
    assert!(
        recipe
            .reconcile_dotd_posix("relative", b"", resolve)
            .unwrap_err()
            .contains("absolute")
    );
    assert!(
        recipe
            .reconcile_dotd_posix("/exec\0", b"", resolve)
            .is_err()
    );
}

#[test]
fn disabled_validation_omits_path_problems_but_not_resolver_errors_or_forgery() {
    let mut data = data();
    data.construction.needs_include_validation = false;
    data.compilation_context = AnalysisValue::provider(ProviderOccurrence::empty(
        ProviderIdentity::builtin("CcCompilationContext"),
    ));
    let recipe = CppCompileSpec::new(data).unwrap();
    let found = source("@@//unknown:header.h");
    let result = recipe
        .reconcile_dotd_posix(
            "/exec",
            b"x: /outside.h missing.h unknown/header.h\n",
            |path| Ok((path == found.path()).then(|| found.clone())),
        )
        .unwrap();
    assert_eq!(result.discovered(), &[found]);
    assert_eq!(
        recipe
            .reconcile_dotd_posix("/exec", b"x: missing.h\n", |_| Err(
                "resolver failure".into()
            ))
            .unwrap_err(),
        "resolver failure"
    );
    for forged in [
        file("wrong.h"),
        generated("@@//pkg:owner", roots::root(), ActionOutputKind::File),
    ] {
        assert!(
            recipe
                .reconcile_dotd_posix("/exec", b"x: wanted.h\n", |_| Ok(Some(forged.clone())))
                .unwrap_err()
                .contains("non-source or mismatching")
        );
    }
    assert!(
        recipe
            .reconcile_dotd_posix("/exec", b"x: invalid\0.h\n", resolve)
            .is_err()
    );
    assert!(
        recipe
            .reconcile_dotd_posix("/exec", b"x: invalid\xff.h\n", resolve)
            .is_err()
    );
}

#[test]
fn final_validation_uses_declared_membership_directory_boundaries_and_module_exemptions() {
    let sys = source("@@//sys:header.h");
    let ext = source("@@ext+//inc:header.h");
    let module = file("module.pcm");
    let map = file("module.modulemap");
    let mut data = data();
    data.compilation_context = context(&["sys"], &["external/ext+/inc"]);
    let recipe = CppCompileSpec::new(data.clone()).unwrap();
    let all = [sys.clone(), ext.clone(), module, map];
    let dotd = format!("x: {}\n", paths(&all).join(" "));
    let result = recipe
        .reconcile_dotd_posix("/exec", dotd.as_bytes(), |path| {
            Ok(all.iter().find(|a| a.path() == path).cloned())
        })
        .unwrap();
    assert_eq!(result.discovered(), &all);
    let disallowed = source("@@//system:header.h");
    assert!(
        recipe
            .reconcile_dotd_posix("/exec", b"x: system/header.h\n", |_| Ok(Some(
                disallowed.clone()
            )))
            .unwrap_err()
            .contains("undeclared inclusions")
    );
    // The strict switch belongs to the action root, not toolchain_cpp's retained
    // producer configuration (left at default false in this discriminator).
    let config = data.outputs[0].root().configuration();
    let overlay: CommandConfigurationOverlay = vec![CommandConfigurationOccurrence::native(
        NativeCommandOption::StrictSystemIncludes,
        Some("true"),
        false,
    )]
    .into();
    let config = config
        .with_command_configuration(config.starlark_options().clone(), &overlay)
        .unwrap();
    let root = DerivedArtifactRoot::bin(config).unwrap();
    for output in &mut data.construction.outputs {
        *output = ActionOutput::new(root.clone(), output.root_relative_path(), output.kind());
    }
    let strict = CppCompileSpec::new(data.clone()).unwrap();
    assert!(
        strict
            .reconcile_dotd_posix("/exec", b"x: sys/header.h\n", |_| Ok(Some(sys.clone())))
            .unwrap_err()
            .contains("undeclared inclusions")
    );
    data.construction.toolchain_cpp = crate::RetainedCppFragment::new(
        CppFragmentProjection::new(root.configuration().clone()).unwrap(),
    );
    for output in &mut data.construction.outputs {
        *output = ActionOutput::new(roots::root(), output.root_relative_path(), output.kind());
    }
    let nonstrict = CppCompileSpec::new(data).unwrap();
    assert_eq!(
        nonstrict
            .reconcile_dotd_posix("/exec", b"x: sys/header.h\n", |_| Ok(Some(sys.clone())))
            .unwrap()
            .discovered(),
        &[sys]
    );
}

#[test]
fn malformed_context_and_nonregular_inputs_fail_closed() {
    let mut data = data();
    for value in [
        None,
        Some(AnalysisValue::list(vec![])),
        Some(AnalysisValue::depset(
            AnalysisDepset::new(
                DepsetOrder::Default,
                vec![AnalysisValue::integer(1)],
                vec![],
            )
            .unwrap(),
        )),
    ] {
        let mut fields = vec![("external_includes", directories(&[]))];
        if let Some(value) = value {
            fields.push(("system_includes", value));
        }
        data.compilation_context = AnalysisValue::provider(ProviderOccurrence::new(
            ProviderIdentity::builtin("CcCompilationContext"),
            fields,
        ));
        let recipe = CppCompileSpec::new(data.clone()).unwrap();
        assert!(
            recipe
                .reconcile_dotd_posix("/exec", b"", resolve)
                .unwrap_err()
                .contains("system_includes")
        );
    }
    for kind in [ActionOutputKind::Directory, ActionOutputKind::RunfilesTree] {
        let mut data = self::data();
        data.construction.declared_headers =
            inputs(&[generated("@@//pkg:tree", roots::root(), kind)]);
        let recipe = CppCompileSpec::new(data).unwrap();
        assert!(recipe.input_sets().unwrap_err().contains("regular Files"));
        assert!(
            recipe
                .reconcile_dotd_posix("/exec", b"", resolve)
                .unwrap_err()
                .contains("regular Files")
        );
    }
}

#[test]
fn identical_source_exec_paths_do_not_replace_canonical_membership() {
    let declared = source("@@//pkg:sub/header.h");
    let resolved = source("@@//pkg/sub:header.h");
    assert_eq!(declared.path(), resolved.path());
    assert_ne!(declared, resolved);
    let mut data = data();
    data.construction.declared_headers = inputs(&[declared.clone()]);
    let recipe = CppCompileSpec::new(data).unwrap();
    let bytes = b"x: pkg/sub/header.h\n";
    assert!(
        recipe
            .reconcile_dotd_posix("/exec", bytes, |_| Ok(Some(resolved.clone())))
            .unwrap_err()
            .contains("undeclared inclusions")
    );
    assert_eq!(
        recipe
            .reconcile_dotd_posix("/exec", bytes, |_| Ok(Some(declared.clone())))
            .unwrap()
            .discovered(),
        &[declared]
    );
}
