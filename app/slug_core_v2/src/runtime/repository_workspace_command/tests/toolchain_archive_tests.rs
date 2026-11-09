//! Generic native Archive chain over five pinned original distribution payloads.
//! This isolates archive-owner closure; it is not the full Rust cquery or an RSS
//! proof. No repository/caller selector or completed-result injection is used.
use std::fs::File;
use std::fs::OpenOptions;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::OpenOptionsExt;

use super::*;

const CONFIG_SHA: &str = "933102028a1dd729d5dd0f6ab5988c91ef2dcfb67ccfaf60de1415f0019af203";
const MAX_CONFIG: u64 = 128 * 1024;
const MAX_COMPRESSED: u64 = 128 * 1024 * 1024;
const SENTINEL: &[u8] = b"five original archives completed through the generic owner\n";
const INPUTS: [(&str, &str); 5] = [
    ("rustc-nightly-x86_64-unknown-linux-gnu", "rustc"),
    ("clippy-nightly-x86_64-unknown-linux-gnu", "clippy-preview"),
    ("cargo-nightly-x86_64-unknown-linux-gnu", "cargo"),
    (
        "rustfmt-nightly-x86_64-unknown-linux-gnu",
        "rustfmt-preview",
    ),
    (
        "llvm-tools-nightly-x86_64-unknown-linux-gnu",
        "llvm-tools-preview",
    ),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Frame {
    device: u64,
    inode: u64,
    mode: u32,
    links: u64,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}
fn frame(metadata: &std::fs::Metadata) -> Frame {
    Frame {
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
        links: metadata.nlink(),
        size: metadata.len(),
        modified: (metadata.mtime(), metadata.mtime_nsec()),
        changed: (metadata.ctime(), metadata.ctime_nsec()),
    }
}
fn open_regular(path: &Path) -> (File, Frame) {
    let named = std::fs::symlink_metadata(path).unwrap();
    assert!(named.is_file() && !named.file_type().is_symlink());
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .unwrap();
    let actual = file.metadata().unwrap();
    assert!(actual.is_file());
    assert_eq!(frame(&named), frame(&actual));
    (file, frame(&actual))
}
fn unchanged(path: &Path, file: &File, before: Frame) {
    assert_eq!(frame(&file.metadata().unwrap()), before);
    assert_eq!(frame(&std::fs::symlink_metadata(path).unwrap()), before);
}
struct Input {
    path: std::path::PathBuf,
    frame: Frame,
    bytes: u64,
    digest: [u8; 32],
    operation: RepositoryWorkspaceOperation,
}
fn inputs() -> (std::path::PathBuf, Frame, Vec<Input>) {
    let config_path = std::path::PathBuf::from(
        std::env::var_os("SLUG_NATIVE_RUST_INPUTS").expect("pinned source-only inventory required"),
    );
    assert!(config_path.is_absolute());
    let (mut config_file, config_frame) = open_regular(&config_path);
    assert!(config_frame.size <= MAX_CONFIG);
    let mut bytes = Vec::new();
    (&mut config_file)
        .take(MAX_CONFIG + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes.len() as u64, config_frame.size);
    assert_eq!(hex::encode(sha2::Sha256::digest(&bytes)), CONFIG_SHA);
    unchanged(&config_path, &config_file, config_frame);
    let config: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let rows = config["objects"].as_array().unwrap();
    let mut inputs = Vec::with_capacity(INPUTS.len());
    for (stem, component) in INPUTS {
        let relative = format!("distdir/rust/{stem}.tar.xz");
        let matching = rows
            .iter()
            .filter(|row| row["destination"].as_str() == Some(relative.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(matching.len(), 1);
        let row = matching[0];
        assert_eq!(row["kind"], "tool_archive");
        let expected_size = row["size"].as_u64().unwrap();
        assert!(expected_size > 0 && expected_size <= MAX_COMPRESSED);
        let digest: [u8; 32] = hex::decode(row["sha256"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        let path = config_path.parent().unwrap().join(&relative);
        // The exact hardcoded manifest-relative names have no traversal or
        // caller-selected component. Refuse symlinked preparation directories.
        for directory in [
            path.parent().unwrap(),
            path.parent().unwrap().parent().unwrap(),
        ] {
            let metadata = std::fs::symlink_metadata(directory).unwrap();
            assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
        }
        let (mut file, before) = open_regular(&path);
        assert_eq!(before.size, expected_size);
        let mut hash = sha2::Sha256::new();
        let mut observed = 0u64;
        let mut scratch = [0; 64 * 1024];
        loop {
            let count = file.read(&mut scratch).unwrap();
            if count == 0 {
                break;
            }
            observed += count as u64;
            assert!(observed <= MAX_COMPRESSED);
            hash.update(&scratch[..count]);
        }
        assert_eq!(observed, expected_size);
        assert_eq!(<[u8; 32]>::from(hash.finalize()), digest);
        unchanged(&path, &file, before);
        let url = url::Url::from_file_path(&path).unwrap();
        // This direct generic effect explicitly supplies the pinned expected
        // checksum. It does not claim byte-identical omitted-checksum intent
        // reconstruction from the full authentic rules_rust invocation.
        let operation = RepositoryWorkspaceOperation::Archive(
            GeneratedRepositoryArchiveEffect::new(
                Arc::from([url.as_str().into()]),
                Some(digest),
                format!("{stem}/{component}").into(),
                GeneratedRepositoryArchiveFormat::TarXz,
                "".into(),
                "".into(),
            )
            .unwrap(),
        );
        inputs.push(Input {
            path,
            frame: before,
            bytes: expected_size,
            digest,
            operation,
        });
    }
    (config_path, config_frame, inputs)
}

#[test]
#[ignore = "five pinned original tar.xz archives; actual trusted factory and fixed512MiB/27+3 native supervisor required"]
fn native_five_pinned_toolchain_archives_complete_one_generic_root() {
    // All source pin validation/preparation precedes the ONE command clock.
    let (config_path, config_frame, inputs) = inputs();
    let fixture = tempfile::Builder::new()
        .prefix("slug-wp425-rolling-")
        .tempdir()
        .unwrap();
    let display = fixture.path().join("root");
    std::fs::create_dir(&display).unwrap();
    let selected = reservation(&display, 1894);
    let bank = RepositoryRootResources::new(
        RepositoryRootResourceLimits::new(512 * 1024 * 1024, 512, 8 * 1024 * 1024 * 1024).unwrap(),
    )
    .unwrap();
    let baseline = bank.snapshot().retained_bytes;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(27);
    let scope = SourceServiceScope::new_command(
        selected.workspace().clone(),
        deadline,
        deadline + Duration::from_secs(3),
        SourceServiceLimits::default(),
        bank.clone(),
    )
    .unwrap();
    assert!(Arc::ptr_eq(scope.command_resources().unwrap(), &bank));
    let (mut command, fd) = start(selected.clone(), bank.clone(), scope.clone());
    let mut previous_bodies = 0;
    let mut previous_decoded = 0;
    for (index, input) in inputs.iter().enumerate() {
        command = apply(
            command,
            &selected,
            input.operation.clone(),
            &runtime,
            &scope,
        );
        let transcript = &command.epoch().transcripts().unwrap()[0];
        assert_eq!(transcript.records().len(), index + 1);
        let record = transcript.records().last().unwrap();
        assert_eq!(
            record.result(),
            &RepositoryWorkspaceResult::Archive(input.digest)
        );
        assert_eq!(record.request().operation(), &input.operation);
        let bodies = transcript.retention().body_bytes();
        let decoded = scope.snapshot().decoded_bytes;
        assert!(
            bodies > previous_bodies,
            "each archive contributes real unique body data"
        );
        assert!(decoded > previous_decoded, "each archive really decodes");
        assert!(bodies <= 1024 * 1024 * 1024);
        previous_bodies = bodies;
        previous_decoded = decoded;
        eprintln!(
            "NativeToolchainArchive index={index} bodies={bodies} decoded={decoded} peak={}",
            bank.snapshot().peak_retained_bytes
        );
    }
    // Complete decoded tar sizes from the independently verified original
    // envelope. This scalar is a progress discriminator, never byte identity.
    assert_eq!(previous_decoded, 654_041_088);
    assert_eq!(scope.snapshot().source_attempts, INPUTS.len() as u64);
    assert_eq!(
        scope.snapshot().transferred_bytes,
        inputs.iter().map(|input| input.bytes).sum::<u64>()
    );
    command = apply(
        command,
        &selected,
        RepositoryWorkspaceOperation::File {
            path: "archive-chain-kept".into(),
            body: Arc::from(SENTINEL),
            mode: 0o644,
        },
        &runtime,
        &scope,
    );
    command = apply(
        command,
        &selected,
        RepositoryWorkspaceOperation::Read("archive-chain-kept".into()),
        &runtime,
        &scope,
    );
    let transcript = &command.epoch().transcripts().unwrap()[0];
    assert_eq!(transcript.records().len(), 7);
    assert!(
        matches!(transcript.records().last().unwrap().result(), RepositoryWorkspaceResult::Read(body) if body.as_ref()==SENTINEL)
    );
    let request = final_request(&command, &selected);
    let (command, sealed) = command
        .seal_for_materialization(request, &scope, &|| true)
        .unwrap();
    command.check_epoch_body_coverage(&scope, &|| true).unwrap();
    let (request, root, history) = sealed.into_parts();
    let weak = Arc::downgrade(&root);
    let mut reader = root
        .open_member("archive-chain-kept", &scope, &|| true)
        .unwrap();
    drop((command, request, root, history));
    let mut actual = Vec::new();
    reader.read_to_end(&mut actual).unwrap();
    assert_eq!(actual, SENTINEL);
    assert!(weak.upgrade().is_some());
    drop(reader);
    assert!(weak.upgrade().is_none());
    assert!(
        scope.check().is_ok(),
        "the original27s work clock was never refreshed"
    );
    let counts = scope.snapshot();
    assert_eq!(
        (counts.live_buffer_bytes, counts.live_decoder_bytes),
        (0, 0)
    );
    // The caller-owned Source scope itself has positive bank control until its
    // final Arc retires, so release it before asserting the native baseline.
    drop(scope);
    release(&bank, baseline, fd);
    assert_eq!(
        frame(&std::fs::symlink_metadata(&config_path).unwrap()),
        config_frame
    );
    for input in inputs {
        assert_eq!(
            frame(&std::fs::symlink_metadata(input.path).unwrap()),
            input.frame
        );
    }
}
