//! Actual pinned provider declaration and initialized/raw calls. Minimal inert
//! fields isolate authentication; Analysis owns full File-based producer proof.
use std::path::PathBuf;

use compact_str::CompactString;
use slug_configuration_v2::CppFragmentProjection;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;
use slug_identity_v2::CanonicalLabel;
use starlark::environment::FrozenModule;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::tuple::TupleRef;

use super::*;
use crate::analysis_fragments::CppFragmentValue;
use crate::bzl_module::BzlModuleIdentity;
use crate::bzl_module::BzlModuleSourceProvenance;
use crate::package::loading_globals;
use crate::provider::BzlEvaluationContext;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../slug_analysis_v2/tests/fixtures/rustc-map-each/rules_cc/cc/private/rules_impl/cc_toolchain_info.bzl"
));
const FILE: &str = "cc_toolchain_info.bzl";
const CALLER: &str = "auth_test.bzl";

fn provenance(
    label: &str,
    digest: [u8; 32],
    filename: &str,
) -> (CompactString, BzlModuleSourceProvenance) {
    let identity = BzlModuleIdentity {
        label: CanonicalLabel::parse(label).unwrap(),
        workspace_path: PathBuf::from(filename),
        repository_mapping: Arc::from([]),
    };
    (
        filename.into(),
        BzlModuleSourceProvenance::new(identity, digest),
    )
}

fn definition(source: &str, label: &str, observed: [u8; 32], duplicate: bool) -> FrozenModule {
    let module = Module::new();
    let entry = provenance(label, observed, FILE);
    let mut entries = vec![entry.clone()];
    if duplicate {
        entries.push(entry.clone());
    }
    let context =
        BzlEvaluationContext::macro_runtime_context(entry.1.identity.clone(), entries.into());
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    eval.eval_module(
        AstModule::parse(FILE, source.to_owned(), &Dialect::Bazel).unwrap(),
        &loading_globals(),
    )
    .unwrap();
    drop(eval);
    module.freeze().unwrap()
}

fn cpp(module: &Module) {
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap();
    let projection =
        CppFragmentProjection::new(SlugConfiguration::default_target(&host).unwrap()).unwrap();
    module.set(
        "CPP",
        module
            .heap()
            .alloc_simple(CppFragmentValue::new(projection)),
    );
}

fn evaluate(module: &Module, body: &str, collect: bool) {
    let caller = provenance(
        "@@rules_cc+//cc/private:auth_test.bzl",
        Sha256::digest(body).into(),
        CALLER,
    );
    // Give every invocation the same permitted Cpp caller. This cannot promote
    // a provider defined with invalid provenance to authenticated authority.
    let context = BzlEvaluationContext::macro_runtime_context(
        caller.1.identity.clone(),
        Arc::from([provenance(LABEL, SHA, FILE), caller]),
    );
    let mut eval = Evaluator::new(module);
    eval.extra = Some(&context);
    eval.eval_module(
        AstModule::parse(CALLER, body.to_owned(), &Dialect::Bazel).unwrap(),
        &loading_globals(),
    )
    .unwrap();
    if collect {
        // Collect at supported statement safe points after the ordinary slots
        // have released the toolchain and constructor arguments. Only the
        // registry roots its runtime closures and their captured depsets.
        eval.eval_module(
            AstModule::parse(
                "allocation_pressure.bzl",
                "def churn():\n    for i in range(1000):\n        temporary = [str(i)] * 256\nchurn()\ncollected = True\n".to_owned(),
                &Dialect::Bazel,
            )
            .unwrap(),
            &loading_globals(),
        )
        .unwrap();
        assert!(
            module.heap().allocated_bytes() < module.heap().peak_allocated_bytes(),
            "allocation pressure must actually collect registry-only values"
        );
    }
}

fn invocation(cache_first: bool) -> String {
    // Read parameter names, never extract or rewrite a production body.
    let signature = SOURCE
        .split("def _create_cc_toolchain_info(")
        .nth(1)
        .unwrap()
        .split("):")
        .next()
        .unwrap();
    let names: Vec<_> = signature
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_suffix(','))
        .filter(|name| *name != "*" && !name.contains('='))
        .collect();
    assert_eq!(names.len(), 51);
    let cache = "E = [depset(order=o) for o in ['default','postorder','preorder','topological']]\n";
    let mut body = if cache_first {
        cache.to_owned()
    } else {
        String::new()
    };
    body.push_str(&format!(
        "args = {{ {} }}\n",
        names
            .iter()
            .map(|name| format!("'{name}':None"))
            .collect::<Vec<_>>()
            .join(",")
    ));
    body.push_str(r#"
args['cpp_configuration'] = CPP
args['toolchain_config_info'] = struct(compiler='gcc',target_libc='local',target_cpu='k8',target_system_name='proof',toolchain_id='proof',abi_version='',abi_libc_version='')
args['static_runtime_lib_depset'] = depset(['static'])
args['dynamic_runtime_lib_depset'] = depset(['dynamic'])
tc = CcToolchainInfo(**args)
# Leave runtime closures/captures reachable ONLY through the traced registry.
tc = None
args = None
"#);
    if !cache_first {
        body.push_str(cache);
    }
    body
}

