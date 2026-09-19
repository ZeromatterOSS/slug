//! Requested artifact aliases retain backing provenance through execution/publication.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use slug_core_v2::runtime::AcceptedCommand;
use slug_core_v2::runtime::ActionChainOutputTransport;
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

#[path = "artifact_symlink_fixture.rs"]
mod authored;
#[path = "artifact_symlink_cas_tests.rs"]
mod cas_tests;

struct Fixture {
    root: PathBuf,
    runtime: Option<WorkspaceRuntime>,
    module: String,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/wp752/wire-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let module = authored::write(&root);
        let runtime = Some(WorkspaceRuntime::new(&root, ProcessHostOwner::native()).unwrap());
        Self {
            root,
            runtime,
            module,
        }
    }
    fn content(&self, bytes: &str) {
        authored::content(&self.root, &self.module, bytes);
    }
    fn run<T: ActionChainOutputTransport>(
        &self,
        targets: &[&str],
        transport: &T,
        publish: bool,
    ) -> Result<
        AcceptedCommand<Result<RequestedActionResult<T::Output>, BuildCommandError>>,
        BuildCommandError,
    > {
        let targets = targets
            .iter()
            .map(|value| TargetPattern::parse(value).unwrap())
            .collect::<Vec<_>>();
        let registry = [format!("file://{}/empty-registry", self.root.display())];
        let command = BzlmodCommandPolicyKey::from_flags(None, false).unwrap();
        let environment =
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap();
        let runtime = self.runtime.as_ref().unwrap();
        if publish {
            runtime.execute_and_publish_requested_actions_with_repository_environment(
                &targets,
                command,
                environment,
                LockfileMode::Update,
                &registry,
                Default::default(),
                Default::default(),
                transport,
            )
        } else {
            runtime.execute_requested_actions_with_repository_environment(
                &targets,
                command,
                environment,
                LockfileMode::Update,
                &registry,
                Default::default(),
                Default::default(),
                transport,
            )
        }
    }
    fn paths(&self, inputs: &PreparedActionChainInputs) -> Vec<(String, PathBuf)> {
        inputs
            .evaluation()
            .unwrap()
            .analyses()
            .filter(|node| !node.actions().is_empty())
            .flat_map(|node| {
                let root = slug_core_v2::runtime::configured_output_root(
                    &self.root,
                    node.configured_target_key()
                        .unwrap()
                        .configuration()
                        .slug_configuration()
                        .unwrap(),
                );
                node.actions().iter().flat_map(move |action| {
                    let root = root.clone();
                    action
                        .outputs()
                        .iter()
                        .map(move |output| (output.path().to_owned(), root.join(output.path())))
                })
            })
            .collect()
    }
    fn assert_absent(&self, inputs: &PreparedActionChainInputs, selected_consumers: bool) {
        for (name, path) in self.paths(inputs) {
            if selected_consumers && name.ends_with("_consumer.out") {
                continue;
            }
            assert!(
                fs::symlink_metadata(&path).is_err(),
                "unrequested output installed: {}",
                path.display()
            );
        }
        assert!(!self.root.join("bazel-out/.slug-runfiles-sources").exists());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.runtime.take();
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
fn transport(endpoint: &str) -> ActionChainReapiTransport {
    ActionChainReapiTransport::new(
        RemoteConfig::from_args(&[&format!("--remote_executor={endpoint}")]).unwrap(),
    )
    .unwrap()
}
fn remote_transport() -> ActionChainReapiTransport {
    transport(&std::env::var("SLUG_V2_NATIVELINK_ENDPOINT").unwrap())
}

#[test]
fn source_artifact_alias_execute_permission_changes_without_backend() {
    let fixture = Fixture::new();
    let transport = transport("grpc://127.0.0.1:1");
    let mut paths = Vec::new();
    let script = fixture.root.join("tools/source_script");
    let expected = ReapiDigest::of_bytes(&fs::read(&script).unwrap());
    for mode in [0o755, 0o644, 0o655, 0o744] {
        fs::set_permissions(&script, fs::Permissions::from_mode(mode)).unwrap();
        for target in ["//:direct_exec", "//:nested_exec"] {
            let result = fixture.run(&[target], &transport, false);
            if mode & 0o100 == 0 {
                let error = result.unwrap_err().to_string();
                assert!(
                    error.contains("executable"),
                    "{target} mode {mode:o}: {error}"
                );
                continue;
            }
            drop(result.unwrap().project(|accepted| {
                let accepted = accepted.as_ref().unwrap();
                assert!(accepted.published_outputs().is_empty());
                let output = accepted.output().unwrap();
                assert_eq!(
                    output.results().len(),
                    if target == "//:direct_exec" { 1 } else { 2 }
                );
                for step in output.results() {
                    let ActionChainStepResult::ArtifactSymlink(alias) = step else {
                        panic!("source alias fabricated remote work: {step:?}");
                    };
                    assert_eq!(alias.digest(), &expected);
                    assert!(alias.is_executable());
                }
                paths.extend(
                    fixture
                        .paths(accepted.inputs())
                        .into_iter()
                        .map(|(_, path)| path),
                );
                fixture.assert_absent(accepted.inputs(), false);
                TerminalOutput::new(0, String::new(), String::new())
            }));
        }
    }
    for path in paths {
        assert!(fs::symlink_metadata(path).is_err());
    }
}

#[test]
#[ignore = "requires supervised fresh verifying NativeLink"]
fn nativelink_artifact_alias_consumers_restore_without_tool_publication() {
    let fixture = Fixture::new();
    let transport = remote_transport();
    let mut original = None;
    for (iteration, bytes) in ["aaa", "aaa", "bbb", "aaa"].into_iter().enumerate() {
        if iteration >= 2 {
            fixture.content(bytes);
        }
        drop(
            fixture
                .run(
                    &["//:source_consumer", "//:generated_consumer"],
                    &transport,
                    true,
                )
                .unwrap()
                .project(|accepted| {
                    let accepted = accepted.as_ref().unwrap();
                    let output = accepted.output().unwrap();
                    let remotes = output
                        .results()
                        .iter()
                        .filter_map(ActionChainStepResult::remote)
                        .collect::<Vec<_>>();
                    assert_eq!(
                        remotes.len(),
                        4,
                        "one shared producer, one nonexecutable script write, two consumers"
                    );
                    assert_eq!(
                        remotes
                            .iter()
                            .map(|remote| remote.evidence.ac_hits)
                            .sum::<u64>(),
                        [0, 4, 1, 4][iteration]
                    );
                    let digests = remotes
                        .iter()
                        .map(|remote| remote.action_digest.clone())
                        .collect::<Vec<_>>();
                    if let Some(original) = &original {
                        assert_eq!(original == &digests, bytes == "aaa");
                    } else {
                        original = Some(digests);
                    }
                    let script = remotes
                        .iter()
                        .flat_map(|remote| remote.result.output_files())
                        .find(|file| file.path() == "tools/generated_script")
                        .unwrap();
                    assert!(
                        !script.is_executable(),
                        "remote-only generated modefalse target is deliberate"
                    );
                    let alias = output
                        .results()
                        .iter()
                        .find_map(|step| match step {
                            ActionChainStepResult::ArtifactSymlink(alias)
                                if alias.output().path() == "tools/generated_binary" =>
                            {
                                Some(alias)
                            }
                            _ => None,
                        })
                        .unwrap();
                    assert_eq!(alias.digest(), script.digest());
                    assert!(alias.is_executable());
                    for group in accepted.published_outputs() {
                        for out in group.outputs().outputs() {
                            assert!(out.path().ends_with("_consumer.out"));
                            assert_eq!(
                                fs::read(group.outputs().root().join(out.path())).unwrap(),
                                bytes.as_bytes().repeat(3)
                            );
                        }
                    }
                    assert_eq!(
                        accepted
                            .published_outputs()
                            .iter()
                            .map(|group| group.outputs().outputs().len())
                            .sum::<usize>(),
                        2
                    );
                    fixture.assert_absent(accepted.inputs(), true);
                    TerminalOutput::new(0, String::new(), String::new())
                }),
        );
    }
}

#[test]
#[ignore = "requires supervised fresh verifying NativeLink and Linux publication"]
fn nativelink_artifact_alias_publication_replacement_and_manifest_backing_survive_shutdown() {
    let mut fixture = Fixture::new();
    let transport = remote_transport();
    let mut bin = None;
    // Only the alias to the public MANIFEST is requested. Its virtual tree and
    // transitive alias backing must nevertheless complete and publish together.
    drop(
        fixture
            .run(&["//:manifest_alias"], &transport, true)
            .unwrap()
            .project(|accepted| {
                let accepted = accepted.as_ref().unwrap();
                let requested = accepted.inputs().plan().unwrap();
                let artifacts = requested.requested().unwrap().selection().artifacts();
                assert_eq!(artifacts.len(), 1);
                assert_eq!(artifacts[0].path(), "manifest.alias");
                assert!(
                    accepted
                        .output()
                        .unwrap()
                        .results()
                        .iter()
                        .any(|step| matches!(step, ActionChainStepResult::RunfilesTree { .. }))
                );
                bin = accepted
                    .published_outputs()
                    .iter()
                    .find(|group| {
                        group
                            .outputs()
                            .outputs()
                            .iter()
                            .any(|out| out.path() == "manifest.alias")
                    })
                    .map(|group| group.outputs().root().to_owned());
                TerminalOutput::new(0, String::new(), String::new())
            }),
    );
    let bin = bin.unwrap();
    authored::check_manifest_alias(&fixture.root, &bin, b"aaa");
    let source_mode = fs::metadata(fixture.root.join("input"))
        .unwrap()
        .permissions()
        .mode();
    for regular in [true, false] {
        authored::replace_host_alias(&fixture.root, regular);
        drop(
            fixture
                .run(&["//:aliases"], &transport, true)
                .unwrap()
                .project(|accepted| {
                    accepted.as_ref().unwrap();
                    TerminalOutput::new(0, String::new(), String::new())
                }),
        );
        assert_eq!(
            fs::symlink_metadata(bin.join("host.alias"))
                .unwrap()
                .file_type()
                .is_symlink(),
            !regular
        );
        assert_eq!(
            fs::read(bin.join("host.alias")).unwrap(),
            if regular {
                b"replacement".as_slice()
            } else {
                b"aaa"
            }
        );
        assert_eq!(fs::read(fixture.root.join("input")).unwrap(), b"aaa");
        assert_eq!(
            fs::metadata(fixture.root.join("input"))
                .unwrap()
                .permissions()
                .mode(),
            source_mode
        );
    }
    drop(fixture.runtime.take());
    authored::check_manifest_alias(&fixture.root, &bin, b"aaa");
}
