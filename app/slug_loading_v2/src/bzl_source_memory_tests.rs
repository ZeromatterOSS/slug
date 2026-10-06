use dice::DetectCycles;
use dice::Dice;

use super::*;
use crate::keys::WorkspaceSnapshot;
use crate::keys::WorkspaceSnapshotKey;

#[tokio::test]
async fn parsed_source_shares_workspace_storage_and_survives_its_owners() {
    let workspace = PathBuf::from("/workspace");
    let path = workspace.join("shared.bzl");
    let source = Arc::new(
        "load(\":first.bzl\", \"first\")\nload(\":second.bzl\", \"second\")\nvalue = \"é\"\n"
            .to_owned(),
    );
    let expected_digest = digest(&source);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: workspace.clone(),
            },
            Arc::new(WorkspaceSnapshot {
                files: Arc::new(
                    [(path.clone(), WorkspaceFileValue::Present(source.clone()))]
                        .into_iter()
                        .collect(),
                ),
            }),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    let result = transaction
        .compute(&BzlParseKey { workspace, path })
        .await
        .unwrap();
    let parsed = result.as_ref().as_ref().unwrap();
    assert_eq!(parsed.source.as_ptr(), source.as_ptr());
    assert_eq!(parsed.source.as_str(), source.as_str());
    assert_eq!(parsed.source_digest, expected_digest);
    assert_eq!(parsed.loads, [":first.bzl", ":second.bzl"]);
    let held = parsed.clone();
    assert_eq!(held.source.as_ptr(), parsed.source.as_ptr());

    drop(result);
    drop(transaction);
    drop(dice);
    drop(source);
    assert!(held.source.contains("value = \"é\""));
    assert_eq!(held.source_digest, expected_digest);

    let independent = ParsedBzl {
        source: Arc::new(held.source.as_ref().clone()),
        loads: held.loads.clone(),
        source_digest: expected_digest,
    };
    assert_ne!(held.source.as_ptr(), independent.source.as_ptr());
    assert!(BzlParseKey::equality(
        &Arc::new(Ok(held)),
        &Arc::new(Ok(independent)),
    ));
}

// Opt-in profiles receive only one verified regular-file descriptor as stdin.
// They do not evaluate a module, dispatch child loads, or certify a repository.
#[cfg(target_os = "linux")]
fn native_source_profile(name: &str, expected_bytes: usize, expected_sha256: &str) {
    use std::fs::File;
    use std::io::Read;
    use std::os::fd::AsFd;

    use sha2::Digest;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;
    use starlark::syntax::StringEncoding;

    fn phase(name: &str) {
        let status = std::fs::read_to_string("/proc/self/status").unwrap();
        let rss = status
            .lines()
            .find_map(|line| line.strip_prefix("VmRSS:"))
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<usize>()
            .unwrap();
        println!("SLUG_NATIVE_PHASE {name} {rss}");
    }

    phase("baseline");
    assert!(expected_bytes < 16 * 1024 * 1024);
    // A direct owned duplicate shares the supervisor's file offset. Avoid stdin's
    // buffered reader, which could prefetch beyond the one-byte EOF probe.
    let mut file = File::from(std::io::stdin().as_fd().try_clone_to_owned().unwrap());
    assert!(file.metadata().unwrap().is_file());
    assert_eq!(file.metadata().unwrap().len(), expected_bytes as u64);
    let mut bytes = Vec::with_capacity(expected_bytes);
    bytes.resize(expected_bytes, 0);
    file.read_exact(&mut bytes).unwrap();
    let mut probe = [0u8; 1];
    assert_eq!(file.read(&mut probe).unwrap(), 0, "source grew beyond pin");
    drop(file);
    assert_eq!(
        format!("{:x}", sha2::Sha256::digest(&bytes)),
        expected_sha256
    );
    let transferred_pointer = bytes.as_ptr();
    let source = Arc::new(String::from_utf8(bytes).expect("pinned source is UTF8"));
    assert_eq!(source.as_ptr(), transferred_pointer);
    let source_weak = Arc::downgrade(&source);
    println!(
        "SLUG_NATIVE_SOURCE {} {} {}",
        source.len(),
        source.capacity(),
        expected_sha256,
    );
    phase("after_input");

    // This whole String scratch is required by the current production parser API.
    // It is an explicitly measured copy, separate from the retained source Arc.
    let parser_source = source.as_ref().clone();
    let parser_pointer = parser_source.as_ptr();
    assert_ne!(parser_pointer, source.as_ptr());
    println!("SLUG_NATIVE_PARSER_CAPACITY {}", parser_source.capacity());
    let ast = match AstModule::parse_with_string_encoding(
        name,
        parser_source,
        &Dialect::Bazel,
        StringEncoding::BazelInternal,
    ) {
        Ok(ast) => ast,
        Err(_) => panic!("native source syntax refused"),
    };
    assert_eq!(
        ast.file_span(ast.statement().span).file.source().as_ptr(),
        parser_pointer,
    );
    phase("after_parse");
    let compact_loads = ast
        .loads()
        .into_iter()
        .map(|load| {
            assert!(load.module_id.len() <= 1024);
            let span = load.span.resolve_span_for_reporting();
            (
                load.module_id.to_owned(),
                span.begin.line,
                span.begin.column,
                span.end.line,
                span.end.column,
            )
        })
        .collect::<Vec<_>>();
    assert!(compact_loads.len() <= 128);
    drop(ast);
    phase("after_ast_retirement");
    assert_eq!(Arc::strong_count(&source), 1);
    assert!(source_weak.upgrade().is_some());
    println!(
        "SLUG_NATIVE_LOAD_STORAGE {} {}",
        compact_loads.len(),
        compact_loads
            .iter()
            .map(|load| load.0.capacity())
            .sum::<usize>(),
    );
    drop(source);
    assert!(source_weak.upgrade().is_none());
    phase("after_source_retirement");
    // Only owned names and scalar locations remain; no FileSpan/CodeMap is kept.
    for (label, begin_line, begin_column, end_line, end_column) in &compact_loads {
        print!("SLUG_NATIVE_LOAD {begin_line} {begin_column} {end_line} {end_column} ");
        for byte in label.bytes() {
            print!("{byte:02x}");
        }
        println!();
    }
    drop(compact_loads);
    phase("after_final_drop");
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires the bounded native source profile supervisor and verified stdin"]
fn native_bazel_internal_rustc_source_profile() {
    native_source_profile(
        "rust/private/rustc.bzl",
        145_040,
        "a7712508f50e5952f3f51e33c98acbe8ba39554e9c6f6edc26f16820d7c2a9e4",
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires the bounded native source profile supervisor and verified stdin"]
fn native_bazel_internal_known_shas_source_profile() {
    native_source_profile(
        "rust/private/known_shas.bzl",
        4_749_878,
        "f35c95378eb10fd57f719e42fd88fea2b1c15b5049556daf8cdc18ef385ff446",
    );
}
