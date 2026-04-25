//! File sinks for BEP events: length-delimited binary + proto text format.
//!
//! Wired behind the `--build_event_binary_file` and `--build_event_text_file`
//! CLI flags, mirroring Bazel's file-output options.
//!
//! ## Formats
//!
//! - Binary (`write_binary`): length-delimited encoding of
//!   `build_event_stream.BuildEvent`, identical to what Bazel writes for
//!   `--build_event_binary_file`. Readable by Bazel's own BEP tooling and by
//!   BuildBuddy's offline parsers.
//! - Text (`write_text`): human-readable `Debug` rendering of each event,
//!   separated by a form-feed. This is NOT identical to Bazel's proto
//!   `TextFormat` output — BEP consumers that require exact parity with
//!   Bazel's text file should prefer the binary sink. The text sink is meant
//!   for developer debugging.
//!
//! ## JSON (deferred)
//!
//! `--build_event_json_file` needs proto3-canonical JSON (camelCase fields,
//! ISO-8601 timestamps, string-name enums, base64 bytes). Prost doesn't emit
//! proto3-JSON-compatible serde impls for Well-Known Types; adding this
//! correctly requires `pbjson-build` integration. Deferred to Plan 18.8
//! conformance work, where byte-for-byte JSON parity with Bazel matters.

use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;

use prost::Message;

use crate::build_event_stream::BuildEvent;

/// Encode a `BuildEvent` with a varint length prefix and write it to `writer`.
///
/// Matches Bazel's `--build_event_binary_file` wire format: a stream of
/// `<varint length><BuildEvent proto bytes>` records with no outer framing.
pub fn write_binary<W: Write>(event: &BuildEvent, writer: &mut W) -> io::Result<()> {
    let mut buf = Vec::with_capacity(event.encoded_len() + 10);
    event
        .encode_length_delimited(&mut buf)
        .map_err(io::Error::other)?;
    writer.write_all(&buf)
}

/// Write a debug rendering of the event to `writer`, terminated by a
/// form-feed so the stream can be split apart later.
pub fn write_text<W: Write>(event: &BuildEvent, writer: &mut W) -> io::Result<()> {
    writeln!(writer, "{event:#?}\n\x0c")
}

/// File-backed BEP sink. Wrap one per `--build_event_*_file` flag.
///
/// Writes are buffered and guarded by a `Mutex` so concurrent senders can
/// feed events in. On `flush()` the buffer is flushed to the underlying fd;
/// `Drop` does a best-effort flush.
pub struct FileSink {
    path: PathBuf,
    encoding: Encoding,
    writer: Mutex<BufWriter<File>>,
}

#[derive(Debug, Clone, Copy)]
pub enum Encoding {
    /// Length-delimited proto; Bazel-compatible.
    Binary,
    /// `Debug`-formatted, form-feed delimited.
    Text,
}

impl FileSink {
    pub fn create(path: impl Into<PathBuf>, encoding: Encoding) -> io::Result<Self> {
        let path: PathBuf = path.into();
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)?;
        Ok(Self {
            path,
            encoding,
            writer: Mutex::new(BufWriter::new(file)),
        })
    }

    pub fn write(&self, event: &BuildEvent) -> io::Result<()> {
        let mut guard = self.writer.lock().expect("FileSink writer poisoned");
        match self.encoding {
            Encoding::Binary => write_binary(event, &mut *guard),
            Encoding::Text => write_text(event, &mut *guard),
        }
    }

    pub fn flush(&self) -> io::Result<()> {
        let mut guard = self.writer.lock().expect("FileSink writer poisoned");
        guard.flush()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for FileSink {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.writer.lock() {
            let _ = guard.flush();
        }
    }
}
