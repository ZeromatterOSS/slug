//! Production Build adapters; backend selectors run under the existing supervisor.
#![cfg(all(target_os = "linux", target_env = "gnu"))]
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use serde_json::Value;
use slug_reapi_v2::ReapiDigest;

#[path = "../../slug_core_v2/src/runtime/source_staging/test_workspace.rs"]
mod fixture;

#[path = "requested_build/runfiles.rs"]
mod runfiles;

const DEFS: &str = r#"print('WP746_DEFS')
def _impl(ctx):
    print('WP746_ANALYSIS')
    seed = ctx.actions.declare_file('seed')
    file = ctx.outputs.file
    tree = ctx.actions.declare_directory('tree')
    empty = ctx.actions.declare_directory('emptytree')
    done = ctx.actions.declare_file('done')
    result = ctx.actions.declare_directory('result_tree')
    unused = ctx.actions.declare_file('unused')
    ctx.actions.write(unused, 'must not execute')
    ctx.actions.write(seed, 'seed')
    tool = ctx.attr.tool[DefaultInfo].files.to_list()[0]
    source = ctx.attr.input[DefaultInfo].files.to_list()[0]
    ctx.actions.run(executable=tool, inputs=[seed, source], outputs=[file, tree, empty], arguments=['produce'])
    args = ctx.actions.args()
    args.add('consume')
    args.use_param_file('@%s', use_always=True)
    args.set_param_file_format('multiline')
    ctx.actions.run(executable=tool, inputs=[file, tree, empty, source], outputs=[done, result], arguments=[args])
    return [DefaultInfo(files=depset([file, result]))]
stage = rule(implementation=_impl, attrs={'file':attr.output(mandatory=True), 'input':attr.label(allow_single_file=True), 'tool':attr.label(allow_single_file=True)})
def _empty(ctx): return []
empty = rule(implementation=_empty)
def _check(ctx):
    CHECK_BODY
check = rule(implementation=_check)
def _writer(ctx):
    out = ctx.actions.declare_file('conflict.out')
    ctx.actions.write(out, ctx.attr.content)
    return []
writer = rule(implementation=_writer, attrs={'content':attr.string()})
"#;
const TOOL: &str = r#"#!/bin/sh
set -eu
if [ "$1" = produce ]; then
  [ "$(/bin/cat seed)" = seed ]
  [ -d tree ] && [ -d emptytree ]
  /bin/mkdir -p tree/nested tree/empty
  /bin/cat input > tree/nested/value
  /bin/cat input > file
  /bin/chmod 0644 file tree/nested/value
else
  read step < "${1#@}"
  [ "$step" = consume ]
  [ -x file ] && [ -x tree/nested/value ]
  [ -d emptytree ] && [ ! -e tree/empty ]
  /bin/cat file tree/nested/value input > done
  /bin/mkdir -p result_tree/nested result_tree/empty
  /bin/cat input > result_tree/nested/value
  if [ "$(/bin/cat input)" = aaa ]; then /bin/cat input > result_tree/stale; fi
  /bin/chmod 0644 done result_tree/nested/value
fi
"#;
const BUILD: &str = "load(':defs.bzl', 'stage', 'empty', 'check', 'writer')\nprint('WP746_BUILD')\nplatform(name='platform')\nexports_files(['tools/tool', 'input'])\nstage(name='one', file='file', input='input', tool='tools/tool')\nalias(name='one_alias', actual='one')\nempty(name='empty')\ncheck(name='check')\nwriter(name='left', content='left')\nwriter(name='right', content='RIGHT_CONTENT')\n";

