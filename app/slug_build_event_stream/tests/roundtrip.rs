//! Round-trip a sample BEP `BuildEvent` through the generated Rust types.

use slug_build_event_stream::build_event_stream as bes;
use prost::Message;

fn sample_build_event() -> bes::BuildEvent {
    use bes::build_event_id as beid;

    bes::BuildEvent {
        id: Some(bes::BuildEventId {
            id: Some(beid::Id::Started(beid::BuildStartedId {})),
        }),
        children: vec![bes::BuildEventId {
            id: Some(beid::Id::BuildFinished(beid::BuildFinishedId {})),
        }],
        last_message: false,
        payload: Some(bes::build_event::Payload::Started(bes::BuildStarted {
            uuid: "00000000-0000-4000-8000-000000000000".to_owned(),
            start_time: Some(prost_types::Timestamp {
                seconds: 1_700_000_000,
                nanos: 0,
            }),
            build_tool_version: "slug-test".to_owned(),
            options_description: "".to_owned(),
            command: "build".to_owned(),
            working_directory: "/tmp/ws".to_owned(),
            workspace_directory: "/tmp/ws".to_owned(),
            user: "tester".to_owned(),
            host: "localhost".to_owned(),
            server_pid: 0,
            java_version_info: None,
            ..Default::default()
        })),
    }
}

#[test]
fn buildevent_proto_roundtrip() {
    let original = sample_build_event();

    let mut buf = Vec::new();
    original.encode(&mut buf).expect("encode");
    let decoded = bes::BuildEvent::decode(buf.as_slice()).expect("decode");

    assert_eq!(original, decoded);
}

#[test]
fn publish_build_event_types_available() {
    use slug_build_event_stream::google::devtools::build::v1 as bes_v1;

    let _ = bes_v1::PublishLifecycleEventRequest::default();
    let _ = bes_v1::PublishBuildToolEventStreamRequest::default();
    let _ = bes_v1::BuildEvent::default();
    let _ = bes_v1::OrderedBuildEvent::default();
}
