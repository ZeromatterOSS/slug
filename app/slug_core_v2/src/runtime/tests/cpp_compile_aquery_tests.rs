use slug_analysis_v2::ConfiguredActionAspectProvenance;
use slug_analysis_v2::ConfiguredActionOwnerContext;
use slug_analysis_v2::ConfiguredCppCompileActionView;
use slug_analysis_v2::ConfiguredExecGroup;
use slug_analysis_v2::ConfiguredNodeResult;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_analysis_v2::PlatformSemanticFact;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::ArtifactInputSource;
use slug_build_api_v2::ArtifactInputs;
use slug_build_api_v2::CppCompileData;
use slug_build_api_v2::CppCompileSpec;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::RetainedArtifactInputs;
use slug_query_v2::AqueryScope;

use super::*;

mod original_sources {
    include!("../../../../slug_analysis_v2/tests/rustc_map_each/sources.rs");
    pub(super) fn include_files() -> impl Iterator<Item = (&'static str, &'static str)> {
        RUSTC_MAP_EACH_SOURCES.iter().copied().filter(|(p, _)| {
            p.starts_with("rules_cc/cc/private/compile/")
                || p.starts_with("rules_cc/cc/private/link/")
                || p.starts_with("rules_cc/cc/private/toolchain_config/")
                || (p.starts_with("rules_cc/cc/common/")
                    && !matches!(
                        p.rsplit('/').next().unwrap(),
                        "cc_helper.bzl" | "visibility.bzl" | "objc_info.bzl"
                    ))
                || (p.starts_with("rules_cc/cc/private/") && p.matches('/').count() == 3)
                || matches!(
                    *p,
                    "rules_cc/cc/action_names.bzl"
                        | "rules_cc/cc/cc_toolchain_config_lib.bzl"
                        | "rules_cc/cc/private/rules_impl/cc_toolchain_info.bzl"
                        | "rules_cc/cc/private/rules_impl/native_cc_common.bzl"
                        | "bazel_skylib/lib/paths.bzl"
                        | "cc_compatibility_proxy/symbols.bzl"
                )
        })
    }
}

// Authored inputs to the real restricted binder; provider/context/config bodies
// and the existing authored toolchain helper are unchanged. No compiler runs.
const CPP_DEFS: &str = r#"load('//cc/common:cc_common.bzl','cc_common')
load('//cc/private:cc_internal.bzl','cc_internal')
load('//cc/private:toolchain_proof.bzl','make_compile_toolchain')
load('//cc:cc_toolchain_config_lib.bzl','feature','flag_set','flag_group','env_set','env_entry')
def _file(target):
    return target[DefaultInfo].files.to_list()[0]
def _impl(ctx):
    probe=feature(name='query_probe',enabled=True,
        flag_sets=[flag_set(actions=['c-compile','c++-compile'],flag_groups=[flag_group(flags=['--probe','','--same','--same','a b',"'literal'",'--revision=%{revision}'])])],
        env_sets=[env_set(actions=['c-compile','c++-compile'],env_entries=[env_entry(key='FEATURE_ONLY',value='%{bad_env}')])])
    tc=make_compile_toolchain(ctx,compile_requirements=['requires-network'],compile_features=[probe])
    features=cc_common.configure_features(ctx=ctx,cc_toolchain=tc,requested_features=[],unsupported_features=[])
    context=cc_common.create_compilation_context(headers=depset([_file(ctx.attr.header)]))
    variables=cc_internal.cc_toolchain_variables(vars={'revision':ctx.attr.revision,'bad_env':17,
        'preprocessor_defines':[],'quote_include_paths':[],'include_paths':[],
        'system_include_paths':[],'framework_include_paths':[]})
    first=ctx.actions.declare_file('before')
    last=ctx.actions.declare_file('after')
    ctx.actions.write(first,'before')
    outputs=[first]
    for index,src in enumerate([_file(ctx.attr.source),_file(ctx.attr.c_source)]):
        obj=ctx.actions.declare_file('obj'+str(index)+'.o')
        dotd=ctx.actions.declare_file('obj'+str(index)+'.d')
        cc_internal.create_cc_compile_action(action_construction_context=ctx,configuration=ctx.configuration,
            cc_compilation_context=context,cc_toolchain=tc,feature_configuration=features,source=src,
            compile_build_variables=variables,toolchain_type='@bazel_tools//tools/cpp:toolchain_type',
            output_file=obj,dotd_file=dotd,additional_compilation_inputs=[_file(ctx.attr.extra)],
            additional_compilation_inputs_set=depset([_file(ctx.attr.extra)]),cache_key_inputs=depset([_file(ctx.attr.cache)]),
            needs_include_validation=True)
        outputs.extend([obj,dotd])
    ctx.actions.write(last,'after')
    outputs.append(last)
    return [DefaultInfo(files=depset(outputs))]
