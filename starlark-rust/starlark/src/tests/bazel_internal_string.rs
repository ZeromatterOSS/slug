/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use starlark_syntax::codemap::Pos;
use starlark_syntax::codemap::Span;
use starlark_syntax::syntax::module::AstModuleFields;

use crate::environment::Globals;
use crate::environment::GlobalsBuilder;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::syntax::StringEncoding;

const CARRIER_E_ACUTE: &str = "\u{c3}\u{a9}";

fn parse(source: &str, string_encoding: StringEncoding) -> AstModule {
    AstModule::parse_with_string_encoding(
        "strings.bzl",
        source.to_owned(),
        &Dialect::Standard,
        string_encoding,
    )
    .unwrap()
}

fn eval(source: &str, string_encoding: StringEncoding) -> Module {
    let ast = parse(source, string_encoding);
    let module = Module::new();
    Evaluator::new(&module)
        .eval_module(ast, &Globals::standard())
        .unwrap();
    module
}

#[test]
fn test_standard_string_encoding_is_unchanged() {
    let source = r#"
UTF8 = "é"
OCTAL = "\351"
OCTAL_400 = "\400"
HEX = "\x41"
UNICODE = "\u00e9"
NON_BMP = "\U0001f600"
UNKNOWN = "\q"
"#;
    let ordinary = {
        let ast = AstModule::parse("strings.bzl", source.to_owned(), &Dialect::Standard).unwrap();
        let module = Module::new();
        Evaluator::new(&module)
            .eval_module(ast, &Globals::standard())
            .unwrap();
        module
    };
    let explicit = eval(source, StringEncoding::Unicode);

    for name in [
        "UTF8",
        "OCTAL",
        "OCTAL_400",
        "HEX",
        "UNICODE",
        "NON_BMP",
        "UNKNOWN",
    ] {
        assert_eq!(
            ordinary.get(name).unwrap().unpack_str(),
            explicit.get(name).unwrap().unpack_str()
        );
    }
    assert_eq!(ordinary.get("UTF8").unwrap().unpack_str(), Some("é"));
    assert_eq!(ordinary.get("OCTAL").unwrap().unpack_str(), Some("é"));
    assert_eq!(ordinary.get("OCTAL_400").unwrap().unpack_str(), Some("Ā"));
    assert_eq!(ordinary.get("UNKNOWN").unwrap().unpack_str(), Some("\\q"));
}

#[test]
fn test_bazel_internal_source_spans_and_reporting_columns() {
    let source = "VALUE = \"é\"\n";
    let ordinary = AstModule::parse("strings.bzl", source.to_owned(), &Dialect::Standard).unwrap();
    let bazel = parse(source, StringEncoding::BazelInternal);
    let literal_begin = source.find('é').unwrap();
    let literal_end = literal_begin + 'é'.len_utf8();
    let literal_span = Span::new(Pos::new(literal_begin as u32), Pos::new(literal_end as u32));
    let point_after_literal = Span::new(Pos::new(literal_end as u32), Pos::new(literal_end as u32));

    assert_eq!(ordinary.codemap().source(), source);
    assert_eq!(bazel.codemap().source(), source);
    assert_eq!(ordinary.codemap().full_span(), bazel.codemap().full_span());
    assert_eq!(ordinary.statement().span, bazel.statement().span);
    assert_eq!(ordinary.codemap().source_span(literal_span), "é");
    assert_eq!(bazel.codemap().source_span(literal_span), "é");
    assert_eq!(
        ordinary.codemap().resolve_span(point_after_literal),
        bazel.codemap().resolve_span(point_after_literal)
    );
    assert_eq!(
        ordinary
            .codemap()
            .file_span(point_after_literal)
            .to_string(),
        bazel.codemap().file_span(point_after_literal).to_string()
    );
    assert_eq!(
        ordinary
            .codemap()
            .resolve_span_for_reporting(point_after_literal),
        ordinary.codemap().resolve_span(point_after_literal)
    );
    let unicode = bazel.codemap().resolve_span(point_after_literal);
    let byte = bazel
        .codemap()
        .resolve_span_for_reporting(point_after_literal);
    assert_eq!(byte.begin.column, unicode.begin.column + 1);
    assert_eq!(byte.end.column, unicode.end.column + 1);
}

