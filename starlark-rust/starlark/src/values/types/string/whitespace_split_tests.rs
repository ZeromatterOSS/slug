use super::rsplitn_whitespace;
use super::splitn_whitespace;
use crate::assert::Assert;
use crate::values::list::ListRef;

#[test]
fn whitespace_split_parts_borrow_input() {
    let source = format!(" {} 世界  {} ", "é".repeat(512), "γ".repeat(512));
    let begin = source.as_ptr() as usize;
    let end = begin + source.len();
    for parts in [
        splitn_whitespace(&source, 2),
        rsplitn_whitespace(&source, 2),
    ] {
        assert_eq!(parts.len(), 2);
        for part in parts {
            assert!(!part.is_empty());
            let pointer = part.as_ptr() as usize;
            assert!(pointer >= begin && pointer + part.len() <= end);
        }
    }
}

#[test]
fn whitespace_split_limits_preserve_remainders_and_utf8() {
    let cases: &[(&str, usize, &[&str], &[&str])] = &[
        ("", 1, &[], &[]),
        (" \t\r\n\u{a0}\u{2003}", 0, &[], &[]),
        (" a b  c ", 0, &["a b  c "], &[" a b  c"]),
        (" a b  c ", 1, &["a b  c "], &[" a b  c"]),
        (" a b  c ", 2, &["a", "b  c "], &[" a b", "c"]),
        (" a b  c ", 3, &["a", "b", "c "], &[" a", "b", "c"]),
        (" a b  c ", usize::MAX, &["a", "b", "c"], &["a", "b", "c"]),
        (" a\0b  c ", 2, &["a\0b", "c "], &[" a\0b", "c"]),
        (
            " \tα\u{a0}β\u{2003}γ \r\n",
            2,
            &["α", "β\u{2003}γ \r\n"],
            &[" \tα\u{a0}β", "γ"],
        ),
        ("é世界", 1, &["é世界"], &["é世界"]),
    ];
    for &(source, limit, forward, reverse) in cases {
        for (parts, expected) in [
            (splitn_whitespace(source, limit), forward),
            (rsplitn_whitespace(source, limit), reverse),
        ] {
            let actual: Vec<&str> = parts.iter().map(|part| &**part).collect();
            assert_eq!(actual, expected, "source={source:?}, limit={limit}");
        }
    }
}

#[test]
fn whitespace_split_public_results_survive_gc_and_freeze() {
    let module = Assert::new().pass_module(
        r#"
s = " \tα\u00a0β\u2003γ \r\n"
assert_eq(s.split(None, 0), ["α\u00a0β\u2003γ \r\n"])
assert_eq(s.rsplit(None, 0), [" \tα\u00a0β\u2003γ"])
assert_eq(s.split(None, 2), ["α", "β", "γ \r\n"])
assert_eq(s.rsplit(None, 2), [" \tα", "β", "γ"])
assert_eq(s.split(), ["α", "β", "γ"])
assert_eq(s.rsplit(None, -1), ["α", "β", "γ"])
assert_eq(" a,b,c ".split(",", 1), [" a", "b,c "])
assert_eq(" a,b,c ".rsplit(",", 1), [" a,b", "c "])
RESULT = [s.split(None, 1), s.rsplit(None, 1)]
s = None
garbage_collect()
assert_eq(RESULT, [["α", "β\u2003γ \r\n"], [" \tα\u00a0β", "γ"]])
"#,
    );
    let held = module.get("RESULT").unwrap();
    let cloned = module.clone();
    drop(module);
    drop(cloned);
    let list = ListRef::from_value(held.value()).unwrap();
    assert_eq!(list.content().len(), 2);
    let expected = [["α", "β\u{2003}γ \r\n"], [" \tα\u{a0}β", "γ"]];
    for (part, expected) in list.iter().zip(expected) {
        let strings: Vec<_> = ListRef::from_value(part)
            .unwrap()
            .iter()
            .map(|value| value.unpack_str().unwrap())
            .collect();
        assert_eq!(strings, expected);
    }
}
