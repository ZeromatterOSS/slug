/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 * You may select, at your option, one of the above-listed licenses.
 */

use super::*;
use crate::attrs::AttributeProvenance;

fn attribute(name: &str, value: CoercedAttributeValue) -> AttributeValue {
    AttributeValue {
        declaration_name: name.into(),
        provenance: AttributeProvenance::Explicit,
        value: Arc::new(value),
    }
}

fn label(target: &str) -> CanonicalLabel {
    CanonicalLabel::parse(&format!("@@//pkg:{target}")).unwrap()
}

#[test]
fn projected_values_share_all_six_source_shapes_and_first_duplicate_owners() {
    let text = format!("é\0{}", "text".repeat(1024));
    let second = format!("世界{}", "second".repeat(1024));
    let source = format!("dir.with.dot/é{}.tar", "source".repeat(1024));
    let output = format!("output-{}.gen", "output".repeat(1024));
    let mut cases = Vec::new();
    let string = CompactString::new(&text);
    let pointer = string.as_ptr();
    cases.push((
        "string",
        CoercedAttributeValue::String(string),
        vec![(text.clone(), pointer)],
    ));
    let strings: Arc<[CompactString]> = Arc::from([
        text.clone().into(),
        text.clone().into(),
        second.clone().into(),
    ]);
    let expected = vec![
        (text.clone(), strings[0].as_ptr()),
        (second.clone(), strings[2].as_ptr()),
    ];
    cases.push((
        "strings",
        CoercedAttributeValue::StringList(strings),
        expected,
    ));
    let single_label = label(&format!("{source}.gz"));
    let pointer = single_label.target().as_str().as_ptr();
    cases.push((
        "label",
        CoercedAttributeValue::Label(single_label),
        vec![(source.clone(), pointer)],
    ));
    let labels: Arc<[CanonicalLabel]> = Arc::from([
        label(&format!("{source}.gz")),
        label(&format!("{source}.zip")),
        label(&format!("{source}2.ext")),
    ]);
    assert_ne!(
        labels[0].target().as_str().as_ptr(),
        labels[1].target().as_str().as_ptr()
    );
    let expected = vec![
        (source.clone(), labels[0].target().as_str().as_ptr()),
        (format!("{source}2"), labels[2].target().as_str().as_ptr()),
    ];
    cases.push(("labels", CoercedAttributeValue::LabelList(labels), expected));
    let single_output = label(&output);
    let pointer = single_output.target().as_str().as_ptr();
    cases.push((
        "output",
        CoercedAttributeValue::Output(single_output),
        vec![(output.clone(), pointer)],
    ));
    let outputs: Arc<[CanonicalLabel]> =
        Arc::from([label(&output), label(&output), label(&format!("{output}2"))]);
    let expected = vec![
        (output.clone(), outputs[0].target().as_str().as_ptr()),
        (format!("{output}2"), outputs[2].target().as_str().as_ptr()),
    ];
    cases.push((
        "outputs",
        CoercedAttributeValue::OutputList(outputs),
        expected,
    ));
    let mut sharing = Vec::new();
    for (name, value, expected) in cases {
        let actual = projected_values(name, &value).unwrap();
        let bytes: Vec<&str> = actual.iter().map(AsRef::<str>::as_ref).collect();
        assert_eq!(
            bytes,
            expected.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(),
            "{name}"
        );
        let count = actual
            .iter()
            .zip(&expected)
            .filter(|(a, b)| a.as_ptr() == b.1)
            .count();
        println!(
            "OUTPUT_PROJECTION shape={name} count={} shared={count}",
            actual.len()
        );
        sharing.push(count == actual.len());
    }
    assert_eq!(sharing, [true; 6]);
}

#[test]
fn single_placeholders_share_target_and_attribute_bytes() {
    let target = format!("{}/{}", "dirname".repeat(1024), "basename".repeat(1024));
    let text = format!("text-{}", "value".repeat(1024));
    let attributes = [attribute(
        "text",
        CoercedAttributeValue::String(text.clone().into()),
    )];
    let mut sharing = Vec::new();
    for name in ["name", "dirname", "basename", "text"] {
        let expected = if name == "text" {
            match attributes[0].value.as_ref() {
                CoercedAttributeValue::String(value) => value.as_str(),
                _ => unreachable!(),
            }
        } else {
            target_placeholder(name, &target).unwrap()
        };
        let actual = single_value(name, &target, &attributes).unwrap().unwrap();
        let bytes: &str = actual.as_ref();
        assert_eq!(bytes, expected);
        let shared = actual.as_ptr() == expected.as_ptr();
        println!(
            "OUTPUT_SINGLE name={name} bytes={} shared={shared}",
            bytes.len()
        );
        sharing.push(shared);
    }
    assert_eq!(sharing, [true; 4]);
}

#[test]
fn projected_output_semantics_and_owned_results_survive_input_drop() {
    let text = format!("é\0{}", "value".repeat(1024));
    let target = "dir/世界".to_owned();
    let (results, errors) = {
        let attributes = vec![
            attribute("text", CoercedAttributeValue::String(text.clone().into())),
            attribute(
                "labels",
                CoercedAttributeValue::LabelList(Arc::from([
                    label("dir.with.dot/.hidden"),
                    label("dir.with.dot/.other"),
                ])),
            ),
            attribute(
                "words",
                CoercedAttributeValue::StringList(Arc::from(["é\0".into(), "é\0".into()])),
            ),
            attribute(
                "multiple",
                CoercedAttributeValue::StringList(Arc::from(["one".into(), "two".into()])),
            ),
            attribute("empty", CoercedAttributeValue::StringList(Arc::from([]))),
            attribute(
                "configured",
                CoercedAttributeValue::Concatenation(
                    Arc::new(CoercedAttributeValue::String("a".into())),
                    Arc::new(CoercedAttributeValue::String("b".into())),
                ),
            ),
        ];
        let entries = [
            ("repeated".into(), "%{text}:%{text}".into()),
            ("special".into(), "%{dirname}%{basename}-%{name}".into()),
            ("dedup".into(), "%{labels}%{words}".into()),
            ("malformed".into(), "%literal-%{tail".into()),
        ];
        let results = resolve_output_names(&entries, &target, &attributes).unwrap();
        let errors = [
            "%{missing}",
            "%{empty}",
            "%{multiple}",
            "%{configured}",
            "%{missing}%{configured}",
            "%{configured}%{missing}",
        ]
        .map(|template| {
            resolve_output_names(&[("result".into(), template.into())], &target, &attributes)
                .unwrap_err()
        });
        (results, errors)
    };
    let expected = [
        format!("{text}:{text}"),
        "dir/世界-dir/世界".to_owned(),
        "dir.with.dot/é\0".to_owned(),
        "%literal-%{tail".to_owned(),
    ];
    drop(text);
    drop(target);
    assert_eq!(
        results.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
        ["repeated", "special", "dedup", "malformed"]
    );
    assert_eq!(
        results.iter().map(|(_, v)| v.as_str()).collect::<Vec<_>>(),
        expected.iter().map(|v| v.as_str()).collect::<Vec<_>>()
    );
    let invalid = "For attribute 'result' in outputs: Invalid placeholder(s) in template";
    let configured = "Attribute configured is configurable and cannot be used in outputs";
    assert_eq!(
        errors.map(|e| e.to_string()),
        [invalid, invalid, invalid, configured, invalid, configured]
    );
    assert!(resolve_output_names(&[], "", &[]).unwrap().is_empty());
}
