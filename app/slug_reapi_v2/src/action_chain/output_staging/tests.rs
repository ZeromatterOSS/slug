use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use slug_core_v2::runtime::AcceptedCommand;
use slug_core_v2::runtime::ActionChainResult;
use slug_core_v2::runtime::BuildCommandError;
use slug_core_v2::runtime::BzlmodCommandPolicyKey;
use slug_core_v2::runtime::BzlmodEnvironmentPolicyKey;
use slug_core_v2::runtime::LockfileMode;
use slug_core_v2::runtime::TargetPattern;
use slug_core_v2::runtime::TerminalOutput;

use super::super::tests::TOOL;
use super::super::tests::Workspace;
use super::*;

#[test]
fn selected_output_schema_rejects_missing_extra_duplicate_and_wrong_kind() {
    assert!(
        reconcile_outputs(&[], &ActionResult::new(vec![]))
            .unwrap()
            .is_empty()
    );
    let file = GeneratedOutput::new("file", ReapiDigest::of_bytes(b"file"), false);
    let declared = [ActionOutput::new("file", ActionOutputKind::File)];
    assert!(reconcile_outputs(&declared, &ActionResult::new(vec![file.clone()])).is_ok());
    for (outputs, result) in [
        (declared.to_vec(), ActionResult::new(vec![])),
        (vec![], ActionResult::new(vec![file.clone()])),
        (
            declared.to_vec(),
            ActionResult::new(vec![file.clone(), file.clone()]),
        ),
        (
            vec![ActionOutput::new("file", ActionOutputKind::Directory)],
            ActionResult::new(vec![file.clone()]),
        ),
        (
            vec![ActionOutput::new("other", ActionOutputKind::File)],
            ActionResult::new(vec![file.clone()]),
        ),
        (
            vec![declared[0].clone(), declared[0].clone()],
            ActionResult::new(vec![file]),
        ),
    ] {
        assert!(reconcile_outputs(&outputs, &result).is_err());
    }
}

pub(super) struct PublicationWorkspace(Workspace);
impl std::ops::Deref for PublicationWorkspace {
    type Target = Workspace;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl PublicationWorkspace {
    pub(super) fn new() -> Self {
        let workspace = Workspace::new();
        let tool = TOOL.replace(
            "  /bin/cat file tree/nested/value input > done",
            "  /bin/cat file tree/nested/value input > done\n  /bin/mkdir -p result_tree/nested result_tree/empty\n  /bin/cat input > result_tree/nested/value\n  if [ \"$(/bin/cat input)\" = aaa ]; then /bin/cat input > result_tree/stale; fi\n  /bin/chmod 0644 done result_tree/nested/value",
        );
        std::fs::write(workspace.root.join("tools/tool"), tool).unwrap();
        Self(workspace)
    }

    fn publish<T: ActionChainOutputTransport>(
        &self,
        transport: &T,
    ) -> Result<AcceptedCommand<ActionChainResult<T::Output>>, BuildCommandError> {
        self.runtime
            .execute_and_publish_action_chain_with_repository_environment(
                &[TargetPattern::parse("//:one").unwrap()],
                self.owner.clone(),
                2,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[format!("file://{}/empty-registry", self.root.display())],
                Default::default(),
                Default::default(),
                transport,
            )
    }

    fn publish_and_check(
        &self,
        transport: &impl ActionChainOutputTransport<Output = ActionChainRemoteResult>,
        bytes: &[u8],
    ) -> PathBuf {
        let mut root = None;
        drop(self.publish(transport).unwrap().project(|accepted| {
            let published = accepted
                .published_outputs()
                .expect("accepted output publication");
            assert_eq!(
                published.outputs(),
                [
                    ActionOutput::new("done", ActionOutputKind::File),
                    ActionOutput::new("result_tree", ActionOutputKind::Directory),
                ]
            );
            let selected = accepted.output().selected().unwrap();
            assert!(!selected.result.output_files()[0].is_executable());
            let tree = &selected.result.output_directories()[0];
            assert_eq!(tree.directories(), ["", "empty", "nested"]);
            assert!(tree.files().iter().all(|file| !file.is_executable()));
            assert!(
                accepted
                    .output()
                    .results()
                    .iter()
                    .all(|result| result.output_blobs.is_empty())
            );
            root = Some(published.root().to_owned());
            TerminalOutput::new(0, String::new(), String::new())
        }));
        let root = root.unwrap();
        assert_outputs(&root, bytes);
        for intermediate in ["seed", "file", "tree", "emptytree"] {
            assert!(
                !root.join(intermediate).exists(),
                "intermediate output was published"
            );
        }
        root
    }
}
impl Drop for PublicationWorkspace {
    fn drop(&mut self) {
        // The fixture owns this tree; make read-only output directories removable
        // before Workspace's existing cleanup, without following any symlinks.
        fn writable(path: &Path) {
            let Ok(metadata) = path.symlink_metadata() else {
                return;
            };
            if !metadata.is_dir() {
                return;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(
                    path,
                    std::fs::Permissions::from_mode(metadata.permissions().mode() | 0o700),
                );
            }
            if let Ok(entries) = path.read_dir() {
                for entry in entries.flatten() {
                    writable(&entry.path());
                }
            }
        }
        writable(&self.root.join("bazel-out"));
    }
}

pub(super) fn transport() -> ActionChainReapiTransport {
    let endpoint = std::env::var("SLUG_V2_NATIVELINK_ENDPOINT").unwrap();
    ActionChainReapiTransport::new(
        RemoteConfig::from_args(&[&format!("--remote_executor={endpoint}")]).unwrap(),
    )
    .unwrap()
}

fn assert_outputs(root: &Path, bytes: &[u8]) {
    assert_eq!(std::fs::read(root.join("done")).unwrap(), bytes.repeat(3));
    assert_eq!(
        std::fs::read(root.join("result_tree/nested/value")).unwrap(),
        bytes
    );
    assert!(root.join("result_tree/empty").is_dir());
    assert_eq!(root.join("result_tree/stale").exists(), bytes == b"aaa");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in [
            "done",
            "result_tree",
            "result_tree/nested",
            "result_tree/nested/value",
            "result_tree/empty",
        ] {
            assert_eq!(
                std::fs::metadata(root.join(path))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o555,
                "{path}"
            );
        }
    }
}