fn captures(module: &Module) -> Vec<(CcToolchainCallableKind, Option<Vec<String>>)> {
    callable_registrations(module)
        .into_iter()
        .map(|(_, registration, capture)| {
            (
                registration.kind,
                (!capture.is_none()).then(|| {
                    crate::provider::StarlarkDepset::flatten(capture)
                        .unwrap()
                        .into_iter()
                        .map(|value| value.unpack_str().unwrap().to_owned())
                        .collect()
                }),
            )
        })
        .collect()
}

#[test]
fn actual_provider_definition_authenticates_only_pinned_source_identity_and_observation() {
    assert_eq!(<[u8; 32]>::from(Sha256::digest(SOURCE)), SHA);
    let changed = format!("{SOURCE}\n");
    let relocated = format!("\n{SOURCE}");
    for (source, label, digest, duplicate, expected) in [
        (SOURCE, LABEL, SHA, false, 3),
        (
            SOURCE,
            "@@foreign+//cc/private/rules_impl:cc_toolchain_info.bzl",
            SHA,
            false,
            0,
        ),
        (SOURCE, LABEL, [0; 32], false, 0),
        (changed.as_str(), LABEL, SHA, false, 0),
        (relocated.as_str(), LABEL, SHA, false, 0),
        (SOURCE, LABEL, SHA, true, 0),
    ] {
        let definition = definition(source, label, digest, duplicate);
        let module = Module::new();
        module.import_public_symbols(&definition);
        cpp(&module);
        evaluate(&module, &invocation(false), false);
        assert_eq!(
            callable_registrations(&module).len(),
            expected,
            "{label}, digest={digest:?}, duplicate={duplicate}, bytes={}",
            source.len()
        );
    }
}

#[test]
fn actual_initialized_raw_constructor_cannot_register_forged_callable_fields() {
    let definition = definition(SOURCE, LABEL, SHA, false);
    let module = Module::new();
    module.import_public_symbols(&definition);
    // Genuine private raw constructor, accessed by this Rust test only. Imported
    // heap ownership is rooted by import_public_symbols, including the raw value.
    let (raw, _) = definition.get_assigned("_").unwrap();
    module.set("Raw", raw.value());
    cpp(&module);
    evaluate(
        &module,
        r#"
def fake(*, feature_configuration): return depset()
forged = Raw(needs_pic_for_dynamic_libraries=fake,static_runtime_lib=fake,dynamic_runtime_lib=fake,
    _cpp_configuration=CPP,_static_runtime_lib_depset=depset(['static']),_dynamic_runtime_lib_depset=None)
def check():
    if forged.static_runtime_lib != fake: fail('raw constructor changed supplied function')
check()
"#,
        false,
    );
    assert!(callable_registrations(&module).is_empty());
    let fake = module.get("fake").unwrap();
    assert!(
        fake.request_value::<&dyn StarlarkFunctionIdentity>()
            .unwrap()
            .issued_identity()
            .is_none()
    );
}

#[test]
fn registry_roots_functions_and_captures_through_gc_freeze_alongside_empty_depsets() {
    for cache_first in [false, true] {
        let definition = definition(SOURCE, LABEL, SHA, false);
        let module = Module::new();
        module.import_public_symbols(&definition);
        cpp(&module);
        evaluate(&module, &invocation(cache_first), true);
        let entries = callable_registrations(&module);
        assert_eq!(entries.len(), 3);
        for (function, registration, _) in &entries {
            assert!(Arc::ptr_eq(
                function
                    .request_value::<&dyn StarlarkFunctionIdentity>()
                    .unwrap()
                    .issued_identity()
                    .unwrap(),
                &registration.identity,
            ));
        }
        let identities: Vec<_> = entries
            .iter()
            .map(|(_, row, _)| row.identity.clone())
            .collect();
        drop(entries);
        let expected = captures(&module);
        assert_eq!(
            expected.iter().map(|(kind, _)| *kind).collect::<Vec<_>>(),
            [
                CcToolchainCallableKind::SupportsPic,
                CcToolchainCallableKind::StaticRuntime,
                CcToolchainCallableKind::DynamicRuntime
            ]
        );
        assert_eq!(expected[0].1, None);
        assert_eq!(expected[1].1, Some(vec!["static".to_owned()]));
        assert_eq!(expected[2].1, Some(vec!["dynamic".to_owned()]));
        assert_eq!(
            TupleRef::from_value(crate::evaluator_roots::empty_depsets(&module).unwrap())
                .unwrap()
                .content()
                .len(),
            4
        );
        let frozen = module.freeze().unwrap();
        let rehydrated = Module::new();
        rehydrated.frozen_heap().add_reference(frozen.frozen_heap());
        rehydrated.set_extra_value(frozen.extra_value().unwrap().to_value());
        assert_eq!(captures(&rehydrated), expected);
        for ((function, registration, _), identity) in
            callable_registrations(&rehydrated).iter().zip(&identities)
        {
            assert!(Arc::ptr_eq(&registration.identity, identity));
            assert!(Arc::ptr_eq(
                function
                    .request_value::<&dyn StarlarkFunctionIdentity>()
                    .unwrap()
                    .issued_identity()
                    .unwrap(),
                identity,
            ));
        }
        assert_eq!(
            TupleRef::from_value(crate::evaluator_roots::empty_depsets(&rehydrated).unwrap())
                .unwrap()
                .content()
                .len(),
            4
        );
    }
}