compile_rule=rule(implementation=_impl,fragments=['cpp'],attrs={
    'source':attr.label(allow_single_file=True),'c_source':attr.label(allow_single_file=True),
    'extra':attr.label(allow_single_file=True),'cache':attr.label(allow_single_file=True),
    'header':attr.label(allow_single_file=True),'static_file':attr.label(allow_single_file=True),
    'dynamic_file':attr.label(allow_single_file=True),'runtime':attr.string(default='none'),
    'object_prefix':attr.string(default=''),'object_extension':attr.string(default='.o'),
    'unused':attr.string(default='unused'),'revision':attr.string()})
def _wrapper(ctx):
    out=ctx.actions.declare_file('wrapper')
    ctx.actions.write(out,'wrapper')
    return [DefaultInfo(files=depset([out]))]
wrapper=rule(implementation=_wrapper,attrs={'dep':attr.label(cfg='exec')})
"#;

// Genuine builtin mapping, with the same finite Local dependency completion used
// by existing authenticated fixtures; these declarations do not supply callbacks.
const CPP_MODULES: &[(&str, &str)] = &[
    ("rules_license", "1.0.0"),
    ("buildozer", "8.5.1"),
    ("platforms", "1.0.0"),
    ("zlib", "1.3.1.bcr.5"),
    ("bazel_features", "1.42.1"),
    ("protobuf", "33.4"),
    ("rules_java", "9.1.0"),
    ("rules_cc", "0.2.17"),
    ("rules_python", "1.7.0"),
    ("rules_shell", "0.6.1"),
    ("apple_support", "1.24.2"),
    ("rules_apple", "4.1.0"),
    ("rules_swift", "3.1.2"),
    ("abseil-cpp", "20250814.1"),
    ("bazel_skylib", "1.0"),
    ("cc_compatibility_proxy", "1.0"),
];

const CPP_SELECTED: &[&str] = &["rules_cc", "bazel_skylib", "cc_compatibility_proxy"];
fn cpp_module(name: &str, version: &str) -> String {
    let mut contents = format!("module(name='{name}',version='{version}')\n");
    let dependencies: &[&str] = match name {
        "rules_cc" => &["cc_compatibility_proxy", "bazel_skylib", "platforms"],
        "cc_compatibility_proxy" => &["rules_cc"],
        _ => &[],
    };
    for dependency in dependencies {
        let version = CPP_MODULES
            .iter()
            .find(|(name, _)| name == dependency)
            .unwrap()
            .1;
        contents.push_str(&format!(
            "bazel_dep(name='{dependency}',version='{version}')\n"
        ));
    }
    contents
}
// Same finite selected-registry fixture protocol as the authentic Analysis
// compile_context/registry.rs. Supply existing MODULE bytes and metadata only;
// actual normal requests select the immutable route over the unchanged bodies.
struct CppRegistry;
#[async_trait::async_trait]
impl slug_bzlmod_v2::RegistryIo for CppRegistry {
    async fn read_exact(
        &self,
        url: &slug_bzlmod_v2::RegistryFileUrl,
    ) -> Result<slug_bzlmod_v2::RegistryIoOutcome, slug_bzlmod_v2::RegistryTransportError> {
        for name in CPP_SELECTED {
            let version = CPP_MODULES.iter().find(|(n, _)| n == name).unwrap().1;
            let prefix = format!("https://registry.invalid/modules/{name}/{version}/");
            let bytes = match url.as_str().strip_prefix(&prefix) {
                Some("MODULE.bazel") => cpp_module(name, version).into_bytes(),
                Some("source.json") => format!(
                    r#"{{"url":"https://origin.invalid/{name}.tgz","integrity":"sha256-test"}}"#
                )
                .into_bytes(),
                _ => continue,
            };
            return Ok(slug_bzlmod_v2::RegistryIoOutcome::Found(bytes.into()));
        }
        Ok(slug_bzlmod_v2::RegistryIoOutcome::NotFound)
    }
}
fn cpp_dice() -> Arc<Dice> {
    let mut builder = Dice::builder();
    slug_bzlmod_v2::install_registry_io(&mut builder, Arc::new(CppRegistry));
    builder.build(DetectCycles::Enabled)
}

