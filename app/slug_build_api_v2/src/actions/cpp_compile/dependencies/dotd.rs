//! Raw POSIX dependency tokens from Bazel9.2 DependencySet.process137–237.
//! Paths and target names are deliberately not normalized/authenticated here.
use compact_str::CompactString;

#[cfg(test)]
mod tests;

pub(super) fn parse(bytes: &[u8]) -> Result<Vec<CompactString>, String> {
    // Slug's explicit path domain: do not reinterpret Latin-1 or admit NUL even
    // in ignored target/targetless text. The remaining algorithm is byte-based.
    std::str::from_utf8(bytes).map_err(|_| "dotd requires valid UTF-8".to_owned())?;
    if bytes.contains(&0) {
        return Err("dotd contains unsupported NUL".to_owned());
    }
    if !bytes.is_empty() && bytes.last() != Some(&b'\n') {
        return Err("File does not end in a newline".to_owned());
    }
    // Native uses its input prefix as scratch. Copy once to preserve our caller's
    // immutable bytes; write never overtakes read, including delimiter lookahead.
    let mut buffer = bytes.to_vec();
    let mut result = Vec::new();
    let mut read = 0;
    let mut write = 0;
    let mut saw_target = false;
    while read < buffer.len() {
        let byte = buffer[read];
        read += 1;
        match byte {
            b' ' => {
                if saw_target && write > 0 {
                    emit(&mut result, &buffer[..write]);
                    write = 0;
                }
            }
            b'\r' => {}
            b'\n' => {
                if saw_target && write > 0 {
                    emit(&mut result, &buffer[..write]);
                }
                write = 0;
                saw_target = false;
            }
            b':' if matches!(buffer[read], b' ' | b'\n' | b'\r') => {
                // This also applies to a colon encountered after an earlier
                // target: the current token becomes a new ignored target name.
                if write > 0 {
                    write = 0;
                    saw_target = true;
                }
            }
            b'\\' => match buffer[read] {
                b' ' => {
                    buffer[write] = b' ';
                    write += 1;
                    read += 1;
                }
                b'\n' => read += 1,
                b'\r' => {
                    read += 1;
                    if buffer[read] == b'\n' {
                        read += 1;
                    }
                }
                _ => {
                    buffer[write] = byte;
                    write += 1;
                }
            },
            b'$' if buffer[read] == b'$' => {
                buffer[write] = b'$';
                write += 1;
                read += 1;
            }
            _ => {
                buffer[write] = byte;
                write += 1;
            }
        }
    }
    // In particular, a final escaped LF does not flush the buffered token.
    Ok(result)
}

fn emit(result: &mut Vec<CompactString>, token: &[u8]) {
    // Input was UTF-8 and transformations only remove/replace ASCII bytes.
    result.push(CompactString::new(
        std::str::from_utf8(token).expect("ASCII-only dotd decoding preserves UTF-8"),
    ));
}
