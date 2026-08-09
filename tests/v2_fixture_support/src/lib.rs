use std::fs;
use std::io::Write;
use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

const PAYLOAD: &[u8] = include_bytes!("../../v2_fixture_payload/fixtures.payload");
const PAYLOAD_SHA256: &str = "ec920183c2777faf183f6143cca131650c770067c9583952333b947ac7b21df0";
#[rustfmt::skip]
const PROJECTIONS: &[(&str, &str)] = &[
    ("simple-rule-action", "3b8a1425ef7ea5b92de2f363465e5d52d92ce25c2b1818450bffc9098277f5fb"),
    ("recursive-custom-rule-providers-actions", "56584525959da70efe9fa64ef5acd862cb70fdf19ed5466d4c2d8f7a8d900c0f"),
    ("build-file-loading", "a54763ef1ff899547f4620bc2c3ec912d9c1cdca1d30714a2e43fcfc851f9cbf"),
    ("query-parser-and-sets", "0be99e30892443f9262e9618dd38c4a89522e107e0176f122a7a8cf4162542d6"),
    ("tests-query-expansion", "8b9ee022d4736bc58d3adc1adb67b6a1e6de5569950a475b6cc6c03cb70ffdee"),
    ("query-visible-visibility", "5bed82ad5b929c8d5f64dcfb2bb800ffdfa3fa13126ba22d02438bd5fe12cb9a"),
    ("query-build-load-files-provenance", "85a2e8fdedbe19e46f4b11a9e6e008d44b290d56c672775e018938eacaec9f7a"),
    ("query-siblings-build-file-node", "c2c102d891f2095f07878eae45fa0d3ad75bff269b32a213b7f4b2826d63b2b9"),
    ("query-loading-thin-vertical", "74e8d13fceff7c8868431a3e57653f70d7dea73bc3d203c7000090d66fceb330"),
    ("query-labels-attribute-metadata", "6ee33fce813b0ea9f286fea78dfde2a8e98389afdb01647c1e6d4892fed6ff5d"),
    ("query-executables-rule-capability", "7d320eb69086edf9ca85ca512d65b7259baabc7ac35fa7077c011536a57af227"),
    ("query-rdeps-and-subtree-patterns", "c4f5d3970fd6a3c8e04ebe277e12072311ef87dfccd372303d86dc1515260110"),
    ("query-path-topology", "50e86ad2c6528567aa9b106cd487e024f562b34020b84174a96e1012d24b52be"),
    ("query-some-selection", "9c0422b184f725508bd598d6b554f635a0f6ceeb507ac79c2bc59d2a3b1bc121"),
];

#[derive(Debug)]
struct Entry<'a> {
    path: &'a str,
    directory: bool,
    body: &'a [u8],
}

pub struct FixtureWorkspace {
    root: PathBuf,
}

impl FixtureWorkspace {
    pub fn new(workspace: &str) -> Result<Self, String> {
        valid_path(workspace)?;
        let entries = parse(PAYLOAD)?;
        if hex_sha256(PAYLOAD) != PAYLOAD_SHA256 {
            return Err("fixture payload digest mismatch".to_owned());
        }
        let entries: Vec<_> = entries
            .into_iter()
            .filter(|entry| {
                entry.path == workspace || entry.path.starts_with(&format!("{workspace}/"))
            })
            .collect();
        let expected = PROJECTIONS
            .iter()
            .find_map(|(name, digest)| (*name == workspace).then_some(*digest))
            .ok_or_else(|| format!("unknown fixture workspace {workspace}"))?;
        if entries.first().map(|entry| (entry.path, entry.directory)) != Some((workspace, true)) {
            return Err(format!("missing fixture workspace {workspace}"));
        }
        if hex_sha256(&encode(&entries)) != expected {
            return Err(format!(
                "fixture projection digest mismatch for {workspace}"
            ));
        }
        let root = fresh_root(workspace)?;
        if let Err(error) = materialize(&root, workspace, &entries) {
            let _ = fs::remove_dir_all(&root);
            return Err(error);
        }
        Ok(Self { root })
    }

    pub fn path(&self) -> &Path {
        &self.root
    }
}

impl AsRef<Path> for FixtureWorkspace {
    fn as_ref(&self) -> &Path {
        self.path()
    }
}

impl Deref for FixtureWorkspace {
    type Target = Path;
    fn deref(&self) -> &Self::Target {
        self.path()
    }
}

impl Drop for FixtureWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn fresh_root(workspace: &str) -> Result<PathBuf, String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let parent = std::env::var_os("TEST_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    for _ in 0..100 {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let root = parent.join(format!("slug-v2-fixture-{workspace}-{nanos}-{id}"));
        match fs::create_dir(&root) {
            Ok(()) => {
                set_mode(&root, 0o755)?;
                return Ok(root);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("create fixture root {}: {error}", root.display())),
        }
    }
    Err("could not allocate a fresh fixture root".to_owned())
}

