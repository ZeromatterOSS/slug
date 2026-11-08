//! One bounded JSON line, including its newline. stderr is a byte-array wire
//! field; count its serialized expansion before any response-sized allocation.
use std::io::BufRead;
use std::io::Read;
use std::io::Write;
use std::io::{self};

use serde::Serialize;

pub(super) const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;

fn too_large() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "daemon frame exceeds serialized byte limit",
    )
}
struct Counter {
    len: usize,
    limit: usize,
}
impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.len = self
            .len
            .checked_add(bytes.len())
            .filter(|len| *len <= self.limit)
            .ok_or_else(too_large)?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
struct Buffer {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|len| len > self.limit)
        {
            return Err(too_large());
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(super) fn encode(value: &impl Serialize, limit: usize) -> anyhow::Result<Vec<u8>> {
    let json_limit = limit.checked_sub(1).ok_or_else(too_large)?;
    let mut counter = Counter {
        len: 0,
        limit: json_limit,
    };
    serde_json::to_writer(&mut counter, value)?;
    let total = counter.len.checked_add(1).ok_or_else(too_large)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(total)?;
    let mut output = Buffer {
        bytes,
        limit: counter.len,
    };
    serde_json::to_writer(&mut output, value)?;
    output.bytes.push(b'\n');
    Ok(output.bytes)
}
pub(super) fn response(value: &super::DaemonResponse, limit: usize) -> anyhow::Result<Vec<u8>> {
    match encode(value, limit) {
        Ok(frame) => Ok(frame),
        Err(_) => encode(
            &super::DaemonResponse {
                exit_code: 2,
                stdout: String::new(),
                stderr: b"{\"error\":\"daemon_response_frame_limit\"}\n".to_vec(),
                invalidated_files: 0,
                run_launch_plan: None,
            },
            limit,
        ),
    }
}
pub(super) fn read_line(input: &mut impl Read, limit: usize) -> anyhow::Result<String> {
    let mut input = io::BufReader::new(input);
    let mut bytes = Vec::new();
    loop {
        let chunk = input.fill_buf()?;
        if chunk.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "daemon frame requires newline",
            )
            .into());
        }
        let newline = chunk.iter().position(|byte| *byte == b'\n');
        let count = newline.map_or(chunk.len(), |index| index + 1);
        let total = bytes
            .len()
            .checked_add(count)
            .filter(|len| *len <= limit)
            .ok_or_else(too_large)?;
        if total > bytes.capacity() {
            let capacity = bytes.capacity().saturating_mul(2).max(total).min(limit);
            bytes.try_reserve_exact(capacity - bytes.len())?;
        }
        bytes.extend_from_slice(&chunk[..count]);
        input.consume(count);
        if newline.is_some() {
            bytes.pop();
            if bytes.last() == Some(&b'\r') {
                bytes.pop();
            }
            return Ok(String::from_utf8(bytes)?);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::DaemonResponse;
    use super::*;

    fn output(bytes: Vec<u8>) -> DaemonResponse {
        DaemonResponse {
            exit_code: 0,
            stdout: String::new(),
            stderr: bytes,
            invalidated_files: 0,
            run_launch_plan: None,
        }
    }
    #[test]
    fn serialized_frame_exact_and_plus_one_include_newline_and_binary_expansion() {
        let value = output(vec![255, 0, 128, b'\n']);
        let expected = serde_json::to_vec(&value).unwrap();
        assert!(expected.len() > value.stderr.len());
        let encoded = encode(&value, expected.len() + 1).unwrap();
        assert_eq!(&encoded[..expected.len()], expected);
        assert_eq!(encoded.last(), Some(&b'\n'));
        assert!(encode(&value, expected.len()).is_err());
        let parsed: DaemonResponse = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(parsed.stderr, value.stderr);
        assert_eq!(
            read_line(&mut encoded.as_slice(), encoded.len())
                .unwrap()
                .as_bytes(),
            expected
        );
        assert!(read_line(&mut encoded.as_slice(), encoded.len() - 1).is_err());
    }
    #[test]
    fn incomplete_oversized_and_invalid_frames_refuse_without_parsing_suffix() {
        assert!(read_line(&mut b"{}".as_slice(), 8).is_err());
        assert!(read_line(&mut b"123456789\n{}\n".as_slice(), 8).is_err());
        assert!(read_line(&mut b"\xff\n".as_slice(), 8).is_err());
        assert_eq!(read_line(&mut b"{}\r\n".as_slice(), 4).unwrap(), "{}");
    }
    #[test]
    fn oversized_response_is_a_bounded_error_and_never_partial_success() {
        let value = output(vec![255; 1024]);
        let frame = response(&value, 512).unwrap();
        assert!(frame.len() <= 512);
        let error: DaemonResponse = serde_json::from_slice(&frame).unwrap();
        assert_eq!(error.exit_code, 2);
        assert!(error.stdout.is_empty());
        assert_eq!(
            error.stderr,
            b"{\"error\":\"daemon_response_frame_limit\"}\n"
        );
        assert!(response(&value, 1).is_err());
    }
}
