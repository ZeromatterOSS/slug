//! Decode a length-delimited BEP binary file and print each event's variant
//! name + `last_message` flag.
//!
//! Usage: `cargo run -p slug_build_event_stream --example decode_bep -- <path>`

use std::env;
use std::fs::File;
use std::io::Read;

use slug_build_event_stream::build_event_stream as bep;
use prost::Message;

fn main() {
    let path = env::args().nth(1).expect("usage: decode_bep <path>");
    let mut f = File::open(&path).expect("open");
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).expect("read");

    let mut cursor = std::io::Cursor::new(&buf[..]);
    let mut count = 0;
    while (cursor.position() as usize) < buf.len() {
        let e = bep::BuildEvent::decode_length_delimited(&mut cursor).expect("decode");
        let payload = e
            .payload
            .as_ref()
            .map(|p| {
                format!("{p:?}")
                    .split_once('(')
                    .map_or("?".into(), |(n, _)| n.to_string())
            })
            .unwrap_or_default();
        let mut extra = String::new();
        if let Some(id) = e.id.as_ref().and_then(|i| i.id.as_ref()) {
            if let bep::build_event_id::Id::Pattern(p) = id {
                extra = format!(" id.pattern={:?}", p.pattern);
            }
        }
        println!(
            "#{count:02} payload={payload}  last={}{extra}",
            e.last_message
        );
        count += 1;
    }
    println!("total events = {count}");
}