fn materialize(root: &Path, workspace: &str, entries: &[Entry<'_>]) -> Result<(), String> {
    for entry in &entries[1..] {
        let relative = entry
            .path
            .strip_prefix(workspace)
            .unwrap()
            .strip_prefix('/')
            .unwrap();
        let destination = root.join(relative);
        if destination.exists() || destination.is_symlink() {
            return Err(format!(
                "pre-existing extraction component: {}",
                destination.display()
            ));
        }
        if entry.directory {
            fs::create_dir(&destination)
                .map_err(|error| format!("create {}: {error}", destination.display()))?;
            set_mode(&destination, 0o755)?;
        } else {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&destination)
                .map_err(|error| format!("create {}: {error}", destination.display()))?;
            file.write_all(entry.body)
                .map_err(|error| format!("write {}: {error}", destination.display()))?;
            drop(file);
            set_mode(&destination, 0o644)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|error| format!("set mode on {}: {error}", path.display()))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<(), String> {
    Ok(())
}

fn parse(data: &[u8]) -> Result<Vec<Entry<'_>>, String> {
    const MAGIC: &[u8] = b"slug-fixture-payload-v1\n";
    if !data.starts_with(MAGIC) {
        return Err("invalid payload header".to_owned());
    }
    let mut offset = MAGIC.len();
    let mut entries = Vec::new();
    let mut prior = "";
    let mut folded = std::collections::BTreeSet::new();
    let mut directory_paths = std::collections::BTreeSet::new();
    let (mut directories, mut files, mut bytes) = (0usize, 0usize, 0usize);
    loop {
        let (line, next) = line_at(data, offset)?;
        offset = next;
        let fields: Vec<_> = line.split(|byte| *byte == b'\t').collect();
        if fields.first() == Some(&b"E".as_slice()) {
            if fields.len() != 4
                || fields[1..]
                    .iter()
                    .any(|item| !item.iter().all(u8::is_ascii_digit))
            {
                return Err("invalid payload footer".to_owned());
            }
            let values: Result<Vec<usize>, _> = fields[1..]
                .iter()
                .map(|value| std::str::from_utf8(value).unwrap().parse())
                .collect();
            if values.map_err(|_| "invalid payload footer".to_owned())?
                != [directories, files, bytes]
                || offset != data.len()
            {
                return Err("payload footer mismatch".to_owned());
            }
            return Ok(entries);
        }
        let (path_bytes, directory, body) = match fields.first() {
            Some(kind) if *kind == b"D" => {
                if fields.len() != 3 || fields[1] != b"0755" {
                    return Err("invalid directory record".to_owned());
                }
                (fields[2], true, &[][..])
            }
            Some(kind) if *kind == b"F" => {
                if fields.len() != 5
                    || fields[1] != b"0644"
                    || !fields[2].iter().all(u8::is_ascii_digit)
                {
                    return Err("invalid file record".to_owned());
                }
                let length: usize = std::str::from_utf8(fields[2])
                    .map_err(|_| "invalid file length")?
                    .parse()
                    .map_err(|_| "invalid file length")?;
                if fields[3].len() != 64
                    || !fields[3]
                        .iter()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
                    || length
                        .checked_add(1)
                        .and_then(|body| offset.checked_add(body))
                        .is_none_or(|end| end > data.len())
                {
                    return Err("invalid file body".to_owned());
                }
                let body = &data[offset..offset + length];
                offset += length;
                if data[offset] != b'\n' || hex_sha256(body).as_bytes() != fields[3] {
                    return Err("file body mismatch".to_owned());
                }
                offset += 1;
                files += 1;
                bytes += length;
                (fields[4], false, body)
            }
            _ => return Err("unknown payload record".to_owned()),
        };
        let path = std::str::from_utf8(path_bytes).map_err(|_| "non-ASCII payload path")?;
        valid_path(path)?;
        if !prior.is_empty() && path <= prior || !folded.insert(path.to_ascii_lowercase()) {
            return Err("noncanonical payload paths".to_owned());
        }
        if let Some((parent, _)) = path.rsplit_once('/') {
            if !directory_paths.contains(parent) {
                return Err("payload parent is not a prior directory".to_owned());
            }
        }
        prior = path;
        if directory {
            directories += 1;
            directory_paths.insert(path);
        }
        entries.push(Entry {
            path,
            directory,
            body,
        });
    }
}

fn line_at(data: &[u8], offset: usize) -> Result<(&[u8], usize), String> {
    let end = data
        .get(offset..)
        .ok_or_else(|| "truncated payload record".to_owned())?
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|index| offset + index)
        .ok_or_else(|| "truncated payload record".to_owned())?;
    Ok((&data[offset..end], end + 1))
}

