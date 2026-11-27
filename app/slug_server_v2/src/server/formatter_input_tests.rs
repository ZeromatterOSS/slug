use super::*;

fn formatter_request(name: &str, source: &str, request_first: bool) -> String {
    let body = format!(
        r#"{{"expression":"//:target","output":{{"starlark_file":{{"name":{name},"source":{source}}}}}}}"#
    );
    if request_first {
        format!(r#"{{"request":{body},"kind":"cquery"}}"#)
    } else {
        format!(r#"{{"kind":"cquery","request":{body}}}"#)
    }
}

#[test]
fn daemon_preflights_decoded_formatter_bytes_with_either_envelope_order() {
    let name = r"\u00e9".repeat(CQUERY_STARLARK_NAME_MAX_BYTES / 2);
    let source = r"\ud83d\ude00".repeat(CQUERY_STARLARK_SOURCE_MAX_BYTES / 4);
    for request_first in [false, true] {
        let exact = formatter_request(
            &format!("\"{name}\""),
            &format!("\"{source}\""),
            request_first,
        );
        let DaemonRequest::Cquery(request) = decode_request(&exact).unwrap() else {
            panic!("expected cquery request");
        };
        assert_eq!(
            request.output,
            CqueryOutput::StarlarkFile {
                name: "é".repeat(CQUERY_STARLARK_NAME_MAX_BYTES / 2),
                source: "😀".repeat(CQUERY_STARLARK_SOURCE_MAX_BYTES / 4),
            }
        );

        let excess_source =
            formatter_request(r#""fmt.bzl""#, &format!("\"{source}x\""), request_first);
        assert!(matches!(
            decode_request(&excess_source),
            Err(RequestDecodeError::FormatterInput(
                "invalid --starlark:file: formatter source byte limit exceeded"
            ))
        ));
        let excess_name = formatter_request(&format!("\"{name}x\""), r#""""#, request_first);
        assert!(matches!(
            decode_request(&excess_name),
            Err(RequestDecodeError::FormatterInput(
                "invalid --starlark:file: invalid formatter file name"
            ))
        ));
    }
}

#[test]
fn daemon_preserves_json_string_validity_and_names_with_literal_escape_spelling() {
    let valid = formatter_request(r#""fmt\\u0000.bzl""#, r#""\n雪\u0100\/\t\"""#, false);
    let DaemonRequest::Cquery(request) = decode_request(&valid).unwrap() else {
        panic!("expected cquery request");
    };
    assert_eq!(
        request.output,
        CqueryOutput::StarlarkFile {
            name: r"fmt\u0000.bzl".to_owned(),
            source: "\n雪Ā/\t\"".to_owned(),
        }
    );
    for (name, source) in [
        (r#""fmt.bzl""#, "[]"),
        (r#""fmt.bzl""#, r#""\ud800""#),
        (r#""fmt.bzl""#, r#""\ud800\ud800""#),
        (r#""fmt.bzl""#, r#""\udc00""#),
        ("null", r#""""#),
    ] {
        assert!(matches!(
            decode_request(&formatter_request(name, source, false)),
            Err(RequestDecodeError::Malformed)
        ));
    }
    assert!(matches!(
        decode_request(&formatter_request(r#""bad\u0000name""#, r#""""#, true)),
        Err(RequestDecodeError::FormatterInput(
            "invalid --starlark:file: invalid formatter file name"
        ))
    ));
}

#[test]
fn daemon_decoder_preserves_existing_scalar_output_and_request_shapes() {
    for (kind, request) in [
        ("build", r#"{"targets":["//:target"]}"#),
        ("run", r#"{"targets":["//:target"]}"#),
        (
            "query",
            r#"{"expression":"//:target","order_output":"auto"}"#,
        ),
        ("aquery", r#"{"expression":"//:target"}"#),
        ("cquery", r#"{"expression":"//:target","output":"label"}"#),
        (
            "cquery",
            r#"{"expression":"//:target","output":"label_kind"}"#,
        ),
        (
            "cquery",
            r#"{"expression":"//:target","output":"starlark_label"}"#,
        ),
        ("cquery", r#"{"expression":"//:target","output":"graph"}"#),
    ] {
        for raw in [
            format!(r#"{{"kind":"{kind}","request":{request}}}"#),
            format!(r#"{{"request":{request},"kind":"{kind}"}}"#),
        ] {
            let old: DaemonRequest = serde_json::from_str(&raw).unwrap();
            let new = decode_request(&raw).unwrap();
            assert_eq!(
                serde_json::to_value(old).unwrap(),
                serde_json::to_value(new).unwrap()
            );
        }
    }
}
