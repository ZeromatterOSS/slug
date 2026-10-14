//! Unchanged public finalize registers the finite callback on the generic Spawn.
use slug_build_api_v2::actions::RetainedSpawnResourceSet;

use super::super::cc_configure_features::config;
use super::*;

const PROOF: &str = r#"
load('//cc:cc_toolchain_config_lib.bzl', 'feature', 'flag_set', 'flag_group', 'action_config', 'tool')
load('//cc/private:cc_internal.bzl', 'cc_internal')
load('//cc/private/toolchain_config:cc_toolchain_config_info.bzl', 'create_cc_toolchain_config_info')
load('//cc/private/link:finalize_link_action.bzl', 'finalize_link_action')
load('//cc/private/link:target_types.bzl', 'LINK_TARGET_TYPE', 'LINKING_MODE')
ACTION = 'c++-link-static-library'

def _finalize(ctx, configuration, tc):
    actions = cc_internal.wrap_link_actions(ctx.actions)
    output = actions.declare_file('archive.a')
    finalize_link_action(actions = actions, mnemonic = 'CppArchive', action_name = ACTION,
        link_type = LINK_TARGET_TYPE.STATIC_LIBRARY, linking_mode = LINKING_MODE.STATIC,
        stamping = False, feature_configuration = configuration, cc_toolchain = tc,
        progress_message = 'archive:MARKER', object_file_inputs = [], non_code_inputs = [],
        libraries_to_link = [], linkstamp_map = {}, linkstamp_object_artifacts = [],
        linkstamp_object_file_inputs = [], user_link_flags = [], additional_linker_inputs = depset(),
        additional_build_variables = {'stamp': 'MARKER:' + tc._cpp_configuration.compilation_mode()},
        output = output, interface_output = None, dynamic_library_solib_symlink_output = None,
        action_outputs = [output], use_test_only_flags = False, whole_archive = False,
        native_deps = False, additional_linkstamp_defines = [], lto_mapping = {}, allow_lto_indexing = False)
    return output
child = subrule(implementation = _finalize)
def _impl(ctx):
    flags = [flag_group(flags = ['--stamp=%{stamp}', '--params=%{linker_param_file}'])]
    probe = feature(name = 'probe', flag_sets = [flag_set(actions = [ACTION], flag_groups = flags)])
    features = [feature(name = 'no_legacy_features'), feature(name = 'archive_param_file'),
        feature(name = 'gcc_quoting_for_param_files'), probe]
    info = create_cc_toolchain_config_info(ctx = ctx, toolchain_identifier = 'resource-proof', compiler = 'gcc',
        features = features, action_configs = [action_config(action_name = ACTION, tools = [tool(path = 'runner')])])
    configuration = cc_internal.cc_toolchain_features(toolchain_config_info = info, tools_directory = '.').configure_features(
        [ACTION, 'probe', 'archive_param_file', 'gcc_quoting_for_param_files'])
    tc = struct(_cpp_configuration = ctx.fragments.cpp, _solib_dir = '_solib_k8',
        _build_variables_dict = {}, _supports_param_files = True, _ar_files = depset(), _linker_files = depset())
    output = child(configuration, tc) if ctx.attr.sub else _finalize(ctx, configuration, tc)
    return [DefaultInfo(files = depset([output]))]
