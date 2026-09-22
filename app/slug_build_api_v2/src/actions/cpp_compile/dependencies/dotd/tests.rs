//! Pinned DependencySetTest themes plus byte-state-machine discriminators.
use super::parse;

fn check(bytes: &[u8], expected: &[&str]) {
    let original = bytes.to_vec();
    assert_eq!(parse(bytes).unwrap(), expected);
    assert_eq!(parse(bytes).unwrap(), expected);
    assert_eq!(
        bytes, original,
        "parsing must not mutate caller-owned bytes"
    );
}

#[test]
fn dotd_source_regressions_cover_continuations_and_line_endings() {
    for bytes in [
        b"hello.o: \\\n hello.cc \\\n hello.h \n".as_slice(),
        b"hello.o: \\\r\n hello.cc \\\r\n hello.h \n",
        b"hello.o: \\\r hello.cc \\\r hello.h \n",
        b"\r\nhello.o: \\\r\n hello.cc \\\r\n hello.h \n",
        b"hello.o : hello.cc \\\n hello.h\n",
    ] {
        check(bytes, &["hello.cc", "hello.h"]);
    }
    check(
        b"out: left\\\nright left\\\r\nright left\\\rright\n",
        &["leftright", "leftright", "leftright"],
    );
    check(b"out: a\rb\n", &["ab"]);
    check(b"out: unflushed\\\n", &[]);
    check(b"out: flushed unflushed\\\r\n", &["flushed"]);
}

#[test]
fn dotd_stanzas_preserve_order_duplicates_and_raw_path_coordinates() {
    check(
        b"one.o: src.cc ./a/../header.h /absolute/header.h\ntwo.o: src.cc other.h\n",
        &[
            "src.cc",
            "./a/../header.h",
            "/absolute/header.h",
            "src.cc",
            "other.h",
        ],
    );
    check(b"wrong-output-name: source.cc\n", &["source.cc"]);
    check(b"out: a a b a\n", &["a", "a", "b", "a"]);
}

#[test]
fn dotd_delimiter_lookahead_and_targetless_lines_match_native_quirks() {
    for bytes in [
        b"".as_slice(),
        b"\n\r\n",
        b"targetless a b\n",
        b": orphan\n",
        b"out:\tignored\n",
        b"out: \n a \\\n b\n",
    ] {
        check(bytes, &[]);
    }
    check(
        b"out: before: after keep:with:colon\n",
        &["after", "keep:with:colon"],
    );
    check(b"out: : retained\n", &["retained"]);
    check(b"out: discarded:\r retained\n", &["retained"]);
    check(b"out: discarded:\nnext: retained\n", &["retained"]);
    check(b"out: colon:\ttab\n", &["colon:\ttab"]);
}

#[test]
fn dotd_spaces_dollars_tabs_and_backslashes_are_not_generic_make_unescaping() {
    check(
        b"out: a\\ b c\\\\d e\\\tf\tg h\\q\n",
        &["a b", "c\\\\d", "e\\\tf\tg", "h\\q"],
    );
    check(
        b"out: $$cash hel$$$$lo $literal $$$tail\n",
        &["$cash", "hel$$lo", "$literal", "$$tail"],
    );
    check(
        b"out: tab\tseparated \\$$name\n",
        &["tab\tseparated", "\\$name"],
    );
}

#[test]
fn dotd_requires_final_lf_and_rejects_non_utf8_or_nul_without_partial_success() {
    for bytes in [
        b"out: header.h".as_slice(),
        b"out: header.h\r",
        b"targetless",
    ] {
        assert_eq!(parse(bytes).unwrap_err(), "File does not end in a newline");
    }
    for bytes in [b"out: good.h \xff\n".as_slice(), b"ignored\xff\n"] {
        assert!(parse(bytes).unwrap_err().contains("UTF-8"));
    }
    for bytes in [b"out: good.h bad\0name\n".as_slice(), b"ignored\0\n"] {
        assert!(parse(bytes).unwrap_err().contains("NUL"));
    }
    check(
        "目标.o: 源.cc 路\\ 径.h 😀.h\n".as_bytes(),
        &["源.cc", "路 径.h", "😀.h"],
    );
}
