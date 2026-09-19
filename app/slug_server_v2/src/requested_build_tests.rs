//! Production daemon request dispatch, without a backend or action execution.
use super::*;
use crate::RemoteRequest;

#[path = "../../slug_core_v2/src/runtime/source_staging/test_workspace.rs"]
mod workspace_fixture;

pub(super) fn write_hermetic_module(workspace: &Path, source: &str) {
    // Reuse the authored local built-in module declarations, without replacing
    // the protected fixture's BUILD files, diagnostic lines, or execution policy.
    let dependencies = workspace.join(".slug_test_builtin/offline");
    fs::create_dir_all(&dependencies).unwrap();
    workspace_fixture::write(&dependencies);
    let declarations = fs::read_to_string(dependencies.join("MODULE.bazel"))
        .unwrap()
        .lines()
        .filter(|line| {
            !line.starts_with("module(") && !line.starts_with("register_execution_platforms(")
        })
        .map(|line| line.replace("path='", "path='.slug_test_builtin/offline/"))
        .collect::<Vec<_>>()
        .join("\n");
    write(
        &workspace.join("MODULE.bazel"),
        &format!("{source}{declarations}\n"),
    );
}

struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let root = scratch("requested-build");
        workspace_fixture::write(&root);
        write(
            &root.join("defs.bzl"),
            "def _impl(ctx): return [DefaultInfo(files=depset([]))]\nempty = rule(implementation=_impl)\n",
        );
        write(
            &root.join("BUILD.bazel"),
            "load(':defs.bzl', 'empty')\nplatform(name='platform')\nexports_files(['input'])\nempty(name='empty')\nalias(name='source_alias', actual=':input')\n",
        );
        Self(root)
    }

    fn request(&self, targets: &[&str], remote: &RemoteConfig) -> BuildRequest {
        BuildRequest {
            targets: targets.iter().map(|value| (*value).to_owned()).collect(),
            configuration_overlay: Default::default(),
            remote: remote.into(),
            bzlmod: BzlmodRequestInputs {
                registry_urls: vec![format!("file://{}/empty-registry", self.0.display())],
                ..Default::default()
            },
            repository_environment: Default::default(),
        }
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn remote_execute() -> RemoteConfig {
    RemoteConfig::from_args(&["--remote_executor=grpc://127.0.0.1:1"]).unwrap()
}

fn dispatch(daemon: &mut Daemon, request: BuildRequest) -> DaemonResponse {
    handle_request(
        daemon,
        &serde_json::to_string(&DaemonRequest::Build(request)).unwrap(),
    )
}

fn terminal(response: &DaemonResponse) -> serde_json::Value {
    serde_json::from_str(response.stderr.lines().last().unwrap()).unwrap()
}

#[test]
fn requested_sources_and_empty_rules_succeed_without_backend_and_preserve_other_modes() {
    let workspace = Workspace::new();
    let mut daemon = Daemon::new(&workspace.0).unwrap();
    for labels in [
        vec!["//:input"],
        vec!["//:source_alias"],
        vec!["//:empty"],
        vec!["//:input", "//:empty"],
    ] {
        let response = dispatch(&mut daemon, workspace.request(&labels, &remote_execute()));
        assert_eq!(response.exit_code, 0, "{response:?}");
        let evidence = terminal(&response);
        assert_eq!(evidence["completed_boundary"], "reapi_native_execution");
        for field in [
            "reapi_actions",
            "direct_local_actions",
            "ac_hits",
            "ac_misses",
        ] {
            assert_eq!(evidence[field], 0);
        }
        for field in ["action_digests", "uploaded_digests", "materialized_outputs"] {
            assert_eq!(evidence[field], serde_json::json!([]));
        }
        assert!(response.run_launch_plan.is_none());
    }
    let mut cache = remote_disabled();
    cache.cache = Some("grpc://127.0.0.1:1".into());
    for remote in [remote_disabled(), cache] {
        let empty = dispatch(&mut daemon, workspace.request(&["//:empty"], &remote));
        assert_eq!(empty.exit_code, 2);
        assert_eq!(terminal(&empty)["error"], "analysis_not_implemented");
        let source = dispatch(&mut daemon, workspace.request(&["//:input"], &remote));
        assert_eq!(source.exit_code, 0, "{source:?}");
        assert_eq!(
            terminal(&source)["completed_boundary"],
            "dice_exported_source_file"
        );
    }
    let run = DaemonRequest::Run(workspace.request(&["//:empty"], &remote_execute()));
    let response = handle_request(&mut daemon, &serde_json::to_string(&run).unwrap());
    assert_ne!(
        response.exit_code, 0,
        "Run must retain executable admission"
    );
    assert!(response.run_launch_plan.is_none());
}

fn error_recovery(analysis: bool) {
    let workspace = Workspace::new();
    let mut daemon = Daemon::new(&workspace.0).unwrap();
    let mut baseline = Daemon::new(&workspace.0).unwrap();
    let good = "print('SERVER_LOADING')\ndef _impl(ctx):\n    print('SERVER_ANALYSIS')\n    out = ctx.actions.declare_file('shared')\n    ctx.actions.write(out, ctx.attr.content)\n    return [DefaultInfo(files=depset([]))]\nsubject = rule(implementation=_impl, attrs={'content': attr.string()})\n";
    let build = "load(':defs.bzl', 'subject')\nplatform(name='platform')\nsubject(name='one', content='same')\nsubject(name='two', content='same')\n";
    for (iteration, broken) in [true, false, true].into_iter().enumerate() {
        write(
            &workspace.0.join("defs.bzl"),
            &if analysis && broken {
                good.replace("    out =", "    fail('SERVER_FAILURE')\n    out =")
            } else {
                good.to_owned()
            },
        );
        write(
            &workspace.0.join("BUILD.bazel"),
            &if !analysis && broken {
                build.replace(
                    "name='two', content='same'",
                    "name='two', content='different'",
                )
            } else {
                build.to_owned()
            },
        );
        let request = workspace.request(&["//:one", "//:two"], &remote_execute());
        let response = dispatch(&mut daemon, request.clone());
        let mut ordinary = request;
        ordinary.remote = RemoteRequest::default();
        let expected = dispatch(&mut baseline, ordinary);
        if broken {
            assert_eq!(
                response.exit_code, expected.exit_code,
                "iteration {iteration}: {response:?}"
            );
            let actual = terminal(&response);
            let expected = terminal(&expected);
            assert_eq!(
                actual["error"],
                if analysis {
                    "build_runtime_error"
                } else {
                    "configured_action_conflict"
                }
            );
            assert_eq!(actual["error"], expected["error"]);
            assert_eq!(actual["message"], expected["message"]);
        } else {
            assert_eq!(response.exit_code, 0, "iteration {iteration}: {response:?}");
            assert_eq!(terminal(&response)["reapi_actions"], 0);
        }
        assert_eq!(
            response
                .stderr
                .lines()
                .filter(|line| line.starts_with("DEBUG:"))
                .collect::<Vec<_>>(),
            expected
                .stderr
                .lines()
                .filter(|line| line.starts_with("DEBUG:"))
                .collect::<Vec<_>>(),
            "iteration {iteration}"
        );
        assert!(response.run_launch_plan.is_none());
        // Both rules declare outputs, but DefaultInfo selects none, including
        // during the successful repair; no backend or visible output is needed.
        let entries = match fs::read_dir(workspace.0.join("bazel-out")) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => panic!("reading output directory: {error}"),
        };
        for entry in entries {
            assert!(!entry.unwrap().path().join("bin/shared").exists());
        }
    }
}

