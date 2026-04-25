//! Verify that `make_aborted` produces a BuildFinished-scoped Aborted event
//! suitable for closing a BEP stream on the cancel / timeout / interrupt path.

use kuro_build_event_stream::build_event_stream as bep;
use kuro_build_event_stream::file_sink::Encoding;
use kuro_build_event_stream::file_sink::FileSink;
use kuro_build_event_stream::translate::make_aborted;
use prost::Message;
use tempfile::TempDir;

#[test]
fn aborted_event_closes_a_file_sink_stream() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("events.pb");
    let sink = FileSink::create(&path, Encoding::Binary).expect("create");

    let aborted = make_aborted(
        bep::BuildEventId {
            id: Some(bep::build_event_id::Id::BuildFinished(
                bep::build_event_id::BuildFinishedId {},
            )),
        },
        bep::aborted::AbortReason::UserInterrupted,
        "Ctrl+C",
    );
    sink.write(&aborted).expect("write");
    sink.flush().expect("flush");

    let bytes = std::fs::read(&path).expect("read");
    let mut cursor = std::io::Cursor::new(&bytes[..]);
    let decoded = bep::BuildEvent::decode_length_delimited(&mut cursor).expect("decode");

    assert!(
        decoded.last_message,
        "Aborted must be the final stream message"
    );
    match decoded.payload.as_ref().expect("payload") {
        bep::build_event::Payload::Aborted(a) => {
            assert_eq!(a.reason, bep::aborted::AbortReason::UserInterrupted as i32);
            assert_eq!(a.description, "Ctrl+C");
        }
        other => panic!("expected Aborted, got {other:?}"),
    }
}
