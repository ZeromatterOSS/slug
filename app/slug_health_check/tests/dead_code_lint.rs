use std::fs;
use std::path::Path;

fn visit_rs_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).expect("read_dir should succeed") {
        let entry = entry.expect("directory entry should load");
        let path = entry.path();
        if path.is_dir() {
            visit_rs_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

#[test]
fn shipping_modules_do_not_use_unconditional_dead_code_allow() {
    let src_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut rs_files = Vec::new();
    visit_rs_files(&src_root, &mut rs_files);

    rs_files.sort();

    let mut offenders = Vec::new();
    for file in rs_files {
        let contents = fs::read_to_string(&file).expect("source file should be readable");
        if contents
            .lines()
            .any(|line| line.trim_start() == "#![allow(dead_code)]")
        {
            offenders.push(
                file.strip_prefix(&src_root)
                    .unwrap_or(&file)
                    .display()
                    .to_string(),
            );
        }
    }

    assert!(
        offenders.is_empty(),
        "found unconditional #![allow(dead_code)] in: {offenders:?}"
    );
}
