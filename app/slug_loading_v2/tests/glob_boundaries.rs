use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use slug_loading_v2::file_discovery::BUILD_FILE_PRIMARY;
use slug_loading_v2::glob::GlobError;
use slug_loading_v2::glob::GlobSpec;
use slug_loading_v2::glob::expand_glob;

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("slug-loading-glob-{name}-{nanos}"));
    fs::create_dir_all(&root).unwrap();
    root
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

#[test]
fn glob_skips_subpackages_and_records_watched_dirs() {
    let pkg = scratch("boundary");
    write(&pkg.join("keep.txt"), "keep\n");
    write(&pkg.join("skip.txt"), "skip\n");
    write(&pkg.join("sub/child.txt"), "child\n");
    write(&pkg.join("subpkg").join(BUILD_FILE_PRIMARY), "# boundary\n");
    write(&pkg.join("subpkg/hidden.txt"), "hidden\n");

    let expansion = expand_glob(
        &pkg,
        &GlobSpec {
            includes: vec![
                "*.txt".to_owned(),
                "sub/*.txt".to_owned(),
                "subpkg/*.txt".to_owned(),
            ],
            excludes: vec!["skip.txt".to_owned()],
            allow_empty: true,
        },
    )
    .unwrap();

    assert_eq!(expansion.matches, vec!["keep.txt", "sub/child.txt"]);
    assert_eq!(expansion.skipped_subpackages, vec!["subpkg"]);
    assert!(expansion.watched_dirs.contains(&".".to_owned()));
    assert!(expansion.watched_dirs.contains(&"sub".to_owned()));
    assert!(expansion.watched_dirs.contains(&"subpkg".to_owned()));
}

#[test]
fn allow_empty_false_is_checked_per_include_pattern() {
    let pkg = scratch("allow-empty");
    write(&pkg.join("keep.txt"), "keep\n");
    write(&pkg.join("subpkg").join(BUILD_FILE_PRIMARY), "# boundary\n");
    write(&pkg.join("subpkg/hidden.txt"), "hidden\n");

    let error = expand_glob(
        &pkg,
        &GlobSpec {
            includes: vec!["*.txt".to_owned(), "subpkg/*.txt".to_owned()],
            excludes: Vec::new(),
            allow_empty: false,
        },
    )
    .unwrap_err();

    assert_eq!(
        error,
        GlobError::EmptyPattern {
            pattern: "subpkg/*.txt".to_owned()
        }
    );
    assert!(error.to_string().contains("allow_empty is set to False"));
}
