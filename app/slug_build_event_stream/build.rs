use std::env;
use std::io;

fn main() -> io::Result<()> {
    let proto_files = &[
        "proto/src/main/java/com/google/devtools/build/lib/buildeventstream/proto/build_event_stream.proto",
        "proto/google/devtools/build/v1/publish_build_event.proto",
    ];

    let includes = if let Ok(path) = env::var("BUCK_PROTO_SRCS") {
        vec![path]
    } else {
        vec!["proto".to_owned()]
    };

    let builder = slug_protoc_dev::configure();
    unsafe { builder.setup_protoc() }.compile(proto_files, &includes)
}