#[test]
#[ignore = "requires supervised fresh verifying NativeLink and Linux publication"]
fn nativelink_selected_outputs_publish_modes_and_replace_tree() {
    let workspace = PublicationWorkspace::new();
    let transport = transport();
    let root = workspace.publish_and_check(&transport, b"aaa");
    std::fs::write(root.join("unrelated"), b"keep").unwrap();
    std::fs::write(workspace.root.join("input"), b"bbb").unwrap();
    assert_eq!(workspace.publish_and_check(&transport, b"bbb"), root);
    assert_eq!(std::fs::read(root.join("unrelated")).unwrap(), b"keep");
}

#[derive(Clone, Copy)]
enum DuringDownload {
    Missing,
    Corrupt,
    ChangeSource,
}

struct InterruptedDownload {
    inner: ActionChainReapiTransport,
    behavior: DuringDownload,
    workspace: PathBuf,
    old_root: PathBuf,
    starts: AtomicUsize,
    downloads: AtomicUsize,
}
impl ActionChainTransport for InterruptedDownload {
    type Session = ActionChainReapiSession;
    type Staged = StagedChainAction;
    type Output = ActionChainRemoteResult;
    type Error = RemoteExecutionError;
    async fn start(
        &self,
        inputs: Arc<PreparedActionChainInputs>,
    ) -> Result<Self::Session, Self::Error> {
        self.starts.fetch_add(1, Ordering::SeqCst);
        self.inner.start(inputs).await
    }
    async fn stage(
        &self,
        session: &mut Self::Session,
        index: usize,
    ) -> Result<Self::Staged, Self::Error> {
        self.inner.stage(session, index).await
    }
    async fn execute(
        &self,
        session: &mut Self::Session,
        index: usize,
        staged: Self::Staged,
    ) -> Result<(), Self::Error> {
        self.inner.execute(session, index, staged).await
    }
    async fn finish(&self, session: Self::Session) -> Result<Self::Output, Self::Error> {
        self.inner.finish(session).await
    }
}
impl ActionChainOutputTransport for InterruptedDownload {
    async fn stage_outputs(
        &self,
        session: &mut Self::Session,
        stages: &[PlannedActionOutputStaging],
    ) -> Result<(), Self::Error> {
        let attempt = self.downloads.fetch_add(1, Ordering::SeqCst);
        assert_outputs(&self.old_root, b"aaa");
        assert_eq!(stages.len(), 1);
        let staging = stages[0].staging();
        let selected = selected_outputs(session, stages)?.pop().unwrap();
        assert!(matches!(selected[0], SelectedOutput::File(_)));
        stage_output(&session.cache, staging, 0, selected[0]).await?;
        // The first selected file is fully staged. No old visible artifact may
        // change when a later tree download fails or the source frontier changes.
        assert_outputs(&self.old_root, b"aaa");
        if attempt == 0 {
            match self.behavior {
                DuringDownload::ChangeSource => {
                    std::fs::write(self.workspace.join("input"), b"ccc").unwrap()
                }
                DuringDownload::Missing | DuringDownload::Corrupt => {
                    let SelectedOutput::Directory(tree) = selected[1] else {
                        panic!("selected tree expected");
                    };
                    let digest = tree
                        .files()
                        .iter()
                        .find(|file| file.path() == "nested/value")
                        .unwrap()
                        .digest();
                    let root =
                        PathBuf::from(std::env::var("SLUG_V2_NATIVELINK_TEST_ROOT").unwrap());
                    for store in ["fast-content", "slow-content"] {
                        let path = root.join("cas").join(store).join("d").join(format!(
                            "{}-{}",
                            digest.hash(),
                            digest.size_bytes()
                        ));
                        assert!(
                            path.is_file(),
                            "missing verified fixture CAS file {}",
                            path.display()
                        );
                        match self.behavior {
                            DuringDownload::Missing => std::fs::remove_file(path).unwrap(),
                            DuringDownload::Corrupt => {
                                #[cfg(unix)]
                                {
                                    use std::os::unix::fs::PermissionsExt;
                                    let permissions = std::fs::Permissions::from_mode(
                                        path.metadata().unwrap().permissions().mode() | 0o200,
                                    );
                                    std::fs::set_permissions(&path, permissions).unwrap();
                                }
                                std::fs::write(path, vec![b'!'; digest.size_bytes() as usize])
                                    .unwrap();
                            }
                            DuringDownload::ChangeSource => unreachable!(),
                        }
                    }
                }
            }
        }
        for (index, output) in selected.into_iter().enumerate().skip(1) {
            stage_output(&session.cache, staging, index, output).await?;
        }
        Ok(())
    }
}

