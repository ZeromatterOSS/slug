//! Round-trip tests for the BEP file sink.
//!
//! Binary-written events must decode back to equal `BuildEvent` values when
//! read with `prost::Message::decode_length_delimited`, matching Bazel's
//! `--build_event_binary_file` wire format.

use std::fs::File;
use std::io::Read;

use prost::Message;
use slug_build_event_stream::build_event_stream as bep;
use slug_build_event_stream::file_sink::Encoding;
use slug_build_event_stream::file_sink::FileSink;
use tempfile::TempDir;

fn make_event(code: i32) -> bep::BuildEvent {
    use bep::build_event_id as beid;
    bep::BuildEvent {
        id: Some(bep::BuildEventId {
            id: Some(beid::Id::BuildFinished(beid::BuildFinishedId {})),
        }),
        children: Vec::new(),
        last_message: false,
        payload: Some(bep::build_event::Payload::Finished(bep::BuildFinished {
            exit_code: Some(bep::build_finished::ExitCode {
                name: "TEST".into(),
                code,
            }),
            ..Default::default()
        })),
    }
}

fn read_all_length_delimited(path: &std::path::Path) -> Vec<bep::BuildEvent> {
    let mut file = File::open(path).expect("open");
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).expect("read");

    let mut cursor = std::io::Cursor::new(&bytes[..]);
    let mut out = Vec::new();
    while (cursor.position() as usize) < bytes.len() {
        let ev = bep::BuildEvent::decode_length_delimited(&mut cursor).expect("decode");
        out.push(ev);
    }
    out
}

#[test]
fn binary_sink_roundtrip_preserves_events() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("events.pb");
    let sink = FileSink::create(&path, Encoding::Binary).expect("create");

    let events = vec![make_event(0), make_event(1), make_event(42)];
    for e in &events {
        sink.write(e).expect("write");
    }
    sink.flush().expect("flush");

    let read = read_all_length_delimited(&path);
    assert_eq!(events, read);
}

#[test]
fn text_sink_writes_form_feed_delimited_events() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("events.txt");
    let sink = FileSink::create(&path, Encoding::Text).expect("create");

    sink.write(&make_event(0)).expect("write");
    sink.write(&make_event(1)).expect("write");
    sink.flush().expect("flush");

    let contents = std::fs::read_to_string(&path).expect("read");
    assert_eq!(contents.matches('\x0c').count(), 2);
    assert!(contents.contains("BuildFinished"));
}