fn encode(entries: &[Entry<'_>]) -> Vec<u8> {
    let mut result = b"slug-fixture-payload-v1\n".to_vec();
    let (mut directories, mut files, mut bytes) = (0usize, 0usize, 0usize);
    for entry in entries {
        if entry.directory {
            result.extend_from_slice(format!("D\t0755\t{}\n", entry.path).as_bytes());
            directories += 1;
        } else {
            result.extend_from_slice(
                format!(
                    "F\t0644\t{}\t{}\t{}\n",
                    entry.body.len(),
                    hex_sha256(entry.body),
                    entry.path
                )
                .as_bytes(),
            );
            result.extend_from_slice(entry.body);
            result.push(b'\n');
            files += 1;
            bytes += entry.body.len();
        }
    }
    result.extend_from_slice(format!("E\t{directories}\t{files}\t{bytes}\n").as_bytes());
    result
}

fn valid_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.split('/').any(|component| {
            component.is_empty()
                || component == "."
                || component == ".."
                || component.ends_with('.')
                || !component
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
                || is_device_name(component)
        })
    {
        return Err("invalid payload path".to_owned());
    }
    Ok(())
}

fn is_device_name(component: &str) -> bool {
    let stem = component.split('.').next().unwrap().to_ascii_lowercase();
    matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || ["com", "lpt"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
        })
}

#[rustfmt::skip]
fn hex_sha256(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
        0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
        0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
        0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
        0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
        0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
        0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
        0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2,
    ];
    let mut hash = [0x6a09e667u32,0xbb67ae85,0x3c6ef372,0xa54ff53a,0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19];
    let bit_length = (bytes.len() as u64).wrapping_mul(8);
    let mut padded = bytes.to_vec();
    padded.push(0x80);
    while (padded.len() + 8) % 64 != 0 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_length.to_be_bytes());
    for chunk in padded.chunks_exact(64) {
        let mut words = [0u32; 64];
        for (i, word) in words[..16].iter_mut().enumerate() { *word = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap()); }
        for i in 16..64 {
            let s0 = words[i-15].rotate_right(7) ^ words[i-15].rotate_right(18) ^ (words[i-15] >> 3);
            let s1 = words[i-2].rotate_right(17) ^ words[i-2].rotate_right(19) ^ (words[i-2] >> 10);
            words[i] = words[i-16].wrapping_add(s0).wrapping_add(words[i-7]).wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h) = (
            hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7],
        );
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ (!e & g);
            let temporary1 = h.wrapping_add(s1).wrapping_add(choice).wrapping_add(K[i]).wrapping_add(words[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temporary2 = s0.wrapping_add(majority);
            (h,g,f,e,d,c,b,a) = (g,f,e,d.wrapping_add(temporary1),c,b,a,temporary1.wrapping_add(temporary2));
        }
        for (slot, value) in hash.iter_mut().zip([a,b,c,d,e,f,g,h]) { *slot = slot.wrapping_add(value); }
    }
    hash.iter().map(|word| format!("{word:08x}")).collect()
}

#[cfg(test)]
#[rustfmt::skip]
mod tests {
    use super::*;

    fn doc(records: &[Vec<u8>], footer: &[u8]) -> Vec<u8> {
        [b"slug-fixture-payload-v1\n".as_slice(), &records.concat(), footer].concat()
    }
    fn dir(path: &[u8], mode: &str) -> Vec<u8> {
        [format!("D\t{mode}\t").as_bytes(), path, b"\n"].concat()
    }
    fn file(path: &[u8], body: &[u8], length: Option<&str>, digest: Option<&str>, mode: &str, terminator: bool) -> Vec<u8> {
        let length = length.map(str::to_owned).unwrap_or_else(|| body.len().to_string());
        let digest = digest.map(str::to_owned).unwrap_or_else(|| hex_sha256(body));
        [format!("F\t{mode}\t{length}\t{digest}\t").as_bytes(), path, b"\n", body, if terminator { b"\n" } else { b"" }].concat()
    }
    fn path_payload(path: &[u8]) -> Vec<u8> {
        let mut records = if path.starts_with(b"root/") { vec![dir(b"root", "0755")] } else { vec![] };
        records.push(dir(path, "0755"));
        doc(&records, format!("E\t{}\t0\t0\n", records.len()).as_bytes())
    }
    fn canonical(path: Option<(&str, &[u8])>) -> Vec<u8> {
        let mut entries = vec![Entry { path: "root", directory: true, body: &[] }];
        if let Some((path, body)) = path { entries.push(Entry { path, directory: false, body }); }
        encode(&entries)
    }

