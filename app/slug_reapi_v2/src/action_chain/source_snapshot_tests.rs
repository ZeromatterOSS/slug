//! Actual captured source families continue through ordinary Analysis, native
//! source staging, remote execution and durable runfiles without Host reopening.
use std::fs;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use base64::Engine as _;
use slug_core_v2::runtime::ActionChainOutputTransport;
use slug_core_v2::runtime::NativeSourceReader;
use slug_core_v2::runtime::PlannedActionOutputStaging;
use slug_core_v2::runtime::TerminalOutput;
use slug_workspace_v2::PathObservationInstanceId;

use super::original_repository_tests::Fixture;
use super::original_repository_tests::REPOSITORY;
use super::original_repository_tests::authored;
use super::original_repository_tests::published_root;
use super::*;

#[derive(Clone, Copy, Debug)]
enum Family {
    Local,
    Archive,
    Builtin,
    Workspace,
}
impl Family {
    fn label(self) -> &'static str {
        match self {
            Self::Local => "@@source_payload+//:payload",
            Self::Archive => "@@source_archive+//:payload",
            Self::Builtin => "@@bazel_tools//tools/build_defs/repo:utils.bzl",
            Self::Workspace => "//:input",
        }
    }
    fn repository(self) -> &'static str {
        match self {
            Self::Local => "source_payload+",
            Self::Archive => "source_archive+",
            Self::Builtin => "bazel_tools",
            Self::Workspace => "",
        }
    }
    fn runfile(self) -> &'static str {
        match self {
            Self::Local => "source_payload+/payload",
            Self::Archive => "source_archive+/payload",
            Self::Builtin => "bazel_tools/tools/build_defs/repo/utils.bzl",
            Self::Workspace => "_main/input",
        }
    }
    fn payload(self, generation: &str) -> Vec<u8> {
        match self {
            Self::Local => format!("local:{generation}\n").into_bytes(),
            Self::Archive => format!("archive:{generation}\n").into_bytes(),
            Self::Builtin => include_bytes!(
                "../../../slug_bzlmod_v2/builtin/bazel_tools/tools/build_defs/repo/utils.bzl"
            )
            .to_vec(),
            Self::Workspace => b"aaa".to_vec(),
        }
    }
}
const SOURCE_TOOL: &str = "#!/bin/sh\nset -eu\n/bin/cat input > \"$1\"\n/bin/cat input > \"$2\"\n";