subject = rule(implementation = _impl, fragments = ['cpp'], attrs = {'sub': attr.bool()}, subrules = [child], exec_groups = {'cpp_link': exec_group()})
"#;
fn source(root: &std::path::Path, marker: &str) {
    fs::write(
        root.join("rules_cc/cc/private/resource_proof.bzl"),
        PROOF.replace("MARKER", marker),
    )
    .unwrap();
}
fn assert_registration(result: &ConfiguredNodeResult, marker: &str, mode: &str) {
    let [action] = result.actions() else {
        panic!("one existing Spawn owner")
    };
    assert_eq!(action.exec_group(), Some("cpp_link"));
    let spawn = action.spawn_spec().unwrap();
    let recipe = spawn.resource_set().expect("finite resource descriptor");
    assert_eq!(recipe.source(), RetainedSpawnResourceSet::SOURCE_LABEL);
    assert_eq!(recipe.digest(), &RetainedSpawnResourceSet::SOURCE_DIGEST);
    assert_eq!(
        spawn.render_argv().unwrap(),
        ["runner".to_owned(), format!("--stamp={marker}:{mode}")]
    );
    let expanded = spawn.expand_forced_param_files().unwrap();
    assert_eq!(
        expanded.param_files()[0].bytes(),
        format!("--stamp={marker}:{mode}\n").as_bytes()
    );
    assert_eq!(
        spawn.progress_message(),
        Some(format!("archive:{marker}").as_str())
    );
}
#[tokio::test]
async fn unchanged_finalize_registers_resource_metadata_and_restores_source_configuration() {
    for observed in [false, true] {
        let root = super::workspace_with(
            "load('@rules_cc//cc/private:resource_proof.bzl','subject')\n",
            "subject(name='subject')\nsubject(name='sub',sub=True)\n",
        );
        source(&root, "A");
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let fast = config(&[]);
        let first = super::request_configured(
            &dice,
            &root,
            root_epoch(&root),
            observed,
            "subject",
            fast.clone(),
        )
        .await
        .unwrap();
        assert_registration(&first, "A", "fastbuild");
        let dbg = config(&[(NativeCommandOption::CompilationMode, "dbg")]);
        let configured =
            super::request_configured(&dice, &root, root_epoch(&root), observed, "subject", dbg)
                .await
                .unwrap();
        assert_registration(&configured, "A", "dbg");
        assert_ne!(configured, first);
        assert_eq!(
            super::request_configured(
                &dice,
                &root,
                root_epoch(&root),
                observed,
                "subject",
                fast.clone()
            )
            .await
            .unwrap(),
            first
        );
        source(&root, "B");
        let changed = super::request_configured(
            &dice,
            &root,
            root_epoch(&root),
            observed,
            "subject",
            fast.clone(),
        )
        .await
        .unwrap();
        assert_registration(&changed, "B", "fastbuild");
        assert_ne!(changed, first);
        source(&root, "A");
        assert_eq!(
            super::request_configured(
                &dice,
                &root,
                root_epoch(&root),
                observed,
                "subject",
                fast.clone()
            )
            .await
            .unwrap(),
            first
        );
        let pinned_path = root.join("rules_cc/cc/private/link/finalize_link_action.bzl");
        let pinned_source = fs::read(&pinned_path).unwrap();
        let mut changed_source = pinned_source.clone();
        changed_source.extend_from_slice(b"\n# changed source certificate\n");
        fs::write(&pinned_path, changed_source).unwrap();
        let error = super::request_configured(
            &dice,
            &root,
            root_epoch(&root),
            observed,
            "subject",
            fast.clone(),
        )
        .await
        .unwrap_err();
        assert!(
            error.contains("callable resource_set is not supported"),
            "{error}"
        );
        fs::write(&pinned_path, pinned_source).unwrap();
        assert_eq!(
            super::request_configured(
                &dice,
                &root,
                root_epoch(&root),
                observed,
                "subject",
                fast.clone(),
            )
            .await
            .unwrap(),
            first
        );
        let error =
            super::request_configured(&dice, &root, root_epoch(&root), observed, "sub", fast)
                .await
                .unwrap_err();
        // The helper first reads its recovered root context while a genuine
        // child suspends that root. Preserve this earlier ownership boundary.
        assert!(
            error.contains("has no attribute `workspace_name`"),
            "{error}"
        );
        assert!(error.contains("finalize_link_action.bzl:183"), "{error}");
        fs::remove_dir_all(root).unwrap();
    }
}