fn cpp_epoch(revision: i64, marker: &str) -> PathObservationEpoch {
    let mut epoch = BuildRootEpoch::base(revision);
    let mut module =
        "module(name='root')\nregister_execution_platforms('//:platform')\n".to_owned();
    for &(name, version) in CPP_MODULES {
        epoch.node(
            &format!("/workspace/{name}"),
            PathNodeKind::Directory,
            revision,
        );
        let contents = cpp_module(name, version);
        epoch.file(
            &format!("/workspace/{name}/MODULE.bazel"),
            &contents,
            revision,
        );
        epoch.missing(&format!("/workspace/{name}/REPO.bazel"));
        epoch.missing(&format!("/workspace/{name}/.bazelignore"));
        if !CPP_SELECTED.contains(&name) {
            module.push_str(&format!(
                "local_path_override(module_name='{name}',path='{name}')\n"
            ));
        }
        if matches!(
            name,
            "rules_cc"
                | "bazel_skylib"
                | "cc_compatibility_proxy"
                | "bazel_features"
                | "rules_apple"
                | "rules_swift"
                | "abseil-cpp"
        ) {
            module.push_str(&format!("bazel_dep(name='{name}',version='{version}')\n"));
        }
    }
    epoch.file("/workspace/MODULE.bazel", &module, revision);
    let files = original_sources::include_files().collect::<Vec<_>>();
    assert_eq!(files.len(), 44);
    assert_eq!(
        files
            .iter()
            .map(|(p, _)| p)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        44
    );
    for (path, body) in files.into_iter().chain([
        (
            "rules_cc/cc/private/toolchain_proof.bzl",
            include_str!("../../../../slug_analysis_v2/tests/rustc_map_each/cc_toolchain.bzl"),
        ),
        ("rules_cc/cc/private/query_compile.bzl", CPP_DEFS),
    ]) {
        let absolute = format!("/workspace/{path}");
        let parent = absolute.rsplit_once('/').unwrap().0;
        let mut ancestor = Path::new(parent);
        while ancestor != Path::new("/workspace") {
            epoch.node(
                ancestor.to_str().unwrap(),
                PathNodeKind::Directory,
                revision,
            );
            ancestor = ancestor.parent().unwrap();
        }
        epoch.package(parent.strip_prefix("/workspace/").unwrap(), "", revision);
        epoch.file(&absolute, body, revision);
    }
    for path in [
        "source-a.cc",
        "source-a.c",
        "extra.h",
        "cache.h",
        "header.h",
        "static.a",
        "dynamic.so",
    ] {
        epoch.file(&format!("/workspace/{path}"), "inert", revision);
    }
    epoch.package("",&format!("load('@rules_cc//cc/private:query_compile.bzl','compile_rule','wrapper')\nplatform(name='platform')\nexports_files(['source-a.cc','source-a.c','extra.h','cache.h','header.h','static.a','dynamic.so'])\ncompile_rule(name='compile',source='source-a.cc',c_source='source-a.c',extra='extra.h',cache='cache.h',header='header.h',static_file='static.a',dynamic_file='dynamic.so',revision='{marker}')\nwrapper(name='wrapper',dep=':compile')\n"),revision);
    epoch.build()
}