struct SourceFixture {
    common: Fixture,
    family: Family,
}
impl SourceFixture {
    fn new(family: Family) -> Self {
        let common = Fixture::new();
        let root = &common.root;
        let prefix = format!(
            r#"    if ctx.execute(['/bin/true']).return_code != 0: fail('warmup')
    captured = Label('{}')
    path = ctx.path(captured)
    if not ctx.read(captured): fail('empty captured source')
    if ctx.execute(['/bin/sh', '-c', 'test -s "$1"', 'source', captured]).return_code != 0: fail('source view')
    ctx.file('captured-display', str(path), executable=False)
"#,
            family.label()
        );
        fs::write(
            root.join("repo.bzl"),
            REPOSITORY.replace(
                "    ctx.file('seed'",
                &format!("{prefix}    ctx.file('seed'"),
            ),
        )
        .unwrap();
        let mut build = fs::read_to_string(root.join("BUILD.bazel"))
            .unwrap()
            .replace(
                "data=['@generated//:data', '@generated//:executable_data']",
                &format!(
                    "data=['@generated//:data', '@generated//:executable_data', '{}']",
                    family.label()
                ),
            );
        if matches!(family, Family::Local | Family::Archive) {
            build = build.replace(
                "tool='@generated//:tool'",
                &format!("tool='@@{}//:tool'", family.repository()),
            );
        }
        fs::write(root.join("BUILD.bazel"), build).unwrap();
        fs::write(
            root.join("tools/consumer"),
            format!(
                "#!/bin/sh\nset -eu\n\"./$1\" > \"$2\"\n/bin/cat \"$1.runfiles/{}\" >> \"$2\"\n",
                family.runfile()
            ),
        )
        .unwrap();
        let fixture = Self { common, family };
        fixture.generation("A");
        fixture
    }
    fn generation(&self, generation: &str) {
        let root = &self.common.root;
        let mut module = self.common.module.clone();
        match self.family {
            Family::Local => {
                let directory = root.join("source_payload");
                fs::create_dir_all(&directory).unwrap();
                fs::write(
                    directory.join("MODULE.bazel"),
                    "module(name='source_payload',version='1.0')\n",
                )
                .unwrap();
                fs::write(
                    directory.join("BUILD.bazel"),
                    "exports_files(['payload','tool'])\n",
                )
                .unwrap();
                fs::write(directory.join("payload"), self.family.payload(generation)).unwrap();
                fs::write(directory.join("tool"), SOURCE_TOOL).unwrap();
                fs::set_permissions(directory.join("tool"), fs::Permissions::from_mode(0o755))
                    .unwrap();
                module.push_str("\nbazel_dep(name='source_payload',version='1.0')\nlocal_path_override(module_name='source_payload',path='source_payload')\n");
            }
            Family::Archive => {
                self.archive(generation);
                module.push_str("\nbazel_dep(name='source_archive',version='1.0')\n");
            }
            Family::Builtin | Family::Workspace => {}
        }
        authored::content(root, &module, "aaa");
    }
    fn archive(&self, generation: &str) {
        let root = &self.common.root;
        let registry = root.join("empty-registry");
        let metadata = registry.join("modules/source_archive/1.0");
        fs::create_dir_all(metadata.join("patches")).unwrap();
        fs::create_dir_all(metadata.join("overlay")).unwrap();
        fs::write(registry.join("bazel_registry.json"), "{}").unwrap();
        fs::write(
            metadata.join("MODULE.bazel"),
            "module(name='source_archive',version='1.0')\n",
        )
        .unwrap();
        let mut tar = tar::Builder::new(
            flate2::GzBuilder::new()
                .mtime(0)
                .write(Vec::new(), flate2::Compression::default()),
        );
        for (name, body, mode) in [
            (
                "BUILD.bazel",
                b"exports_files(['payload','tool','overlay'])\n".as_slice(),
                0o644,
            ),
            (
                "MODULE.bazel",
                b"module(name='unverified_archive')\n".as_slice(),
                0o644,
            ),
            ("payload", b"archive:pre\n".as_slice(), 0o644),
            ("tool", SOURCE_TOOL.as_bytes(), 0o755),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(mode);
            header.set_uid(0);
            header.set_gid(0);
            header.set_mtime(0);
            header.set_cksum();
            tar.append_data(&mut header, name, body).unwrap();
        }
        let archive = tar.into_inner().unwrap().finish().unwrap();
        let archive_path = root.join("source-payload.tar.gz");
        fs::write(&archive_path, &archive).unwrap();
        let patch = format!(
            "--- a/payload\n+++ b/payload\n@@ -1 +1 @@\n-archive:pre\n+archive:{generation}\n"
        );
        let overlay = b"overlay-verified\n";
        fs::write(metadata.join("patches/payload.patch"), &patch).unwrap();
        fs::write(metadata.join("overlay/overlay"), overlay).unwrap();
        let sri = |bytes: &[u8]| {
            let digest = ReapiDigest::of_bytes(bytes);
            let raw = (0..32)
                .map(|index| {
                    u8::from_str_radix(&digest.hash()[index * 2..index * 2 + 2], 16).unwrap()
                })
                .collect::<Vec<_>>();
            format!(
                "sha256-{}",
                base64::engine::general_purpose::STANDARD.encode(raw)
            )
        };
        fs::write(metadata.join("source.json"), format!(r#"{{"url":"file://{}","integrity":"{}","archive_type":"tar.gz","strip_prefix":"","patches":{{"payload.patch":"{}"}},"overlay":{{"overlay":"{}"}},"patch_strip":1}}"#, archive_path.display(), sri(&archive), sri(patch.as_bytes()), sri(overlay))).unwrap();
    }
}

struct SourceTransport {
    inner: ActionChainReapiTransport,
    family: Family,
    expected: Mutex<Vec<u8>>,
    held: Mutex<Vec<NativeActionChainInputs>>,
    first_reader: Mutex<Option<NativeSourceReader>>,
    generations: Mutex<Vec<(PathBuf, PathObservationInstanceId)>>,
    payload_uploads: AtomicUsize,
    starts: AtomicUsize,
}
impl SourceTransport {
    fn new(family: Family) -> Self {
        let endpoint = std::env::var("SLUG_V2_NATIVELINK_ENDPOINT").unwrap();
        Self {
            inner: ActionChainReapiTransport::new(
                RemoteConfig::from_args(&[&format!("--remote_executor={endpoint}")]).unwrap(),
            )
            .unwrap(),
            family,
            expected: Mutex::new(family.payload("A")),
            held: Mutex::new(Vec::new()),
            first_reader: Mutex::new(None),
            generations: Mutex::new(Vec::new()),
            payload_uploads: AtomicUsize::new(0),
            starts: AtomicUsize::new(0),
        }
    }
}
impl ActionChainTransport for SourceTransport {
    type Session = ActionChainReapiSession;
    type Staged = StagedChainAction;
    type Output = ActionChainRemoteResult;
    type Error = RemoteExecutionError;
    async fn start(
        &self,
        inputs: NativeActionChainInputs,
        args: &dyn slug_build_api_v2::ArgsDirectoryContext,
    ) -> Result<Self::Session, Self::Error> {
        self.starts.fetch_add(1, Ordering::Relaxed);
        let (index, source) = inputs
            .sources()
            .enumerate()
            .find(|(_, source)| {
                source.label().package().repo().as_str() == self.family.repository()
                    && match self.family {
                        Family::Builtin => source.label().target().as_str() == "utils.bzl",
                        Family::Workspace => source.label().target().as_str() == "input",
                        _ => source.label().target().as_str() == "payload",
                    }
            })
            .expect("actual captured-source artifact from ordinary Analysis");
        let anchor = source
            .anchor()
            .expect("current captured source must not use Host fallback");
        assert!(fs::symlink_metadata(anchor.root().as_path()).is_err());
        assert!(
            anchor
                .root()
                .as_path()
                .starts_with("/__slug_repositories__/s1")
        );
        let mut reader = inputs.open_source(index).unwrap();
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, *self.expected.lock().unwrap());
        let digest = ReapiDigest::of_bytes(&bytes);
        assert_eq!(digest.size_bytes(), source.digest().size_bytes());
        assert_eq!(
            digest.hash(),
            source
                .digest()
                .sha256()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        let slug_workspace_v2::PathObservationNamespace::Materialization(instance) =
            source.namespace()
        else {
            panic!("source namespace")
        };
        self.generations
            .lock()
            .unwrap()
            .push((anchor.root().as_path().to_owned(), instance));
        {
            let mut first = self.first_reader.lock().unwrap();
            if first.is_none() {
                *first = Some(reader);
            }
        }
        self.held.lock().unwrap().push(inputs.clone());
        self.inner.start(inputs, args).await
    }
    async fn stage(
        &self,
        session: &mut Self::Session,
        index: usize,
    ) -> Result<Self::Staged, Self::Error> {
        let staged = self.inner.stage(session, index).await?;
        if let StagedPayload::Remote { uploaded, .. } = &staged.payload {
            let digest = ReapiDigest::of_bytes(&self.expected.lock().unwrap());
            if uploaded.contains(&digest) {
                self.payload_uploads.fetch_add(1, Ordering::Relaxed);
            }
        }
        Ok(staged)
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
impl ActionChainOutputTransport for SourceTransport {
    async fn stage_outputs(
        &self,
        session: &mut Self::Session,
        stages: &[PlannedActionOutputStaging],
    ) -> Result<(), Self::Error> {
        self.inner.stage_outputs(session, stages).await
    }
}

fn run(family: Family) {
    let mut fixture = SourceFixture::new(family);
    let transport = SourceTransport::new(family);
    let mut output_root = None;
    let generations: &[&str] = if matches!(family, Family::Workspace) {
        &["A"]
    } else {
        &["A", "A", "B"]
    };
    for (index, generation) in generations.iter().enumerate() {
        if index != 0 {
            fixture.generation(generation);
            *transport.expected.lock().unwrap() = family.payload(generation);
        }
        drop(
            fixture
                .common
                .publish(&transport)
                .unwrap_or_else(|error| panic!(
                    "captured-source {family:?} command {index} failed; transport_starts={}: {error}",
                    transport.starts.load(Ordering::Relaxed)
                ))
                .project(|accepted| {
                    let accepted = accepted.as_ref().unwrap();
                    let remotes = accepted
                        .output()
                        .unwrap()
                        .results()
                        .iter()
                        .filter_map(ActionChainStepResult::remote)
                        .collect::<Vec<_>>();
                    assert_eq!(remotes.len(), 3);
                    let expected_hits = if index == 0 {
                        0
                    } else if index == 1 || matches!(family, Family::Builtin) {
                        3
                    } else {
                        2
                    };
                    assert_eq!(
                        remotes
                            .iter()
                            .map(|remote| remote.evidence.ac_hits)
                            .sum::<u64>(),
                        expected_hits
                    );
                    if index == 0 {
                        assert!(transport.payload_uploads.load(Ordering::Relaxed) > 0);
                    } else if index == 1 || matches!(family, Family::Builtin) {
                        assert!(
                            remotes
                                .iter()
                                .all(|remote| remote.evidence.uploaded_digests.is_empty())
                        );
                    }
                    let root = published_root(accepted);
                    authored::check_tree(&fixture.common.root, &root, b"aaa");
                    let mut expected = b"aaa".repeat(3);
                    expected.extend_from_slice(&family.payload(generation));
                    assert_eq!(fs::read(root.join("consumer.out")).unwrap(), expected);
                    let backing =
                        fs::read_link(root.join("binary.runfiles").join(family.runfile())).unwrap();
                    assert!(
                        backing.starts_with(
                            fixture
                                .common
                                .root
                                .join("bazel-out/.slug-runfiles-sources/v1")
                        )
                    );
                    assert_eq!(fs::read(&backing).unwrap(), family.payload(generation));
                    assert_eq!(
                        fs::metadata(&backing).unwrap().permissions().mode() & 0o777,
                        0o444
                    );
                    output_root = Some(root);
                    TerminalOutput::new(0, String::new(), String::new())
                }),
        );
        let mut first = transport.first_reader.lock().unwrap();
        let reader = first.as_mut().unwrap();
        reader.seek(SeekFrom::Start(0)).unwrap();
        let mut old = Vec::new();
        reader.read_to_end(&mut old).unwrap();
        assert_eq!(
            old,
            family.payload("A"),
            "B must not reauthorize A through its new scope or Host name"
        );
    }
    let observed = transport.generations.lock().unwrap();
    assert_eq!(observed.len(), generations.len());
    for (index, (display, instance)) in observed.iter().enumerate() {
        assert_eq!(*display, observed[0].0);
        assert!(
            observed[..index]
                .iter()
                .all(|(_, earlier)| earlier != instance)
        );
    }
    drop(observed);
    drop(fixture.common.runtime.take());
    assert!(fixture.common.resources.snapshot().backing_bytes > 0);
    transport.held.lock().unwrap().clear();
    transport.first_reader.lock().unwrap().take();
    let now = fixture.common.resources.snapshot();
    assert_eq!(
        (now.native_fds, now.backing_bytes, now.retained_bytes),
        (0, 0, fixture.common.resources.baseline().retained_bytes)
    );
    authored::check_tree(&fixture.common.root, &output_root.unwrap(), b"aaa");
}

#[test]
#[ignore = "requires compiled CLI and supervised fresh verifying NativeLink"]
fn nativelink_captured_local_sources_analysis_cas_runfiles_and_fresh_scope() {
    run(Family::Local);
}
#[test]
#[ignore = "requires compiled CLI and supervised fresh verifying NativeLink"]
fn nativelink_captured_bcr_sources_analysis_cas_runfiles_and_fresh_scope() {
    run(Family::Archive);
}
#[test]
#[ignore = "requires compiled CLI and supervised fresh verifying NativeLink"]
fn nativelink_captured_builtin_sources_analysis_cas_runfiles_and_fresh_scope() {
    run(Family::Builtin);
}
#[test]
#[ignore = "requires compiled CLI and supervised fresh verifying NativeLink"]
fn nativelink_captured_workspace_sources_analysis_cas_runfiles_first_command() {
    run(Family::Workspace);
}
