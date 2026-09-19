use super::*;

#[path = "../../../slug_reapi_v2/src/action_chain/runfiles_fixture.rs"]
mod authored;

#[test]
#[ignore = "requires supervised fresh verifying NativeLink and Linux publication"]
fn one_shot_binary_runfiles_survive_cli_process_exit() {
    let workspace = Workspace::new();
    authored::write(&workspace.root);
    let endpoint = format!(
        "--remote_executor={}",
        std::env::var("SLUG_V2_NATIVELINK_ENDPOINT").unwrap()
    );
    let output = workspace.run(false, &["build", "//:one", &endpoint]);
    let (json, _) = terminal(&output, 0);
    assert_eq!(json["success"], true);
    assert_eq!(json["runtime_mode"], "one-shot");
    assert_eq!(json["reapi_actions"], 2, "{json}");
    assert_eq!(json["direct_local_actions"], 0);
    assert_eq!(json["ac_hits"], 0, "{json}");
    assert_eq!(json["ac_misses"], 2, "{json}");
    assert_eq!(json["action_digests"].as_array().unwrap().len(), 2);
    assert_eq!(json["completed_boundary"], "reapi_native_execution");
    assert!(workspace.daemon.is_none());
    // run() has reaped the actual CLI process. The binary must resolve its tree
    // and both source modes without a live WorkspaceRuntime or prepared handle.
    authored::check_tree(&workspace.root, &workspace.bin(), b"aaa");
}