#[test]
fn requested_analysis_errors_keep_typed_diagnostics_through_recovery() {
    error_recovery(true);
}

#[test]
fn requested_conflicts_keep_typed_diagnostics_through_recovery() {
    error_recovery(false);
}

#[test]
fn remote_policy_round_trips_and_rejects_unsupported_options_without_secrets() {
    let workspace = Workspace::new();
    let mut daemon = Daemon::new(&workspace.0).unwrap();
    let secret = "REMOTE_HEADER_SENTINEL";
    let mut config = remote_execute();
    config.instance_name = Some("instance".into());
    config.headers.insert("authorization".into(), secret.into());
    config.timeout_seconds = Some(19);
    config.retry_attempts = Some(3);
    config
        .default_exec_properties
        .insert("cpu".into(), "test".into());
    let wire: RemoteRequest = (&config).into();
    let decoded: RemoteRequest =
        serde_json::from_str(&serde_json::to_string(&wire).unwrap()).unwrap();
    assert_eq!(decoded.to_config(), config);
    assert!(!format!("{wire:?}").contains(secret));
    let mut cases = Vec::new();
    for index in 0..4 {
        let mut config = remote_execute();
        match index {
            0 => {
                config.headers.insert("authorization".into(), secret.into());
            }
            1 => config.timeout_seconds = Some(1),
            2 => config.retry_attempts = Some(1),
            _ => config.cache = Some("grpc://127.0.0.1:2".into()),
        }
        cases.push(config);
    }
    for config in cases {
        let response = dispatch(&mut daemon, workspace.request(&["//:empty"], &config));
        assert_eq!(response.exit_code, 2);
        assert_eq!(terminal(&response)["error"], "build_runtime_error");
        assert!(!response.stderr.contains(secret));
        assert!(!response.stderr.contains("DEBUG:"));
    }
    let mut raw = serde_json::to_value(DaemonRequest::Build(
        workspace.request(&["//:empty"], &remote_execute()),
    ))
    .unwrap();
    raw["request"]["remote"]["headers"] = serde_json::json!(secret);
    let response = handle_request(&mut daemon, &raw.to_string());
    assert_eq!(terminal(&response)["error"], "daemon_parse_error");
    assert!(!response.stderr.contains(secret));
    let result = daemon.build(
        &[target("//:empty")],
        &remote_disabled(),
        &[format!("--remote_header=authorization={secret}")],
    );
    assert!(!result.stderr.contains(secret));
    assert!(result.stderr.contains("--remote_header=<redacted>"));
}

#[test]
fn malformed_or_empty_targets_reject_the_entire_request_before_runtime() {
    let workspace = Workspace::new();
    let mut daemon = Daemon::new(&workspace.0).unwrap();
    for labels in [vec![], vec!["//:empty", "//bad//package:target"]] {
        let response = dispatch(&mut daemon, workspace.request(&labels, &remote_execute()));
        assert_eq!(response.exit_code, 2);
        assert_eq!(terminal(&response)["error"], "build_request_error");
        assert_eq!(response.invalidated_files, 0);
        assert!(daemon.forwarded_bzlmod_inputs.is_empty());
        assert!(daemon.observations.previous.is_none());
    }
    let old =
        r#"{"kind":"build","request":{"targets":["//:empty"],"executor":"grpc://127.0.0.1:1"}}"#;
    assert_eq!(
        terminal(&handle_request(&mut daemon, old))["error"],
        "daemon_parse_error"
    );
}