struct Workspace {
    root: PathBuf,
    output_base: PathBuf,
    daemon: Option<Child>,
}
impl Workspace {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/wp746/cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        fixture::write(&root);
        let module = fs::read_to_string(root.join("MODULE.bazel")).unwrap();
        fs::write(
            root.join("MODULE.bazel"),
            format!("print('WP746_MODULE')\n{module}"),
        )
        .unwrap();
        fs::write(
            root.join("defs.bzl"),
            DEFS.replace("CHECK_BODY", "return []"),
        )
        .unwrap();
        fs::write(
            root.join("BUILD.bazel"),
            BUILD.replace("RIGHT_CONTENT", "left"),
        )
        .unwrap();
        fs::create_dir(root.join("tools")).unwrap();
        fs::write(root.join("tools/tool"), TOOL).unwrap();
        Self {
            output_base: root.join("daemon"),
            root,
            daemon: None,
        }
    }
    fn start_daemon(&mut self) {
        fs::create_dir(&self.output_base).unwrap();
        let socket = slug_server_v2::socket_path(&self.output_base);
        let mut child = Command::new(env!("CARGO_BIN_EXE_slug"))
            .args(["--serve", "--socket"])
            .arg(&socket)
            .arg("--workspace")
            .arg(&self.root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        fs::write(
            slug_server_v2::pid_path(&self.output_base),
            child.id().to_string(),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while std::os::unix::net::UnixStream::connect(&socket).is_err() {
            assert!(
                child.try_wait().unwrap().is_none(),
                "daemon exited during startup"
            );
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("daemon readiness timeout");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        self.daemon = Some(child);
    }
    fn run(&self, daemon: bool, args: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_slug"));
        command.current_dir(&self.root);
        if daemon {
            command.arg(format!("--output_base={}", self.output_base.display()));
        }
        let mut child = command
            .args(args)
            .arg(format!(
                "--registry=file://{}/empty-registry",
                self.root.display()
            ))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("CLI command timed out: {args:?}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        child.wait_with_output().unwrap()
    }
    fn bin(&self) -> PathBuf {
        let roots = fs::read_dir(self.root.join("bazel-out"))
            .unwrap()
            .map(|entry| entry.unwrap().path().join("bin"))
            .filter(|path| path.is_dir())
            .collect::<Vec<_>>();
        assert_eq!(roots.len(), 1);
        roots[0].clone()
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        if let Some(mut child) = self.daemon.take() {
            let _ = slug_server_v2::send_shutdown(&slug_server_v2::socket_path(&self.output_base));
            let deadline = Instant::now() + Duration::from_secs(2);
            while child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
        fn writable(path: &Path) {
            let Ok(metadata) = path.symlink_metadata() else {
                return;
            };
            if !metadata.is_dir() {
                return;
            }
            let _ = fs::set_permissions(
                path,
                fs::Permissions::from_mode(metadata.permissions().mode() | 0o700),
            );
            if let Ok(children) = path.read_dir() {
                for child in children.flatten() {
                    writable(&child.path());
                }
            }
        }
        writable(&self.root);
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn terminal(output: &Output, exit: i32) -> (Value, String) {
    let stderr = String::from_utf8(output.stderr.clone()).unwrap();
    assert_eq!(output.status.code(), Some(exit), "{stderr}");
    assert!(output.stdout.is_empty(), "unexpected stdout: {output:?}");
    let json = serde_json::from_str(stderr.lines().last().unwrap()).unwrap();
    (json, stderr)
}
fn zero(json: &Value) {
    assert_eq!(json["success"], true);
    assert_eq!(json["completed_boundary"], "reapi_native_execution");
    for count in [
        "reapi_actions",
        "direct_local_actions",
        "ac_hits",
        "ac_misses",
    ] {
        assert_eq!(json[count], 0);
    }
    for list in ["action_digests", "uploaded_digests", "materialized_outputs"] {
        assert_eq!(json[list], serde_json::json!([]));
    }
}

#[test]
fn source_and_analyzed_empty_builds_skip_unreachable_executor() {
    for daemon in [false, true] {
        let mut workspace = Workspace::new();
        if daemon {
            workspace.start_daemon();
        }
        let (json, _) = terminal(
            &workspace.run(
                daemon,
                &[
                    "build",
                    "//:input",
                    "//:empty",
                    "--remote_executor=grpc://127.0.0.1:1",
                ],
            ),
            0,
        );
        zero(&json);
        assert!(!workspace.root.join("bazel-out").exists());
    }
}

#[test]
fn build_policy_and_parser_errors_redact_headers_before_daemon_start() {
    let secret = "WP746_HEADER_SECRET";
    for daemon in [false, true] {
        let workspace = Workspace::new();
        for policy in [
            format!("--remote_header=authorization={secret}"),
            "--remote_timeout=1".into(),
            "--remote_retries=1".into(),
            "--remote_cache=grpc://127.0.0.1:2".into(),
        ] {
            let (json, stderr) = terminal(
                &workspace.run(
                    daemon,
                    &[
                        "build",
                        "//:empty",
                        "--remote_executor=grpc://127.0.0.1:1",
                        &policy,
                    ],
                ),
                2,
            );
            assert_eq!(json["error"], "build_runtime_error");
            assert!(!stderr.contains(secret));
            assert!(!slug_server_v2::pid_path(&workspace.output_base).exists());
        }
        for bare_flag in [
            "--remote_header",
            "--remote_timeout",
            "--remote_retries",
            "--remote_cache",
            "--remote_executor",
            "--remote_instance_name",
            "--remote_default_exec_properties",
        ] {
            let (json, stderr) =
                terminal(&workspace.run(daemon, &["build", "//:empty", bare_flag]), 2);
            assert_eq!(json["error"], "build_runtime_error", "{bare_flag}");
            assert!(!stderr.contains(secret));
            assert!(!slug_server_v2::pid_path(&workspace.output_base).exists());
        }
        let (_, stderr) = terminal(
            &workspace.run(
                daemon,
                &[
                    "build",
                    "//:empty",
                    "--unknown_flag",
                    &format!("--remote_header=authorization={secret}"),
                ],
            ),
            2,
        );
        assert!(!stderr.contains(secret));
        assert!(!slug_server_v2::pid_path(&workspace.output_base).exists());
        let (json, stderr) = terminal(
            &workspace.run(
                daemon,
                &[
                    "build",
                    "//:empty",
                    "--remote_header",
                    &format!("authorization=Bearer {secret}"),
                ],
            ),
            2,
        );
        assert_eq!(json["error"], "command_parse_error");
        assert!(!stderr.contains(secret));
        assert!(!slug_server_v2::pid_path(&workspace.output_base).exists());
    }
}

#[test]
fn analysis_only_build_redacts_headers_and_preserves_missing_target_and_run_controls() {
    let workspace = Workspace::new();
    for cache in [None, Some("--remote_cache=grpc://127.0.0.1:1")] {
        let mut args = vec![
            "build",
            "//:empty",
            "--remote_header=authorization=WP746_HEADER_SECRET",
        ];
        args.extend(cache);
        let (json, stderr) = terminal(&workspace.run(false, &args), 2);
        assert_eq!(json["error"], "analysis_not_implemented");
        assert!(!stderr.contains("WP746_HEADER_SECRET"));
        assert!(
            json["argv"]
                .as_array()
                .unwrap()
                .contains(&Value::from("--remote_header=<redacted>"))
        );
    }
    let (json, _) = terminal(&workspace.run(false, &["build"]), 2);
    assert_eq!(json["error"], "command_parse_error");
    assert_eq!(json["message"], "build requires a target pattern");
    let (json, _) = terminal(&workspace.run(false, &["run", "//:empty"]), 2);
    assert_eq!(json["message"], "run requires --remote_executor");
}

#[test]
fn typed_build_errors_repair_and_recur_without_remote_effects() {
    for daemon in [false, true] {
        let mut workspace = Workspace::new();
        if daemon {
            workspace.start_daemon();
        }
        for broken in [true, false, true] {
            fs::write(
                workspace.root.join("defs.bzl"),
                DEFS.replace(
                    "CHECK_BODY",
                    if broken {
                        "fail('WP746_ANALYSIS_FAILURE')"
                    } else {
                        "return []"
                    },
                ),
            )
            .unwrap();
            let (json, _) = terminal(
                &workspace.run(
                    daemon,
                    &["build", "//:check", "--remote_executor=grpc://127.0.0.1:1"],
                ),
                if broken { 2 } else { 0 },
            );
            if broken {
                assert_eq!(json["error"], "build_runtime_error");
                assert!(
                    json["message"]
                        .as_str()
                        .unwrap()
                        .contains("WP746_ANALYSIS_FAILURE")
                );
            } else {
                zero(&json);
            }
        }
        for broken in [true, false, true] {
            fs::write(
                workspace.root.join("BUILD.bazel"),
                BUILD.replace("RIGHT_CONTENT", if broken { "different" } else { "left" }),
            )
            .unwrap();
            let (json, _) = terminal(
                &workspace.run(
                    daemon,
                    &[
                        "build",
                        "//:left",
                        "//:right",
                        "--remote_executor=grpc://127.0.0.1:1",
                    ],
                ),
                if broken { 2 } else { 0 },
            );
            if broken {
                assert_eq!(json["error"], "configured_action_conflict");
                assert!(
                    json["message"]
                        .as_str()
                        .unwrap()
                        .starts_with("configured action output conflict at conflict.out:")
                );
            } else {
                zero(&json);
            }
        }
        assert!(!workspace.root.join("bazel-out").exists());
    }
}

fn wire(daemon: bool) {
    let mut workspace = Workspace::new();
    // Keep the observed package directory stable when publication creates its
    // private children; the backend and each adapter still start cold.
    fs::create_dir(workspace.root.join("bazel-out")).unwrap();
    if daemon {
        workspace.start_daemon();
    }
    let pid = workspace.daemon.as_ref().map(Child::id);
    let endpoint = format!(
        "--remote_executor={}",
        std::env::var("SLUG_V2_NATIVELINK_ENDPOINT").unwrap()
    );
    let mut digests = Vec::new();
    for (index, bytes) in [b"aaa", b"aaa", b"bbb", b"aaa"].into_iter().enumerate() {
        fs::write(workspace.root.join("input"), bytes).unwrap();
        let (json, stderr) = terminal(
            &workspace.run(
                daemon,
                &[
                    "build",
                    "//:one_alias",
                    "//:file",
                    "//:input",
                    "//:one",
                    &endpoint,
                    "--remote_default_exec_properties=container-image=selected:v1",
                ],
            ),
            0,
        );
        assert_eq!(
            json["runtime_mode"],
            if daemon { "daemon" } else { "one-shot" }
        );
        assert_eq!(json["success"], true);
        assert_eq!(json["declared_action_count"], 4);
        assert_eq!(json["reapi_actions"], 3);
        assert_eq!(json["direct_local_actions"], 0);
        let (hits, misses) = [(0, 3), (3, 0), (1, 2), (3, 0)][index];
        assert_eq!(json["ac_hits"], hits, "iteration {index}: {stderr}");
        assert_eq!(json["ac_misses"], misses, "iteration {index}: {stderr}");
        assert_eq!(
            json["platform_properties"],
            serde_json::json!({"container-image":"selected:v1"})
        );
        assert_eq!(
            json["uploaded_digests"].as_array().unwrap().is_empty(),
            index == 1 || index == 3
        );
        digests.push(json["action_digests"].clone());
        let content = ReapiDigest::of_bytes(bytes).to_string();
        assert_eq!(
            json["materialized_outputs"],
            serde_json::json!(vec![content; if bytes == b"aaa" { 3 } else { 2 }])
        );
        let events = stderr
            .lines()
            .filter(|line| line.starts_with("DEBUG:"))
            .map(|line| line.rsplit(": ").next().unwrap())
            .collect::<Vec<_>>();
        if daemon && index > 0 {
            assert!(events.is_empty(), "{stderr}");
        } else {
            assert_eq!(
                events,
                [
                    "WP746_MODULE",
                    "WP746_DEFS",
                    "WP746_BUILD",
                    "WP746_ANALYSIS"
                ]
            );
        }
        let bin = workspace.bin();
        assert_eq!(fs::read(bin.join("file")).unwrap(), bytes);
        assert_eq!(
            fs::read(bin.join("result_tree/nested/value")).unwrap(),
            bytes
        );
        assert_eq!(bin.join("result_tree/stale").exists(), bytes == b"aaa");
        assert!(bin.join("result_tree/empty").is_dir());
        for path in [
            "file",
            "result_tree",
            "result_tree/nested",
            "result_tree/nested/value",
            "result_tree/empty",
        ] {
            assert_eq!(
                fs::metadata(bin.join(path)).unwrap().permissions().mode() & 0o777,
                0o555
            );
        }
        for path in ["seed", "tree", "emptytree", "done", "unused", "input"] {
            assert!(!bin.join(path).exists(), "unselected output {path}");
        }
        if index == 0 {
            fs::write(bin.join("unrelated"), b"keep").unwrap();
        } else {
            assert_eq!(fs::read(bin.join("unrelated")).unwrap(), b"keep");
        }
        if let Some(pid) = pid {
            assert_eq!(
                fs::read_to_string(slug_server_v2::pid_path(&workspace.output_base)).unwrap(),
                pid.to_string()
            );
            assert!(
                workspace
                    .daemon
                    .as_mut()
                    .unwrap()
                    .try_wait()
                    .unwrap()
                    .is_none()
            );
        }
    }
    assert_eq!(digests[0], digests[1]);
    assert_eq!(digests[0], digests[3]);
    assert_eq!(digests[0][0], digests[2][0]);
    assert_ne!(digests[0][1], digests[2][1]);
    assert_ne!(digests[0][2], digests[2][2]);
}
#[test]
#[ignore = "requires supervised fresh NativeLink"]
fn one_shot_requested_file_tree_cold_warm_change_restore() {
    wire(false);
}
#[test]
#[ignore = "requires supervised fresh NativeLink and retained daemon"]
fn daemon_requested_file_tree_cold_warm_change_restore() {
    wire(true);
}