async fn cpp_transaction(dice: &Arc<Dice>, revision: i64, marker: &str) -> dice::DiceTransaction {
    let base = cpp_epoch(revision, marker);
    let mut observations = base
        .observations()
        .iter()
        .map(|(d, v)| (d.clone(), v.clone()))
        .collect::<Vec<_>>();
    for (index, name) in CPP_SELECTED.iter().enumerate() {
        let root = PathBuf::from(format!("/workspace/{name}"));
        let namespace = slug_workspace_v2::PathObservationNamespace::Materialization(
            slug_workspace_v2::PathObservationInstanceId::new(9180 + index as u64),
        );
        for (demand, value) in base.observations() {
            let path = demand.path().as_path();
            if path.starts_with(&root) || root.starts_with(path) {
                observations.push((
                    slug_workspace_v2::PathObservationDemand::new(
                        namespace,
                        demand.path().clone(),
                        demand.operation(),
                    ),
                    value.clone(),
                ));
            }
        }
    }
    let transaction = build_root_transaction(
        dice,
        PathObservationEpoch::from_shared(observations).unwrap(),
    )
    .await;
    let mut updater = transaction.into_updater();
    inject_root_module_request_inputs(
        &mut updater,
        Path::new("/workspace"),
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    slug_bzlmod_v2::inject_registry_request_inputs(
        &mut updater,
        Path::new("/workspace"),
        slug_bzlmod_v2::RegistryUrls::new(["https://registry.invalid"]),
        slug_bzlmod_v2::RegistryRequestGeneration(0),
    )
    .unwrap();
    let mut entries = CPP_MODULES
        .iter()
        .filter(|(name, _)| !CPP_SELECTED.contains(name))
        .map(|&(name, _)| {
            let mut attributes = SmallMap::new();
            attributes.insert(
                "path".into(),
                slug_bzlmod_v2::OverrideAttributeValue::String(name.into()),
            );
            slug_bzlmod_v2::RepositoryMaterializationEpochEntry {
                request: Arc::new(RepositoryMaterializationRequest {
                    id: RepositoryMaterializationRequestId {
                        workspace: workspace.clone(),
                        canonical_repo: slug_identity_v2::CanonicalRepoName::new(format!(
                            "{name}+"
                        ))
                        .unwrap(),
                    },
                    repo_spec: slug_bzlmod_v2::RepoSpec {
                        rule_id: slug_bzlmod_v2::RepoRuleId {
                            bzl_file: CanonicalLabel::parse(
                                "@@bazel_tools//tools/build_defs/repo:local.bzl",
                            )
                            .unwrap(),
                            rule_name: "local_repository".into(),
                        },
                        attributes: Arc::new(attributes),
                    },
                    kind: slug_bzlmod_v2::RepositoryMaterializationKind::Local {
                        logical_root: NormalizedAbsolutePath::new(format!("/workspace/{name}"))
                            .unwrap(),
                    },
                }),
                result: slug_bzlmod_v2::RepositoryMaterializationResult::Success(
                    slug_bzlmod_v2::RepositoryMaterializationSuccess::Local,
                ),
            }
        })
        .collect::<Vec<_>>();
    // Root mapping first observes these finite Local module requests. Complete
    // them before asking the normal selected routes for immutable requests.
    updater
        .changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: workspace.dupe(),
            },
            RepositoryMaterializationResultEpoch::new(workspace.dupe(), entries.clone()).unwrap(),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    for (index, name) in CPP_SELECTED.iter().enumerate() {
        let route = transaction
            .compute(
                &slug_bzlmod_v2::RootRepositoryRouteObservationKey::for_root_build(
                    workspace.dupe(),
                    slug_identity_v2::ApparentRepoName::new(*name).unwrap(),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let slug_bzlmod_v2::SourcePreparationOutcome::Complete(Ok(route)) = route else {
            panic!("selected compile fixture route: {route:?}")
        };
        let route = route.result().as_ref().as_ref().unwrap();
        let slug_bzlmod_v2::HostRepositoryMaterializationDisposition::Request(request) =
            slug_bzlmod_v2::host_repository_materialization_request(&route.source_capability())
                .unwrap()
        else {
            panic!("selected compile source did not request materialization")
        };
        entries.push(slug_bzlmod_v2::RepositoryMaterializationEpochEntry {
            request,
            result: slug_bzlmod_v2::RepositoryMaterializationResult::Success(
                slug_bzlmod_v2::RepositoryMaterializationSuccess::Immutable {
                    source_identity: Arc::from("sha256-test"),
                    generation_root: PathBuf::from(format!("/workspace/{name}")),
                    observation_instance: slug_workspace_v2::PathObservationInstanceId::new(
                        9180 + index as u64,
                    ),
                },
            ),
        });
    }
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(
            RepositoryMaterializationResultEpochKey {
                workspace: workspace.clone(),
            },
            RepositoryMaterializationResultEpoch::new(workspace.dupe(), entries).unwrap(),
        )])
        .unwrap();
    updater.commit().await
}
fn cpp_key(target: &str) -> BuildCommandRootKey {
    let base = build_test_configuration("query");
    let overlay: slug_configuration_v2::CommandConfigurationOverlay = vec![
        slug_configuration_v2::CommandConfigurationOccurrence::native(
            slug_configuration_v2::NativeCommandOption::ActionEnv,
            Some("FIXED=a b"),
            false,
        ),
        slug_configuration_v2::CommandConfigurationOccurrence::native(
            slug_configuration_v2::NativeCommandOption::ActionEnv,
            Some("INHERITED_ONLY"),
            false,
        ),
    ]
    .into();
    let configuration = base
        .slug_configuration()
        .unwrap()
        .with_command_configuration(base.starlark_options().clone(), &overlay)
        .unwrap();
    BuildCommandRootKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        &[TargetPattern::parse(target).unwrap()],
        ConfigurationKey::from_slug(configuration),
    )
    .unwrap()
}
fn cpp_evaluation(outcome: &BuildCommandOutcome) -> &BuildCommandEvaluation {
    let slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) = outcome else {
        panic!("Cpp fixture retained Needs: {outcome:?}")
    };
    value.as_ref().as_ref().unwrap()
}
fn cpp_owner(evaluation: &BuildCommandEvaluation) -> &Arc<ConfiguredNodeResult> {
    evaluation
        .action_closure
        .owners()
        .iter()
        .find(|o| o.actions().iter().any(|a| a.cpp_compile_spec().is_some()))
        .unwrap()
}
fn cpp_text(evaluation: &BuildCommandEvaluation, scope: AqueryScope) -> String {
    crate::runtime::format_aquery_text_output_for_scope(evaluation, scope).unwrap()
}
fn cpp_records(text: &str) -> Vec<&str> {
    assert!(text.ends_with("\n\n"));
    text.split_inclusive("\n\n")
        .map(|r| r.strip_suffix('\n').unwrap())
        .collect()
}

