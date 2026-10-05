use super::parse_percent_s_one;
use crate::assert::Assert;
use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::syntax::StringEncoding;

#[test]
fn percent_s_unescaped_segments_share_input() {
    let prefix = "é".repeat(512);
    let suffix = "世界".repeat(256);
    let source = format!("{prefix}%s{suffix}");
    let (before, after) = parse_percent_s_one(&source).unwrap();
    assert_eq!(&*before, prefix);
    assert_eq!(&*after, suffix);
    assert_eq!(before.as_ptr(), source.as_ptr());
    assert_eq!(after.as_ptr(), source[prefix.len() + 2..].as_ptr());
}

#[test]
fn percent_s_escapes_unicode_and_rejection_are_preserved() {
    let cases: &[(&str, Option<(&str, &str)>)] = &[
        ("%s", Some(("", ""))),
        ("é%s世界", Some(("é", "世界"))),
        ("\0%sé", Some(("\0", "é"))),
        ("a%s", Some(("a", ""))),
        ("%sb", Some(("", "b"))),
        ("%%%s%%", Some(("%", "%"))),
        ("%%%%%s%%%%", Some(("%%", "%%"))),
        ("a%%b%sc%%d%%", Some(("a%b", "c%d%"))),
        ("%%s%s%%s", Some(("%s", "%s"))),
        ("é%%%s%%世界", Some(("é%", "%世界"))),
        ("", None),
        ("plain", None),
        ("%%", None),
        ("%%s", None),
        ("%d", None),
        ("%r", None),
        ("%q", None),
        ("a%", None),
        ("a%s%", None),
        ("a%s%s", None),
        ("%sx%d", None),
        ("%sé%", None),
    ];
    for &(source, expected) in cases {
        let parsed = parse_percent_s_one(source);
        let actual = parsed.as_ref().map(|(before, after)| (&**before, &**after));
        assert_eq!(actual, expected, "source={source:?}");
    }
}

#[test]
fn compiled_percent_s_segments_retain_frozen_ownership() {
    let source = String::from(
        r#"
def format_unicode(x):
    return "é:%s:世界" % x
def format_escaped(x):
    return "%%%s%%" % x
def format_number(x):
    return "%d" % x
def format_repr(x):
    return "%r" % x
def format_two(x):
    return "%s/%s" % x
assert_eq(format_unicode("X"), "é:X:世界")
assert_eq(format_unicode(7), "é:7:世界")
assert_eq(format_escaped(("X",)), "%X%")
garbage_collect()
"#,
    );
    let module = Assert::new().pass_module(&source);
    let functions: Vec<_> = [
        "format_unicode",
        "format_escaped",
        "format_number",
        "format_repr",
        "format_two",
    ]
    .map(|name| (name, module.get(name).unwrap()))
    .into();
    let clone = module.clone();
    drop(source);
    drop(module);
    drop(clone);
    let mut a = Assert::new();
    a.globals_add(|builder| {
        for (name, function) in functions {
            builder.set(name, function);
        }
    });
    a.pass(
        r#"
RESULT = [format_unicode("X"), format_unicode(7), format_escaped(("X",)), format_escaped(7)]
garbage_collect()
assert_eq(RESULT, ["é:X:世界", "é:7:世界", "%X%", "%7%"])
assert_eq(format_unicode((7,)), "é:7:世界")
assert_eq(format_number(7), "7")
assert_eq(format_repr([1]), "[1]")
assert_eq(format_two(("X", "Y")), "X/Y")
"#,
    );
    a.fails(
        "format_unicode(())",
        &["Not enough arguments for format string"],
    );
    a.fails(
        "format_unicode((1, 2))",
        &["Too many arguments for format string"],
    );
}

#[test]
fn percent_s_compiler_preserves_bazel_internal_carriers() {
    let functions = {
        let ast = AstModule::parse_with_string_encoding(
            "percent-memory.bzl",
            "def plain(x):\n    return 'é%s界' % x\ndef escaped(x):\n    return '%%é%s界%%' % x\n"
                .to_owned(),
            &Dialect::Bazel,
            StringEncoding::BazelInternal,
        )
        .unwrap();
        let module = Module::new();
        Evaluator::new(&module)
            .eval_module(ast, &Globals::standard())
            .unwrap();
        let frozen = module.freeze().unwrap();
        [frozen.get("plain").unwrap(), frozen.get("escaped").unwrap()]
    };
    let caller = Module::new();
    let mut eval = Evaluator::new(&caller);
    for (function, expected) in functions.iter().zip([
        "\u{c3}\u{a9}X\u{e7}\u{95}\u{8c}",
        "%\u{c3}\u{a9}X\u{e7}\u{95}\u{8c}%",
    ]) {
        let value = eval
            .eval_function(function.value(), &[caller.heap().alloc("X")], &[])
            .unwrap();
        assert_eq!(value.unpack_str().unwrap(), expected);
    }
}
