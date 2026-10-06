/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 * You may select, at your option, one of the above-listed licenses.
 */

use std::cell::RefCell;

use super::*;

#[test]
fn atom_and_scalar_render_transfer_tight_buffers_and_release_spare_capacity() {
    let expected = format!("é\0{}", "candidate".repeat(1024));
    let mut transfers = Vec::new();
    for route in ["atom", "scalar"] {
        for allocation in ["tight", "spare"] {
            let mut value = if allocation == "tight" {
                CompactString::new(&expected)
            } else {
                CompactString::with_capacity(expected.len() * 3)
            };
            if allocation == "spare" {
                value.push_str(&expected);
            }
            let pointer = value.as_ptr();
            let capacity = value.capacity();
            assert_eq!(capacity == value.len(), allocation == "tight");
            let atom = AttrCandidateAtom::String(value);
            let no_label = |_: &CanonicalLabel| -> CompactString { panic!("unexpected label") };
            let output = match route {
                "atom" => atom.render(&no_label),
                _ => AttrCandidateValue::Scalar(atom).render(&no_label),
            };
            assert_eq!(output.as_str(), expected);
            assert_eq!(output.capacity(), expected.len());
            let shared = output.as_ptr() == pointer;
            println!(
                "ATTR_RENDER route={route} allocation={allocation} bytes={} input_capacity={capacity} output_capacity={} transferred={shared}",
                output.len(),
                output.capacity()
            );
            if allocation == "spare" {
                assert!(!shared);
            } else {
                transfers.push(shared);
            }
        }
    }
    assert_eq!(transfers, [true; 2]);
}

#[test]
fn rendering_preserves_callback_order_duplicates_and_owned_results() {
    let text = format!("世界\0{}", "entry".repeat(1024));
    let expected_list = format!("[{text}, @@//pkg:first, {text}, @@//pkg:first]");
    let expected_dict = format!("{{@@//pkg:second={text}, {text}=[@@//pkg:first, {text}]}}");
    let (results, callbacks) = {
        let first = CanonicalLabel::parse("@@//pkg:first").unwrap();
        let second = CanonicalLabel::parse("@@//pkg:second").unwrap();
        let seen = RefCell::new(Vec::new());
        let render = |label: &CanonicalLabel| {
            let rendered = CompactString::new(label.to_string());
            seen.borrow_mut().push(rendered.clone());
            rendered
        };
        let list = AttrCandidateValue::List(vec![
            AttrCandidateAtom::String(text.clone().into()),
            AttrCandidateAtom::Label(&first),
            AttrCandidateAtom::String(text.clone().into()),
            AttrCandidateAtom::Label(&first),
        ]);
        let dict = AttrCandidateValue::Dict(vec![
            (
                AttrCandidateAtom::Label(&second),
                AttrCandidateValue::Scalar(AttrCandidateAtom::String(text.clone().into())),
            ),
            (
                AttrCandidateAtom::String(text.clone().into()),
                AttrCandidateValue::List(vec![
                    AttrCandidateAtom::Label(&first),
                    AttrCandidateAtom::String(text.clone().into()),
                ]),
            ),
        ]);
        let results = [
            list.render(&render),
            dict.render(&render),
            AttrCandidateValue::List(vec![]).render(&render),
            AttrCandidateValue::Dict(vec![]).render(&render),
            AttrCandidateValue::Scalar(AttrCandidateAtom::String("".into())).render(&render),
            AttrCandidateAtom::String("inline".into()).render(&render),
        ];
        (results, seen.into_inner())
    };
    drop(text);
    assert_eq!(
        results.map(|value| value.to_string()),
        [
            expected_list,
            expected_dict,
            "[]".into(),
            "{}".into(),
            "".into(),
            "inline".into()
        ]
    );
    assert_eq!(
        callbacks,
        [
            "@@//pkg:first",
            "@@//pkg:first",
            "@@//pkg:second",
            "@@//pkg:first"
        ]
    );
}

#[test]
fn public_candidates_remain_owned_after_coerced_inputs_drop() {
    let text = format!("é\0{}", "source".repeat(1024));
    let expected = format!("[{text}, {text}]");
    let candidates = {
        let value = CoercedAttributeValue::StringList(Arc::from([
            CompactString::new(&text),
            CompactString::new(&text),
        ]));
        value
            .attr_visible_candidates(|_| panic!("unexpected label"))
            .unwrap()
    };
    drop(text);
    assert_eq!(candidates, [expected]);
}

#[test]
fn concatenated_candidate_current_render_releases_spare_capacity() {
    let left = "x".repeat(8192);
    let expression = CoercedAttributeValue::Concatenation(
        Arc::new(CoercedAttributeValue::String(left.clone().into())),
        Arc::new(CoercedAttributeValue::String("y".into())),
    );
    let mut candidates = expand_attr_candidates(&expression).unwrap();
    assert_eq!(candidates.len(), 1);
    let candidate = candidates.pop().unwrap();
    let capacity = match &candidate.value {
        AttrCandidateValue::Scalar(AttrCandidateAtom::String(value)) => value.capacity(),
        _ => panic!("unexpected candidate shape"),
    };
    let rendered = candidate.value.render(&|_| panic!("unexpected label"));
    assert_eq!(rendered.as_str(), format!("{left}y"));
    println!(
        "ATTR_CONCAT bytes={} temporary_capacity={capacity} final_capacity={}",
        rendered.len(),
        rendered.capacity()
    );
    assert!(capacity > rendered.capacity());
}
