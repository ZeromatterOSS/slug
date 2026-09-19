//! Public requested Build publishes executable runfiles and durable source backing.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use slug_core_v2::runtime::AcceptedCommand;
use slug_core_v2::runtime::BuildCommandError;
use slug_core_v2::runtime::BzlmodCommandPolicyKey;
use slug_core_v2::runtime::BzlmodEnvironmentPolicyKey;
use slug_core_v2::runtime::LockfileMode;
use slug_core_v2::runtime::ProcessHostOwner;
use slug_core_v2::runtime::RequestedActionResult;
use slug_core_v2::runtime::TargetPattern;
use slug_core_v2::runtime::TerminalOutput;
use slug_core_v2::runtime::WorkspaceRuntime;

use super::*;

#[path = "runfiles_fixture.rs"]
mod authored;

struct Fixture {
    root: PathBuf,
    runtime: Option<WorkspaceRuntime>,
    module: String,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/wp750/wire-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let module = authored::write(&root);
        let runtime = Some(WorkspaceRuntime::new(&root, ProcessHostOwner::native()).unwrap());
        let fixture = Self {
            root,
            runtime,
            module,
        };
        fixture
    }
    fn content(&self, bytes: &str) {
        authored::content(&self.root, &self.module, bytes);
    }
    fn publish(
        &self,
        targets: &[&str],
        transport: &ActionChainReapiTransport,
    ) -> AcceptedCommand<Result<RequestedActionResult<ActionChainRemoteResult>, BuildCommandError>>
    {
        self.runtime
            .as_ref()
            .unwrap()
            .execute_and_publish_requested_actions_with_repository_environment(
                &targets
                    .iter()
                    .map(|target| TargetPattern::parse(target).unwrap())
                    .collect::<Vec<_>>(),
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[format!("file://{}/empty-registry", self.root.display())],
                Default::default(),
                Default::default(),
                transport,
            )
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Only fixture-owned real directories: never follow/chmod a link target.
        fn writable(path: &Path) {
            let Ok(metadata) = path.symlink_metadata() else {
                return;
            };
            if !metadata.is_dir() {
                return;
            }
            fs::set_permissions(
                path,
                fs::Permissions::from_mode(metadata.permissions().mode() | 0o700),
            )
            .unwrap();
            for entry in fs::read_dir(path).unwrap() {
                writable(&entry.unwrap().path());
            }
        }
        writable(&self.root);
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
#[ignore = "requires supervised fresh verifying NativeLink and Linux publication"]
fn nativelink_requested_binary_runfiles_backing_change_restore_survives_shutdown() {
    let mut fixture = Fixture::new();
    let endpoint = std::env::var("SLUG_V2_NATIVELINK_ENDPOINT").unwrap();
    let transport = ActionChainReapiTransport::new(
        RemoteConfig::from_args(&[&format!("--remote_executor={endpoint}")]).unwrap(),
    )
    .unwrap();
    let mut root = None;
    let mut original = None;
    for (iteration, (content, hits)) in [("aaa", 0), ("aaa", 2), ("bbb", 1), ("aaa", 2)]
        .into_iter()
        .enumerate()
    {
        if iteration >= 2 {
            fixture.content(content);
        }
        drop(
            fixture
                .publish(&["//:one"], &transport)
                .project(|accepted| {
                    let accepted = accepted.as_ref().unwrap();
                    let output = accepted.output().unwrap();
                    assert!(output.selected().is_none());
                    assert_eq!(output.results().len(), 6);
                    assert_eq!(
                        output
                            .results()
                            .iter()
                            .filter(|result| result.remote().is_some())
                            .count(),
                        2
                    );
                    assert_eq!(
                        output
                            .results()
                            .iter()
                            .filter(|result| matches!(result, ActionChainStepResult::Manifest(_)))
                            .count(),
                        2
                    );
                    assert_eq!(
                        output
                            .results()
                            .iter()
                            .filter(|result| matches!(
                                result,
                                ActionChainStepResult::SymlinkTree(_)
                            ))
                            .count(),
                        1
                    );
                    assert_eq!(
                        output
                            .results()
                            .iter()
                            .filter(|result| matches!(
                                result,
                                ActionChainStepResult::RunfilesTree { .. }
                            ))
                            .count(),
                        1
                    );
                    assert!(
                        output
                            .results()
                            .iter()
                            .filter_map(ActionChainStepResult::remote)
                            .all(|remote| remote.output_blobs.is_empty())
                    );
                    let json: serde_json::Value = serde_json::from_str(
                        &requested_build_success_json(accepted, "one-shot", None).unwrap(),
                    )
                    .unwrap();
                    assert_eq!(json["reapi_actions"], 2, "iteration {iteration}: {json}");
                    assert_eq!(json["direct_local_actions"], 0);
                    assert_eq!(json["ac_hits"], hits, "iteration {iteration}: {json}");
                    assert_eq!(json["ac_misses"], 2 - hits, "iteration {iteration}: {json}");
                    assert_eq!(json["action_digests"].as_array().unwrap().len(), 2);
                    root = Some(
                        accepted
                            .published_outputs()
                            .iter()
                            .find(|group| {
                                group
                                    .outputs()
                                    .outputs()
                                    .iter()
                                    .any(|output| output.path() == "binary")
                            })
                            .unwrap()
                            .outputs()
                            .root()
                            .to_owned(),
                    );
                    TerminalOutput::new(0, String::new(), String::new())
                }),
        );
        let paths = authored::check_tree(&fixture.root, root.as_ref().unwrap(), content.as_bytes());
        if let Some(original) = &original {
            assert_eq!(original == &paths, content == "aaa");
        } else {
            original = Some(paths);
        }
    }
    drop(fixture.runtime.take());
    authored::check_tree(&fixture.root, root.as_ref().unwrap(), b"aaa");
}

#[test]
#[ignore = "requires supervised fresh verifying NativeLink and Linux publication"]
fn nativelink_requested_manifest_root_publishes_complete_runfiles_backing() {
    let fixture = Fixture::new();
    let endpoint = std::env::var("SLUG_V2_NATIVELINK_ENDPOINT").unwrap();
    let transport = ActionChainReapiTransport::new(
        RemoteConfig::from_args(&[&format!("--remote_executor={endpoint}")]).unwrap(),
    )
    .unwrap();
    let mut root = None;
    drop(
        fixture
            .publish(&["//:binary.runfiles/MANIFEST"], &transport)
            .project(|accepted| {
                let accepted = accepted.as_ref().unwrap();
                let plan = accepted.inputs().plan().unwrap();
                let artifacts = plan.requested().unwrap().selection().artifacts();
                assert_eq!(artifacts.len(), 1);
                assert_eq!(artifacts[0].path(), "binary.runfiles/MANIFEST");
                let manifest = accepted
                    .published_outputs()
                    .iter()
                    .find(|group| {
                        group
                            .outputs()
                            .outputs()
                            .iter()
                            .any(|output| output.path() == "binary.runfiles/MANIFEST")
                    })
                    .unwrap();
                root = Some(manifest.outputs().root().to_owned());
                let json: serde_json::Value = serde_json::from_str(
                    &requested_build_success_json(accepted, "one-shot", None).unwrap(),
                )
                .unwrap();
                assert_eq!(json["reapi_actions"], 2);
                assert_eq!(json["direct_local_actions"], 0);
                TerminalOutput::new(0, String::new(), String::new())
            }),
    );
    authored::check_tree(&fixture.root, root.as_ref().unwrap(), b"aaa");
}
