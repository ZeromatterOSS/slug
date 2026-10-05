use super::parse_format_one;
use crate::assert::Assert;

#[test]
fn format_one_unescaped_segments_share_input() {
    let prefix = "é".repeat(512);
    let suffix = "世界".repeat(256);
    let source = format!("{prefix}{{!s}}{suffix}");
    let (before, after) = parse_format_one(&source).unwrap();
    assert_eq!(&*before, prefix);
    assert_eq!(&*after, suffix);
    assert_eq!(before.as_ptr(), source.as_ptr());
    assert_eq!(after.as_ptr(), source[prefix.len() + 4..].as_ptr());
}

#[test]
fn format_one_escapes_unicode_and_rejection_are_preserved() {
    let cases: &[(&str, Option<(&str, &str)>)] = &[
        ("{}", Some(("", ""))),
        ("{!s}", Some(("", ""))),
        ("é{}世界", Some(("é", "世界"))),
        ("\0{}é", Some(("\0", "é"))),
        ("a{}", Some(("a", ""))),
        ("{}b", Some(("", "b"))),
        ("{{{}}}", Some(("{", "}"))),
        ("a{{b{}c}}d{{", Some(("a{b", "c}d{"))),
        ("{{}}{}", Some(("{}", ""))),
        ("{}{{}}", Some(("", "{}"))),
        ("é{{{!s}}}世界", Some(("é{", "}世界"))),
        ("", None),
        ("plain", None),
        ("{{}}", None),
        ("{x}", None),
        ("{0}", None),
        ("{!r}", None),
        ("{!x}", None),
        ("{!}", None),
        ("a{", None),
        ("a}", None),
        ("a{}{}", None),
        ("{}tail{", None),
        ("{}x}", None),
    ];
    for &(source, expected) in cases {
        let parsed = parse_format_one(source);
        let actual = parsed.as_ref().map(|(before, after)| (&**before, &**after));
        assert_eq!(actual, expected, "source={source:?}");
    }
}

#[test]
fn compiled_format_segments_retain_frozen_ownership() {
    let source = String::from(
        r#"
def format_unicode(x):
    return "é:{}:世界".format(x)
def format_escaped(x):
    return "{{{!s}}}".format(x)
assert_eq(format_unicode("X"), "é:X:世界")
assert_eq(format_unicode(7), "é:7:世界")
assert_eq(format_escaped("X"), "{X}")
assert_eq(format_escaped(7), "{7}")
garbage_collect()
"#,
    );
    let module = Assert::new().pass_module(&source);
    let unicode = module.get("format_unicode").unwrap();
    let escaped = module.get("format_escaped").unwrap();
    let clone = module.clone();
    drop(source);
    drop(module);
    drop(clone);
    let mut a = Assert::new();
    a.globals_add(|builder| {
        builder.set("format_unicode", unicode);
        builder.set("format_escaped", escaped);
    });
    a.pass(
        r#"
RESULT = [format_unicode("X"), format_unicode(7), format_escaped("X"), format_escaped(7)]
garbage_collect()
assert_eq(RESULT, ["é:X:世界", "é:7:世界", "{X}", "{7}"])
assert_eq("{name}".format(name="X"), "X")
assert_eq("{0}".format("X"), "X")
assert_eq("{}{}".format("X", "Y"), "XY")
assert_eq("{!r}".format([1]), "[1]")
"#,
    );
    a.fails("'unmatched{'.format(1)", &["Unmatched '{'"]);
    a.fails("'{}{}'.format(1)", &["Not enough parameters"]);
}