#[tokio::test]
async fn cpp_compile_aquery_real_constructor_fields_scopes_and_tool_owners() {
    let dice = cpp_dice();
    let mut transaction = cpp_transaction(&dice, 1, "A").await;
    for (target, tool, count) in [("//:compile", false, 4), ("//:wrapper", true, 5)] {
        let outcome = transaction.compute(&cpp_key(target)).await.unwrap();
        let evaluation = cpp_evaluation(&outcome);
        let owner = cpp_owner(evaluation);
        assert_eq!(
            owner
                .actions()
                .iter()
                .map(|a| a.mnemonic())
                .collect::<Vec<_>>(),
            ["FileWrite", "CppCompile", "CppCompile", "FileWrite"]
        );
        let deps = cpp_text(evaluation, AqueryScope::Deps);
        let literal = cpp_text(evaluation, AqueryScope::Literal);
        let blocks = cpp_records(&deps);
        assert_eq!(blocks.len(), count);
        assert_eq!(cpp_records(&literal).len(), if tool { 1 } else { 4 });
        for (index, action) in owner
            .actions()
            .iter()
            .filter(|a| a.cpp_compile_spec().is_some())
            .enumerate()
        {
            let view = ConfiguredCppCompileActionView::from_configured_action(action).unwrap();
            assert!(std::ptr::eq(view.action(), action));
            assert!(std::ptr::eq(
                view.spec(),
                action.cpp_compile_spec().unwrap()
            ));
            assert!(std::ptr::eq(view.context(), action.context().as_ref()));
            assert!(std::ptr::eq(view.owner(), action.context().owner()));
            assert!(std::ptr::eq(
                view.execution_platform(),
                action.context().execution_platform().unwrap()
            ));
            assert!(std::ptr::eq(view.data(), view.spec().data()));
            assert!(std::ptr::eq(view.source(), &view.data().source));
            assert!(std::ptr::eq(
                view.object_output(),
                view.spec().object_output()
            ));
            assert!(std::ptr::eq(view.dotd_output(), view.spec().dotd_output()));
            assert_eq!(view.context().exec_group(), &ConfiguredExecGroup::Default);
            let [ArtifactInputSource::Depset(mandatory)] = view.mandatory_spawn_inputs().sources()
            else {
                panic!("native producer retained one mandatory depset")
            };
            assert_eq!(mandatory.order(), DepsetOrder::Postorder);
            let mut has_source = false;
            mandatory
                .visit(|a| has_source |= a == view.source())
                .unwrap();
            assert!(has_source);
            assert!(std::ptr::eq(
                view.mandatory_spawn_inputs(),
                view.spec().mandatory_spawn_inputs()
            ));
            assert!(std::ptr::eq(
                view.cache_key_inputs(),
                view.spec().cache_key_inputs()
            ));
            assert!(std::ptr::eq(
                view.declared_headers(),
                view.spec().declared_headers()
            ));
            if !tool {
                assert_eq!(view.data().environment.fixed().get("FIXED"), Some("a b"));
                assert!(
                    view.data()
                        .environment
                        .inherited()
                        .contains("INHERITED_ONLY")
                );
            }
            assert_eq!(
                view.owner().configuration().kind() == slug_analysis_v2::ConfigurationKind::Exec,
                tool
            );
            let source = if index == 0 {
                "source-a.cc"
            } else {
                "source-a.c"
            };
            let args = view.spec().expand_command_line().unwrap();
            assert_eq!(
                args.iter().map(|v| v.as_str()).collect::<Vec<_>>(),
                [
                    "toolchain/bin/gcc",
                    "--probe",
                    "",
                    "--same",
                    "--same",
                    "a b",
                    "'literal'",
                    "--revision=A"
                ]
            );
            assert!(
                view.spec()
                    .effective_environment(&Default::default())
                    .is_err(),
                "query must not expand bad feature environment"
            );
            let mut outputs = vec![view.object_output().path(), view.dotd_output().path()];
            outputs.sort();
            let cfg = view
                .owner()
                .configuration()
                .slug_configuration()
                .unwrap()
                .projection()
                .display_token();
            let mut env = view
                .data()
                .environment
                .fixed()
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>();
            env.sort();
            let info = if index == 0 {
                "  ExecutionInfo: {requires-network: ''}\n"
            } else {
                ""
            };
            let expected = format!(
                "action 'Compiling {source}{}'\n  Mnemonic: CppCompile\n  Target: //:compile\n  Configuration: {cfg}\n  Execution platform: //:platform\n  Inputs: [cache.h, dynamic.so, extra.h, header.h, {source}, static.a]\n  Outputs: [{}]\n  Environment: [{}]\n  Command Line: (exec toolchain/bin/gcc \\\n    --probe \\\n    '' \\\n    --same \\\n    --same \\\n    'a b' \\\n    ''\\''literal'\\''' \\\n    '--revision=A')\n# Configuration: {cfg}\n# Execution platform: //:platform\n{info}",
                if tool { " [for tool]" } else { "" },
                outputs.join(", "),
                env.join(", ")
            );
            assert_eq!(
                blocks.iter().filter(|b| **b == expected).count(),
                1,
                "{deps}"
            );
            for absent in [
                "  IsExecutable:",
                "  FileWriteContents:",
                "  ActionKey:",
                "  SlugActionToken:",
                "FEATURE_ONLY",
                "INHERITED_ONLY",
                "PWD=",
                "(TreeArtifact)",
            ] {
                assert!(!expected.contains(absent));
            }
        }
        let wanted = evaluation
            .action_closure
            .owners()
            .iter()
            .flat_map(|o| o.actions().iter().map(|a| a.mnemonic()))
            .collect::<Vec<_>>();
        let actual = blocks
            .iter()
            .map(|b| {
                b.lines()
                    .find_map(|l| l.strip_prefix("  Mnemonic: "))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, wanted);
    }
}

#[tokio::test]
async fn cpp_compile_aquery_restores_held_results_and_owned_text() {
    let dice = cpp_dice();
    let key = cpp_key("//:wrapper");
    let mut tx = cpp_transaction(&dice, 10, "A").await;
    let first = tx.compute(&key).await.unwrap();
    let snapshot = cpp_owner(cpp_evaluation(&first)).clone();
    let text = cpp_text(cpp_evaluation(&first), AqueryScope::Deps);
    drop(tx);
    let mut tx = cpp_transaction(&dice, 11, "B").await;
    let changed = tx.compute(&key).await.unwrap();
    assert!(cpp_text(cpp_evaluation(&changed), AqueryScope::Deps).contains("--revision=B"));
    assert_ne!(cpp_owner(cpp_evaluation(&changed)), &snapshot);
    assert_eq!(cpp_owner(cpp_evaluation(&first)), &snapshot);
    drop(changed);
    drop(tx);
    let mut tx = cpp_transaction(&dice, 12, "A").await;
    let restored = tx.compute(&key).await.unwrap();
    assert_eq!(cpp_owner(cpp_evaluation(&restored)), &snapshot);
    assert_eq!(cpp_text(cpp_evaluation(&restored), AqueryScope::Deps), text);
    drop(restored);
    drop(first);
    drop(snapshot);
    drop(tx);
    drop(dice);
    assert_eq!(cpp_records(&text).len(), 5);
    assert!(text.contains("--revision=A"));
    assert!(!text.contains("--revision=B"));
}

fn cpp_node(
    original: &ConfiguredNodeResult,
    specs: Vec<ActionSpec>,
    resolved: bool,
) -> Arc<ConfiguredNodeResult> {
    let key = original.configured_target_key().unwrap().clone();
    let previous = original.actions()[0].context();
    let context = if resolved {
        ConfiguredActionOwnerContext::new(
            key.clone(),
            ConfiguredExecGroup::Default,
            previous.execution_platform().unwrap().clone(),
            PlatformSemanticFact {
                exec_properties: Arc::from([
                    ("owner-only".into(), "a b".into()),
                    ("shared".into(), "owner-value".into()),
                ]),
                missing_toolchain_error: None,
            },
            &Default::default(),
            &Default::default(),
            Vec::new(),
            None,
            ConfiguredActionAspectProvenance::Absent,
        )
        .unwrap()
    } else {
        ConfiguredActionOwnerContext::unresolved_default(key.clone()).unwrap()
    };
    Arc::new(
        ConfiguredNodeResult::new_rule(
            key,
            original.providers().clone(),
            None,
            original.runfiles_packages().clone(),
        )
        .with_action_specs(specs, vec![Arc::new(context)])
        .unwrap(),
    )
}
fn cpp_replaced(
    evaluation: &BuildCommandEvaluation,
    node: Arc<ConfiguredNodeResult>,
) -> BuildCommandEvaluation {
    let mut targets = evaluation.targets.to_vec();
    targets[0].analysis = Some(node.clone());
    BuildCommandEvaluation {
        workspace: evaluation.workspace().clone(),
        anchor: evaluation.anchor.clone(),
        targets: targets.into(),
        action_closure: ValidatedActionClosure::new(Arc::from([node])).unwrap(),
    }
}
fn cpp_spec(data: CppCompileData) -> ActionSpec {
    ActionSpec::cpp_compile(CppCompileSpec::new(data).unwrap())
}

#[tokio::test]
async fn cpp_compile_aquery_metadata_guards_environment_identity_and_priority() {
    let dice = cpp_dice();
    let mut tx = cpp_transaction(&dice, 30, "A").await;
    let outcome = tx.compute(&cpp_key("//:compile")).await.unwrap();
    let evaluation = cpp_evaluation(&outcome);
    let original = cpp_owner(evaluation);
    let data = original.actions()[1]
        .cpp_compile_spec()
        .unwrap()
        .data()
        .clone();
    let key = original.configured_target_key().unwrap();
    let root = data.outputs[0].root().clone();
    let direct = |values: Vec<AnalysisArtifact>| {
        ArtifactInputs::new(
            values
                .into_iter()
                .map(ArtifactInputSource::Direct)
                .collect::<Vec<_>>(),
        )
    };
    let source =
        |s: &str| AnalysisArtifact::Source(CanonicalLabel::parse(&format!("@@//:{s}")).unwrap());
    // Synthetic flat mandatory order, not a claim that native source is first.
    let mut modified = data.clone();
    modified.construction.source = source("%{output}.cc");
    modified.construction.outputs = vec![
        ActionOutput::new(root.clone(), "obj-%{input}.o", ActionOutputKind::File),
        ActionOutput::new(root.clone(), "obj.d", ActionOutputKind::File),
    ]
    .into();
    let other_key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:independent").unwrap(),
        ConfigurationKey::from_slug(
            key.configuration()
                .slug_configuration()
                .unwrap()
                .to_exec()
                .unwrap(),
        ),
    );
    let other_root = slug_build_api_v2::DerivedArtifactRoot::bin(
        other_key
            .configuration()
            .slug_configuration()
            .unwrap()
            .clone(),
    )
    .unwrap();
    let constant = AnalysisArtifact::Derived {
        owner: other_key.artifact_owner(),
        output: ActionOutput::new_constant(other_root.clone(), "constant.h"),
    };
    let equal_path = constant.path().into_owned();
    let equal_source = source(&equal_path);
    modified.construction.mandatory_spawn_inputs = direct(vec![
        source("first.h"),
        modified.source.clone(),
        source("first.h"),
    ]);
    modified.construction.cache_key_inputs = direct(vec![
        source("cache-only.h"),
        constant.clone(),
        equal_source.clone(),
    ]);
    modified.construction.declared_headers =
        direct(vec![source("declared-only.h"), constant.clone()]);
    modified.construction.additional_prunable_inputs = direct(vec![source("prunable-only.h")]);
    modified.construction.execution_requirements =
        slug_configuration_v2::CanonicalStringMap::from_pairs([
            ("shared", "recipe-value"),
            ("recipe-only", "z value"),
        ]);
    let node = cpp_node(original, vec![cpp_spec(modified.clone())], true);
    let altered = cpp_replaced(evaluation, node.clone());
    let text = cpp_text(&altered, AqueryScope::Deps);
    let block = cpp_records(&text)[0];
    assert!(block.starts_with("action 'Compiling obj-first.h.o.cc'\n"));
    let input_line = block.lines().find(|l| l.starts_with("  Inputs:")).unwrap();
    assert_eq!(
        input_line.matches(&equal_path).count(),
        2,
        "distinct identities with equal display"
    );
    for path in ["first.h", "cache-only.h", "declared-only.h"] {
        assert!(input_line.contains(path));
    }
    assert_eq!(input_line.matches("first.h").count(), 1);
    assert!(!input_line.contains("prunable-only.h"));
    assert!(block.contains(
        "  ExecutionInfo: {owner-only: 'a b', recipe-only: 'z value', shared: recipe-value}\n"
    ));
    for omitted in ["FEATURE_ONLY", "INHERITED_ONLY", "PWD=", "(Constant)"] {
        assert!(!block.contains(omitted));
    }
    let saved = node.as_ref().clone();
    for _ in 0..2 {
        assert_eq!(cpp_text(&altered, AqueryScope::Deps), text);
        assert_eq!(node.as_ref(), &saved);
    }
    let base_spec = cpp_spec(data.clone());
    let unresolved = cpp_node(original, vec![base_spec.clone()], false);
    assert!(
        ConfiguredCppCompileActionView::from_configured_action(&unresolved.actions()[0])
            .unwrap_err()
            .contains("selected execution platform")
    );
    for mode in 0..10 {
        let mut bad = data.clone();
        match mode {
            0 => bad.construction.mandatory_spawn_inputs = direct(vec![]),
            1 => bad.construction.mandatory_spawn_inputs = direct(vec![source("missing-source.h")]),
            2 | 3 => {
                bad.construction.mandatory_spawn_inputs =
                    ArtifactInputs::new(vec![ArtifactInputSource::Depset(
                        RetainedArtifactInputs::new(
                            slug_build_api_v2::AnalysisDepset::new(
                                if mode == 2 {
                                    DepsetOrder::Preorder
                                } else {
                                    DepsetOrder::Topological
                                },
                                vec![slug_build_api_v2::AnalysisValue::artifact(
                                    data.source.clone(),
                                )],
                                Vec::new(),
                            )
                            .unwrap(),
                        )
                        .unwrap(),
                    )])
            }
            4 => {
                bad.construction.mandatory_spawn_inputs = ArtifactInputs::new(vec![
                    ArtifactInputSource::Direct(data.source.clone()),
                    ArtifactInputSource::Depset(
                        RetainedArtifactInputs::new(slug_build_api_v2::AnalysisDepset::empty(
                            DepsetOrder::Default,
                        ))
                        .unwrap(),
                    ),
                ])
            }
            5 => {
                bad.construction.outputs = vec![
                    ActionOutput::new_constant(root.clone(), "out.o"),
                    ActionOutput::new(root.clone(), "out.d", ActionOutputKind::File),
                ]
                .into()
            }
            6 => {
                bad.construction.cache_key_inputs = direct(vec![AnalysisArtifact::Derived {
                    owner: other_key.artifact_owner(),
                    output: ActionOutput::new(root.clone(), "wrong.h", ActionOutputKind::File),
                }])
            }
            7 => {
                bad.construction.cache_key_inputs = direct(vec![AnalysisArtifact::Derived {
                    owner: other_key.artifact_owner(),
                    output: ActionOutput::new(
                        other_root.clone(),
                        "dir",
                        ActionOutputKind::Directory,
                    ),
                }])
            }
            8 => {
                bad.construction.declared_headers = direct(vec![AnalysisArtifact::Derived {
                    owner: other_key.artifact_owner(),
                    output: ActionOutput::new(
                        other_root.clone(),
                        "symlink.h",
                        ActionOutputKind::Symlink,
                    ),
                }])
            }
            _ => {
                bad.construction.source = AnalysisArtifact::Derived {
                    owner: other_key.artifact_owner(),
                    output: ActionOutput::new(root.clone(), "wrong.cc", ActionOutputKind::File),
                };
                bad.construction.mandatory_spawn_inputs = direct(vec![bad.source.clone()]);
            }
        }
        let rejected = cpp_node(original, vec![cpp_spec(bad)], true);
        let snapshot = rejected.as_ref().clone();
        let expected = match mode {
            0 | 1 => "configured CppCompile mandatory inputs must contain its source",
            2..=4 => "configured CppCompile mandatory input order is not admitted",
            5 => "configured CppCompile query requires its ordinary object and dotd roots",
            6 | 9 => "configured CppCompile input root differs from its own owner",
            _ => "configured CppCompile query requires Source or Derived regular File inputs",
        };
        assert_eq!(
            ConfiguredCppCompileActionView::from_configured_action(&rejected.actions()[0])
                .unwrap_err(),
            expected,
            "mode {mode}"
        );
        assert_eq!(rejected.as_ref(), &snapshot);
    }
    let custom = cpp_node(
        original,
        vec![
            base_spec
                .clone()
                .with_exec_properties(std::collections::BTreeMap::from([(
                    "custom".to_owned(),
                    "v".to_owned(),
                )])),
        ],
        true,
    );
    assert_eq!(
        ConfiguredCppCompileActionView::from_configured_action(&custom.actions()[0]).unwrap_err(),
        "configured CppCompile query requires default action group and properties"
    );
    let mut generated = data.clone();
    generated.construction.source = AnalysisArtifact::Derived {
        owner: other_key.artifact_owner(),
        output: ActionOutput::new(other_root.clone(), "generated.cc", ActionOutputKind::File),
    };
    generated.construction.mandatory_spawn_inputs = direct(vec![generated.source.clone()]);
    let generated = cpp_node(original, vec![cpp_spec(generated)], true);
    assert!(
        cpp_text(&cpp_replaced(evaluation, generated), AqueryScope::Deps)
            .starts_with("action 'Compiling generated.cc'\n")
    );
    // Later structural validation wins over both earlier rendering and lazy argv errors.
    let mut lazy = data.clone();
    lazy.construction.outputs = vec![
        ActionOutput::new(root.clone(), "lazy.o", ActionOutputKind::File),
        ActionOutput::new(root.clone(), "lazy.d", ActionOutputKind::File),
    ]
    .into();
    lazy.variables = slug_build_api_v2::cc_variables::CcToolchainVariables::new(
        slug_build_api_v2::cc_variables::CcVariablesModel {
            kind: slug_build_api_v2::cc_variables::CcVariablesKind::Map,
            occurrence: slug_build_api_v2::cc_variables::CcVariablesOccurrence::new(),
            parent: None,
            locals: Box::new([]),
            cache: Box::new([]),
        },
    );
    let lazy = cpp_spec(lazy);
    assert!(
        lazy.cpp_compile_spec()
            .unwrap()
            .expand_command_line()
            .is_err()
    );
    let early = ActionSpec::new(
        ActionKind::Write {
            content: String::new(),
            is_executable: true,
        },
        "FileWrite",
        vec![ActionOutput::new(
            root.clone(),
            "early'quoted",
            ActionOutputKind::File,
        )],
    );
    for earlier in [early, lazy] {
        let mut bad = data.clone();
        bad.construction.mandatory_spawn_inputs = direct(vec![]);
        let rejected = cpp_node(original, vec![earlier, cpp_spec(bad)], true);
        if matches!(rejected.actions()[0].kind(), ActionKind::Write { .. }) {
            let early_view = ResolvedFileWriteSemanticView::from_configured_action(
                slug_analysis_v2::ConfiguredActionView::from_configured_action(
                    &rejected.actions()[0],
                )
                .unwrap(),
            );
            assert_eq!(
                crate::runtime::format_file_write_aquery_text(&early_view),
                Err("FileWrite aquery text requires a normalized relative output")
            );
        }
        let expected =
            ConfiguredCppCompileActionView::from_configured_action(&rejected.actions()[1])
                .unwrap_err();
        let rejected = cpp_replaced(evaluation, rejected);
        for scope in [AqueryScope::Literal, AqueryScope::Deps] {
            assert_eq!(
                crate::runtime::format_aquery_text_output_for_scope(&rejected, scope),
                Err(expected)
            );
        }
    }
}
