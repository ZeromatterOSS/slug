/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::path::PathBuf;

pub fn unpinned_local_file_url_paths<'a>(
    urls: impl IntoIterator<Item = &'a str>,
    sha256: Option<&str>,
    integrity: Option<&str>,
) -> Vec<PathBuf> {
    if sha256.is_some_and(|value| !value.is_empty())
        || integrity.is_some_and(|value| !value.is_empty())
    {
        return Vec::new();
    }
    urls.into_iter().filter_map(local_file_url_path).collect()
}

pub fn local_file_url_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    let raw_path = if let Some(localhost_path) = rest.strip_prefix("localhost/") {
        format!("/{localhost_path}")
    } else if rest.starts_with('/') {
        rest.to_owned()
    } else {
        return None;
    };
    Some(PathBuf::from(percent_decode(&raw_path)?))
}

fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hi = *bytes.get(index + 1)?;
            let lo = *bytes.get(index + 2)?;
            decoded.push(hex_value(hi)? << 4 | hex_value(lo)?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_local_file_url_paths() {
        assert_eq!(
            local_file_url_path("file:///tmp/source%20file.txt"),
            Some(PathBuf::from("/tmp/source file.txt"))
        );
        assert_eq!(
            local_file_url_path("file://localhost/tmp/source+file.txt"),
            Some(PathBuf::from("/tmp/source+file.txt"))
        );
        assert_eq!(
            local_file_url_path("file://example.com/tmp/source.txt"),
            None
        );
        assert_eq!(local_file_url_path("https://example.com/source.txt"), None);
    }

    #[test]
    fn pinned_downloads_do_not_record_file_url_sources() {
        assert!(
            unpinned_local_file_url_paths(["file:///tmp/source.txt"], Some("abc"), None).is_empty()
        );
        assert!(
            unpinned_local_file_url_paths(["file:///tmp/source.txt"], None, Some("sha256-abc"))
                .is_empty()
        );
    }
}