fn interrupted(
    workspace: &PublicationWorkspace,
    root: &Path,
    behavior: DuringDownload,
) -> InterruptedDownload {
    InterruptedDownload {
        inner: transport(),
        behavior,
        workspace: workspace.root.clone(),
        old_root: root.to_owned(),
        starts: AtomicUsize::new(0),
        downloads: AtomicUsize::new(0),
    }
}

#[test]
#[ignore = "requires supervised fresh verifying NativeLink and fixture CAS paths"]
fn nativelink_late_output_download_failure_preserves_old_artifact_group() {
    let workspace = PublicationWorkspace::new();
    let root = workspace.publish_and_check(&transport(), b"aaa");
    for (behavior, bytes, expected) in [
        (
            DuringDownload::Missing,
            b"missing".as_slice(),
            "CAS status 5:",
        ),
        (
            DuringDownload::Corrupt,
            b"corrupt",
            "download digest mismatch:",
        ),
    ] {
        std::fs::write(workspace.root.join("input"), bytes).unwrap();
        let transport = interrupted(&workspace, &root, behavior);
        let error = workspace.publish(&transport).unwrap_err().to_string();
        assert!(error.contains(expected), "{error}");
        assert_eq!(transport.starts.load(Ordering::SeqCst), 1);
        assert_eq!(transport.downloads.load(Ordering::SeqCst), 1);
        assert_outputs(&root, b"aaa");
    }
}

#[test]
#[ignore = "requires supervised fresh verifying NativeLink and Linux publication"]
fn nativelink_source_change_during_output_download_retries_before_publication() {
    let workspace = PublicationWorkspace::new();
    let root = workspace.publish_and_check(&transport(), b"aaa");
    std::fs::write(workspace.root.join("input"), b"bbb").unwrap();
    let transport = interrupted(&workspace, &root, DuringDownload::ChangeSource);
    assert_eq!(workspace.publish_and_check(&transport, b"ccc"), root);
    assert_eq!(transport.starts.load(Ordering::SeqCst), 2);
    assert_eq!(transport.downloads.load(Ordering::SeqCst), 2);
}