#[test]
fn test_bazel_internal_oracle_matrix() {
    let module = eval(
        r#"
def identity(value):
    return value

def concat(left, right):
    return left + right

def pattern(value):
    return value + "/**/*.txt"

UTF8 = "é"
TWO_OCTAL = "\303\251"
ONE_OCTAL = "\351"
RAW_UTF8 = r"é"
RAW = r"\303\251"
NON_BMP = "😀"
STORED = [ONE_OCTAL, UTF8]
BY_KEY = {TWO_OCTAL: "two", ONE_OCTAL: "one"}
RETURNED = identity(UTF8)
PATTERN = identity(pattern(RETURNED))
RESULT = (
    'é' == UTF8 and
    """é""" == UTF8 and
    UTF8 == TWO_OCTAL and
    UTF8 != ONE_OCTAL and
    RAW_UTF8 == UTF8 and
    RAW == "\\303\\251" and
    len("") == 0 and
    len("\0") == 1 and
    "\0" == "\000" and
    len("\377") == 1 and
    "\3777" == "\377" + "7" and
    "\378" == "\37" + "8" and
    len("\3777") == 2 and
    len("\378") == 2 and
    len(NON_BMP) == 4 and
    NON_BMP == "\360\237\230\200" and
    "\251" < UTF8 and
    UTF8 < ONE_OCTAL and
    sorted([ONE_OCTAL, UTF8, "\251"]) == ["\251", UTF8, ONE_OCTAL] and
    concat(RETURNED, "\377") == "\303\251\377" and
    RETURNED * 2 == "\303\251\303\251" and
    RETURNED[0] == "\303" and
    RETURNED[1:] == "\251" and
    RETURNED in STORED and
    ONE_OCTAL in STORED and
    BY_KEY[RETURNED] == "two" and
    BY_KEY[ONE_OCTAL] == "one" and
    len(BY_KEY) == 2 and
    PATTERN == "\303\251/**/*.txt" and
    PATTERN[:2] == RETURNED
)
"#,
        StringEncoding::BazelInternal,
    );

    assert_eq!(module.get("RESULT").unwrap().unpack_bool(), Some(true));
    assert_eq!(
        module.get("UTF8").unwrap().unpack_str(),
        Some(CARRIER_E_ACUTE)
    );
}

#[test]
fn test_bazel_internal_separate_parse_and_freeze_pass_through() {
    let literal = eval(r#"VALUE = "é""#, StringEncoding::BazelInternal);
    let octal = eval(r#"VALUE = "\303\251""#, StringEncoding::BazelInternal);
    let literal_value = literal.get("VALUE").unwrap();
    let octal_value = octal.get("VALUE").unwrap();
    assert!(literal_value.equals(octal_value).unwrap());

    let module = eval(
        r#"
def pass_through(value):
    return value

VALUE = pass_through("é")
"#,
        StringEncoding::BazelInternal,
    );
    assert_eq!(
        module.get("VALUE").unwrap().unpack_str(),
        Some(CARRIER_E_ACUTE)
    );
    let frozen = module.freeze().unwrap();
    let function = frozen.get("pass_through").unwrap();
    let value = frozen.get("VALUE").unwrap();
    let call_module = Module::new();
    let returned = Evaluator::new(&call_module)
        .eval_function(function.value(), &[value.value()], &[])
        .unwrap();
    assert!(returned.equals(value.value()).unwrap());
}

#[test]
fn test_bazel_internal_invalid_escapes_and_identifier_location() {
    let invalid = [
        (
            "filegroup(\n    name = \"bad\",\n    tags = [\"\\400\"],\n)",
            "octal escape sequence out of range (maximum is \\377)",
            "3:17",
        ),
        (
            "filegroup(\n    name = \"bad\",\n    tags = [\"\\x41\"],\n)",
            "invalid escape sequence: \\x. Use '\\\\' to insert '\\'.",
            "3:15",
        ),
        (
            "filegroup(\n    name = \"bad\",\n    tags = [\"\\u00e9\"],\n)",
            "invalid escape sequence: \\u. Use '\\\\' to insert '\\'.",
            "3:15",
        ),
        (
            "filegroup(\n    name = \"bad\",\n    tags = [\"\\U0001f600\"],\n)",
            "invalid escape sequence: \\U. Use '\\\\' to insert '\\'.",
            "3:15",
        ),
        (
            "filegroup(\n    name = \"bad\",\n    tags = [\"\\q\"],\n)",
            "invalid escape sequence: \\q. Use '\\\\' to insert '\\'.",
            "3:15",
        ),
    ];
    for (source, message, location) in invalid {
        let error = AstModule::parse_with_string_encoding(
            "bad/BUILD.bazel",
            source.to_owned(),
            &Dialect::Standard,
            StringEncoding::BazelInternal,
        )
        .unwrap_err();
        assert_eq!(error.without_diagnostic().to_string(), message);
        let span = error.span().unwrap();
        assert_eq!(
            span.file.resolve_span_for_reporting(span.span).to_string(),
            location
        );
    }

    let source = r#"filegroup(
    name = "bad",
    tags = ["""é""" + missing_name],
)"#;
    let ast = parse(source, StringEncoding::BazelInternal);
    let module = Module::new();
    let standard = Globals::standard();
    let mut globals = GlobalsBuilder::standard();
    globals.set("filegroup", standard.get_frozen("dict").unwrap());
    let globals = globals.build();
    let error = Evaluator::new(&module)
        .eval_module(ast, &globals)
        .unwrap_err();
    assert_eq!(
        error.without_diagnostic().to_string(),
        "Variable `missing_name` not found"
    );
    let span = error.span().unwrap();
    assert_eq!(span.source_span(), "missing_name");
    let unicode = span.resolve_span();
    let byte = span.resolve_span_for_reporting();
    assert_eq!(byte.begin.to_string(), "3:24");
    assert_eq!(byte.begin.column, unicode.begin.column + 1);
    assert_eq!(byte.end.column, unicode.end.column + 1);
}
