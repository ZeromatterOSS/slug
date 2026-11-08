//! Linked General artifacts retain their logical repository while terminal
//! bytes come from an independently captured Local Source. Reuses the genuine
//! original transport fixture, actual Analysis and verifying NativeLink.

use super::*;

const LINKED_REPOSITORY: &str = r#"def _impl(ctx):
    if ctx.execute(['/bin/true']).return_code != 0:
        fail('warmup')
    source = Label('@@SOURCE_REPO//:data')
    path = ctx.path(source)
    ctx.symlink(path, 'data')
    ctx.symlink(Label('@@SOURCE_REPO//:executable_data'), 'executable_data')
    ctx.symlink(Label('@@SOURCE_REPO//:tool'), 'tool')
    result = ctx.execute(['/usr/bin/python3', '-c', 'import os, sys; os.symlink(sys.argv[1], "payload-link")', source])
    if result.return_code != 0 or ctx.read('payload-link') != ctx.attr.content:
        fail('payload-created Source link did not retain its exact declared route')
    if not ctx.path('data').exists:
        fail('typed Source link missing')
    ctx.file('BUILD.bazel', "exports_files(['data', 'executable_data', 'tool'])\n", executable=False)
repo = repository_rule(implementation=_impl, attrs={'content': attr.string()})
"#;

fn linked_fixture() -> Fixture {
    let mut fixture = Fixture::new();
    fixture.module.push_str("\nbazel_dep(name='linked_source',version='1.0')\nlocal_path_override(module_name='linked_source',path='linked_source')\n");
    let source = fixture.root.join("linked_source");
    fs::create_dir(&source).unwrap();
    fs::write(
        source.join("MODULE.bazel"),
        "module(name='linked_source',version='1.0')\n",
    )
    .unwrap();
    fs::write(
        source.join("BUILD.bazel"),
        "exports_files(['data', 'executable_data', 'tool'])\n",
    )
    .unwrap();
    fs::write(
        fixture.root.join("repo.bzl"),
        LINKED_REPOSITORY.replace("SOURCE_REPO", "linked_source+"),
    )
    .unwrap();
    linked_content(&fixture, "aaa");
    fixture
}
fn linked_content(fixture: &Fixture, bytes: &str) {
    fixture.content(bytes);
    for (name, body, mode) in [
        ("data", bytes, 0o444),
        ("executable_data", bytes, 0o555),
        (
            "tool",
            "#!/bin/sh\nset -eu\n/bin/cat input > \"$1\"\n/bin/cat input > \"$2\"\n",
            0o555,
        ),
    ] {
        let path = fixture.root.join("linked_source").join(name);
        if path.exists() {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        }
        fs::write(&path, body).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }
}
struct LinkedTransport(CheckedTransport);
impl ActionChainTransport for LinkedTransport {
    type Session = ActionChainReapiSession;
    type Staged = StagedChainAction;
    type Output = ActionChainRemoteResult;
    type Error = RemoteExecutionError;
    async fn start(
        &self,
        inputs: NativeActionChainInputs,
        args: &dyn slug_build_api_v2::ArgsDirectoryContext,
    ) -> Result<Self::Session, Self::Error> {
        let mut linked = 0;
        for source in inputs.sources().filter(|s| s.anchor().is_some()) {
            linked += 1;
            let anchor = source.anchor().unwrap();
            assert_ne!(source.content_origin().namespace(), source.namespace());
            assert!(
                source
                    .requested_path()
                    .as_path()
                    .starts_with(anchor.root().as_path())
            );
            assert!(
                !source
                    .real_path()
                    .as_path()
                    .starts_with(anchor.root().as_path())
            );
            assert!(source.content_origin().anchor().is_some());
            assert!(
                source
                    .label()
                    .package()
                    .repo()
                    .to_string()
                    .contains("generated")
            );
        }
        assert_eq!(
            linked, 3,
            "every General data/tool artifact follows a real Source link"
        );
        self.0.start(inputs, args).await
    }
    async fn stage(
        &self,
        session: &mut Self::Session,
        index: usize,
    ) -> Result<Self::Staged, Self::Error> {
        self.0.stage(session, index).await
    }
    async fn execute(
        &self,
        session: &mut Self::Session,
        index: usize,
        staged: Self::Staged,
    ) -> Result<(), Self::Error> {
        self.0.execute(session, index, staged).await
    }
    async fn finish(&self, session: Self::Session) -> Result<Self::Output, Self::Error> {
        self.0.finish(session).await
    }
}
impl ActionChainOutputTransport for LinkedTransport {
    async fn stage_outputs(
        &self,
        session: &mut Self::Session,
        stages: &[PlannedActionOutputStaging],
    ) -> Result<(), Self::Error> {
        self.0.stage_outputs(session, stages).await
    }
}
#[test]
#[ignore = "actual compiled CLI + Source-linked original artifacts + fresh verifying NativeLink"]
fn nativelink_source_linked_general_artifacts_preserve_identity_cache_runfiles_and_retired_data() {
    let mut fixture = linked_fixture();
    let transport = LinkedTransport(CheckedTransport::new());
    let mut original = None;
    let mut published = None;
    for (iteration, bytes) in ["aaa", "aaa", "bbb", "aaa"].into_iter().enumerate() {
        if iteration >= 2 {
            linked_content(&fixture, bytes);
        }
        drop(fixture.publish(&transport).unwrap().project(|accepted| {
            let accepted = accepted.as_ref().unwrap();
            let remotes = accepted
                .output()
                .unwrap()
                .results()
                .iter()
                .filter_map(ActionChainStepResult::remote)
                .collect::<Vec<_>>();
            assert_eq!(remotes.len(), 3);
            assert_eq!(
                remotes.iter().map(|r| r.evidence.ac_hits).sum::<u64>(),
                [0, 3, 1, 3][iteration]
            );
            let digests = remotes
                .iter()
                .map(|r| r.action_digest.clone())
                .collect::<Vec<_>>();
            if let Some(prior) = &original {
                assert_eq!(prior == &digests, bytes == "aaa");
            } else {
                original = Some(digests);
            }
            let root = published_root(accepted);
            authored::check_tree(&fixture.root, &root, bytes.as_bytes());
            assert_eq!(
                fs::read(root.join("consumer.out")).unwrap(),
                bytes.as_bytes().repeat(3)
            );
            published = Some(root);
            TerminalOutput::new(0, String::new(), String::new())
        }));
        // One retained A owner establishes historical data lifetime. Retire
        // redundant wrappers before the next command's fixed backing admission.
        transport.0.held.lock().unwrap().truncate(1);
    }
    assert!(transport.0.physical_tool_uploads.load(Ordering::Relaxed) > 0);
    // Old data remains readable after B and restoration, through its original
    // General logical owner and retained Source FD, not the edited Local path.
    for inputs in transport.0.held.lock().unwrap().iter() {
        for (index, source) in inputs
            .sources()
            .enumerate()
            .filter(|(_, s)| s.anchor().is_some())
        {
            let mut body = Vec::new();
            inputs
                .open_source(index)
                .unwrap()
                .read_to_end(&mut body)
                .unwrap();
            assert_eq!(
                ReapiDigest::of_bytes(&body).size_bytes(),
                source.digest().size_bytes()
            );
            assert_eq!(
                ReapiDigest::of_bytes(&body).hash(),
                source
                    .digest()
                    .sha256()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            );
        }
    }
    fixture.retired(&transport.0);
    authored::check_tree(&fixture.root, &published.unwrap(), b"aaa");
}