    #[test]
    fn canonical_payload_conformance() {
        assert_eq!(parse(PAYLOAD).unwrap().len(), 285);
        assert_eq!(hex_sha256(PAYLOAD), PAYLOAD_SHA256);
        for (workspace, digest) in PROJECTIONS {
            let prefix = format!("{workspace}/");
            let entries: Vec<_> = parse(PAYLOAD).unwrap().into_iter()
                .filter(|entry| entry.path == *workspace || entry.path.starts_with(&prefix)).collect();
            assert_eq!(hex_sha256(&encode(&entries)), *digest, "{workspace}");
        }
        let canonical = [
            ("canonical_empty_directory", canonical(None)),
            ("canonical_empty_file", canonical(Some(("root/empty", b"")))),
            ("canonical_no_final_newline", canonical(Some(("root/text", b"line")))),
            ("canonical_binary_body", canonical(Some(("root/data", b"\x00\x09\x0a\xff")))),
        ];
        for (name, value) in canonical { assert!(parse(&value).is_ok(), "{name}"); }
        let _fixture = FixtureWorkspace::new("simple-rule-action").unwrap();
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(_fixture.path()), 0o755);
            assert_eq!(mode(&_fixture.path().join("MODULE.bazel")), 0o644);
        }
    }

    #[test]
    fn malformed_payload_conformance() {
        let hash = hex_sha256(&[]);
        let mut malformed = vec![
            ("invalid_header", b"not-the-header\nE\t0\t0\t0\n".to_vec()),
            ("invalid_footer_shape", doc(&[], b"E\t0\t0\n")),
            ("invalid_footer_count", doc(&[dir(b"root", "0755")], b"E\t0\t0\t0\n")),
            ("unknown_type", doc(&[b"X\t0755\troot\n".to_vec()], b"E\t0\t0\t0\n")),
            ("directory_mode", doc(&[dir(b"root", "0700")], b"E\t1\t0\t0\n")),
            ("file_mode", doc(&[file(b"root/a", b"", None, None, "0600", true)], b"E\t0\t1\t0\n")),
            ("file_length", doc(&[file(b"root/a", b"", Some("1"), None, "0644", true)], b"E\t0\t1\t0\n")),
            ("oversized_length", doc(&[file(b"root/a", b"", Some("18446744073709551615"), None, "0644", true)], b"E\t0\t1\t0\n")),
            ("file_hash", doc(&[file(b"root/a", b"", None, Some(&"0".repeat(64)), "0644", true)], b"E\t0\t1\t0\n")),
            ("uppercase_hash", doc(&[file(b"root/a", b"", None, Some(&hash.to_uppercase()), "0644", true)], b"E\t0\t1\t0\n")),
            ("missing_body_terminator", doc(&[file(b"root/a", b"x", None, None, "0644", false)], b"E\t0\t1\t1\n")),
            ("trailing_data", doc(&[], b"E\t0\t0\t0\nextra")),
            ("out_of_order", doc(&[dir(b"b", "0755"), dir(b"a", "0755")], b"E\t2\t0\t0\n")),
            ("duplicate_path", doc(&[dir(b"root", "0755"), dir(b"root", "0755")], b"E\t2\t0\t0\n")),
            ("case_collision", doc(&[dir(b"A", "0755"), dir(b"a", "0755")], b"E\t2\t0\t0\n")),
            ("missing_parent", doc(&[dir(b"root", "0755"), file(b"root/a/b", b"", None, None, "0644", true)], b"E\t1\t1\t0\n")),
            ("file_parent", doc(&[dir(b"root", "0755"), file(b"root/a", b"", None, None, "0644", true), dir(b"root/a/b", "0755")], b"E\t2\t1\t0\n")),
        ];
        for (name, path) in [
            ("absolute_path", b"/root".as_slice()), ("dot_component", b"root/."),
            ("dot_dot_component", b"root/.."), ("trailing_dot", b"root/name."),
            ("device_name", b"root/con.txt"), ("backslash_path", b"root\\name"),
            ("tab_path", b"root\tname"), ("newline_path", b"root\nname"),
            ("carriage_return_path", b"root\rname"), ("nul_path", b"root\0name"),
            ("space_path", b"root name"), ("colon_path", b"root:name"),
            ("non_ascii_path", b"root/\xff"),
        ] { malformed.push((name, path_payload(path))); }
        for (name, value) in malformed { assert!(parse(&value).is_err(), "{name}"); }
    }
}
