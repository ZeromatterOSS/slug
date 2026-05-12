//! Diff two length-delimited BEP files (typically one produced by Bazel and
//! one by Slug). Reports an event-type histogram and a per-type delta.
//!
//! Usage: `cargo run -p slug_build_event_stream --example bep_diff -- <bazel.pb> <slug.pb>`

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;

use slug_build_event_stream::build_event_stream as bep;
use prost::Message;

fn read_events(path: &str) -> Vec<bep::BuildEvent> {
    let mut f = File::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"));
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).expect("read");
    let mut cursor = std::io::Cursor::new(&buf[..]);
    let mut out = Vec::new();
    while (cursor.position() as usize) < buf.len() {
        out.push(bep::BuildEvent::decode_length_delimited(&mut cursor).expect("decode"));
    }
    out
}

fn payload_name(e: &bep::BuildEvent) -> String {
    e.payload
        .as_ref()
        .map(|p| {
            format!("{p:?}")
                .split_once('(')
                .map_or("Unknown".into(), |(n, _)| n.to_owned())
        })
        .unwrap_or_else(|| "None".to_owned())
}

fn histogram(events: &[bep::BuildEvent]) -> BTreeMap<String, usize> {
    let mut hist = BTreeMap::new();
    for e in events {
        *hist.entry(payload_name(e)).or_insert(0) += 1;
    }
    hist
}

fn main() {
    let mut args = std::env::args().skip(1);
    let lhs_path = args.next().expect("usage: bep_diff <bazel.pb> <slug.pb>");
    let rhs_path = args.next().expect("usage: bep_diff <bazel.pb> <slug.pb>");

    let lhs = read_events(&lhs_path);
    let rhs = read_events(&rhs_path);
    let lhs_hist = histogram(&lhs);
    let rhs_hist = histogram(&rhs);

    let mut kinds: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    kinds.extend(lhs_hist.keys().map(String::as_str));
    kinds.extend(rhs_hist.keys().map(String::as_str));

    println!(
        "{:<30} {:>10} {:>10} {:>10}",
        "event", "lhs", "rhs", "delta"
    );
    println!("{}", "-".repeat(65));
    for k in &kinds {
        let l = lhs_hist.get(*k).copied().unwrap_or(0);
        let r = rhs_hist.get(*k).copied().unwrap_or(0);
        let delta = r as isize - l as isize;
        let marker = if l == 0 || r == 0 { " *" } else { "" };
        println!("{:<30} {:>10} {:>10} {:>+10}{marker}", k, l, r, delta);
    }
    println!();
    println!("lhs path: {lhs_path}  total: {}", lhs.len());
    println!("rhs path: {rhs_path}  total: {}", rhs.len());
    println!("* = event type present on only one side");
}
