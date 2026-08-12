/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory.
 */

use std::sync::Arc;

use compact_str::CompactString;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;
use starlark_map::sorted_map::SortedMap;

use crate::lockfile_v28::*;

fn parsed(source: &str) -> BazelLockfileV28 {
    match read_lockfile_v28(source.as_bytes(), UnsupportedVersionPolicy::Error).unwrap() {
        LockfileReadOutcome::Parsed(value) => value,
        LockfileReadOutcome::Empty => panic!("expected parsed lockfile"),
    }
}

fn parse_error_for(source: &str) -> LockfileParseError {
    read_lockfile_v28(source.as_bytes(), UnsupportedVersionPolicy::Error).unwrap_err()
}

fn object(fields: &str) -> String {
    format!("{{\"lockFileVersion\":28{fields}}}")
}

fn empty_render() -> String {
    "{\n  \"lockFileVersion\": 28,\n  \"registryFileHashes\": {},\n  \"selectedYankedVersions\": {},\n  \"moduleExtensions\": {},\n  \"facts\": {},\n  \"factsVersions\": {}\n}\n"
        .to_owned()
}

fn extension_json(extra: &str) -> String {
    object(concat!(
        ",\"moduleExtensions\":{\"//:ext.bzl%x\":{\"general\":{",
        "\"bzlTransitiveDigest\":\"AQ==\",",
        "\"usagesDigest\":\"AgM=\",",
        "\"recordedInputs\":[],",
        "\"generatedRepoSpecs\":{}__EXTRA__",
        "}}}"
    ))
    .replace("__EXTRA__", extra)
}

fn only_extension(lockfile: &BazelLockfileV28) -> &LockfileModuleExtension {
    lockfile
        .module_extensions
        .values()
        .next()
        .unwrap()
        .values()
        .next()
        .unwrap()
}

#[test]
fn lockfile_v28_java_utf8_replacement_precedes_marker_scan() {
    let bytes = b"{\"unknown\":\"\xff\",\"lockFileVersion\":28}";
    assert!(matches!(
        read_lockfile_v28(bytes, UnsupportedVersionPolicy::Error).unwrap(),
        LockfileReadOutcome::Parsed(_)
    ));
}

#[test]
fn lockfile_v28_java_utf8_replacement_matches_input_stream_reader_consumption() {
    assert_eq!(java_utf8_decode(b"\xed\xa0\x80"), "\u{fffd}");
    assert_eq!(java_utf8_decode(b"\xe2\x82"), "\u{fffd}");
    assert_eq!(java_utf8_decode(b"\xc0\x80"), "\u{fffd}\u{fffd}");
    assert_eq!(
        java_utf8_decode(b"\xe0\x80\x80"),
        "\u{fffd}\u{fffd}\u{fffd}"
    );
}

#[test]
fn lockfile_v28_first_textual_marker_wins_anywhere() {
    let source = "{\"unknown\":{\"lockFileVersion\":27},\"lockFileVersion\":28}";
    assert!(matches!(
        read_lockfile_v28(source.as_bytes(), UnsupportedVersionPolicy::ReturnEmpty).unwrap(),
        LockfileReadOutcome::Empty
    ));
}

#[test]
fn lockfile_v28_missing_and_unsupported_markers_follow_policy() {
    for source in ["{}", "{\"lockFileVersion\":27}"] {
        assert!(matches!(
            read_lockfile_v28(source.as_bytes(), UnsupportedVersionPolicy::ReturnEmpty).unwrap(),
            LockfileReadOutcome::Empty
        ));
        assert_eq!(
            parse_error_for(source).surface,
            LockfileParseErrorSurface::UnsupportedVersion
        );
    }
}

#[test]
fn lockfile_v28_overflowing_marker_is_typed_caught_failure() {
    let error = parse_error_for("{\"lockFileVersion\":999999999999999999999}");
    assert_eq!(
        error.surface,
        LockfileParseErrorSurface::CaughtIllegalArgument
    );
    assert!(matches!(
        error.kind,
        LockfileParseErrorKind::VersionMarkerOverflow { .. }
    ));
}

#[test]
fn lockfile_v28_nested_marker_can_admit_defaulted_top_level_version() {
    let value = parsed("{\"nested\":{\"lockFileVersion\":28}}");
    assert_eq!(value, BazelLockfileV28::default());
}

#[test]
fn lockfile_v28_gson_lenient_spelling_is_accepted() {
    let value = parsed("{\"lockFileVersion\":28, unknown:'accepted';}");
    assert_eq!(value, BazelLockfileV28::default());
}

#[test]
fn lockfile_v28_gson_keywords_are_ascii_case_insensitive() {
    let value = parsed(&object(
        ",\"facts\":{\"//:ext.bzl%x\":{\"truth\":TrUe,\"falsehood\":FaLsE,\"nothing\":NuLl}}",
    ));
    let facts = &value.facts.values().next().unwrap().values;
    assert!(matches!(facts.get("truth"), Some(FactValue::Bool(true))));
    assert!(matches!(
        facts.get("falsehood"),
        Some(FactValue::Bool(false))
    ));
    assert!(matches!(facts.get("nothing"), Some(FactValue::Null)));
}

#[test]
fn lockfile_v28_gson_lone_surrogates_use_java_replacement_without_overconsumption() {
    let value = parsed(&object(
        ",\"unknown\":\"\\uD800\",\"selectedYankedVersions\":{\
         \"m@1\":\"\\uD800\",\"m@2\":\"\u{fffd}\"},\
         \"moduleExtensions\":{\"//:ext.bzl%x\":{\"general\":{\
         \"bzlTransitiveDigest\":\"AQ==\",\"usagesDigest\":\"AgM=\",\
         \"recordedInputs\":[],\"generatedRepoSpecs\":{\"r\":{\
         \"repoRuleId\":\"//:r.bzl%\\uD800\",\"attributes\":{\
         \"lone\":\"\\uD800\",\"low\":\"\\uDC00\",\"literal\":\"\u{fffd}\",\
         \"nonlow\":\"\\uD800\\u0041\",\"pair\":\"\\uD83D\\uDE00\",\
         \"\\uD800\":1}}}}}},\
         \"facts\":{\"//:ext.bzl%x\":{\
         \"\\uD800\":\"fact key\",\
         \"high\":\"\\uD800x\",\"low\":\"\\uDC00\",\
         \"nonlow\":\"\\uD800\\u0041\",\"pair\":\"\\uD83D\\uDE00\"}}",
    ));
    let reasons: Vec<_> = value
        .selected_yanked_versions
        .values()
        .map(CompactString::as_str)
        .collect();
    assert_eq!(reasons, ["?", "\u{fffd}"]);
    let spec = only_extension(&value)
        .generated_repo_specs
        .values()
        .next()
        .unwrap();
    assert_eq!(spec.repo_rule_id.as_ref().unwrap().rule_name, "?");
    let attributes = &spec.attributes.as_ref().unwrap().values;
    assert!(matches!(
        attributes.get("lone"),
        Some(AttributeValue::String(value)) if value == "?"
    ));
    assert!(matches!(
        attributes.get("low"),
        Some(AttributeValue::String(value)) if value == "?"
    ));
    assert!(matches!(
        attributes.get("literal"),
        Some(AttributeValue::String(value)) if value == "\u{fffd}"
    ));
    assert!(matches!(
        attributes.get("nonlow"),
        Some(AttributeValue::String(value)) if value == "?A"
    ));
    assert!(matches!(
        attributes.get("pair"),
        Some(AttributeValue::String(value)) if value == "\u{1f600}"
    ));
    assert!(attributes.contains_key("?"));

    let facts = &value.facts.values().next().unwrap().values;
    assert!(facts.contains_key("\u{fffd}"));
    assert!(matches!(
        facts.get("high"),
        Some(FactValue::String(value)) if value == "\u{fffd}x"
    ));
    assert!(matches!(
        facts.get("low"),
        Some(FactValue::String(value)) if value == "\u{fffd}"
    ));
    assert!(matches!(
        facts.get("nonlow"),
        Some(FactValue::String(value)) if value == "\u{fffd}A"
    ));
    assert!(matches!(
        facts.get("pair"),
        Some(FactValue::String(value)) if value == "\u{1f600}"
    ));
}

#[test]
fn lockfile_v28_genuinely_malformed_json_is_rejected() {
    assert_eq!(
        parse_error_for("{\"lockFileVersion\":28").surface,
        LockfileParseErrorSurface::CaughtJsonSyntax
    );
}

#[test]
fn lockfile_v28_unknown_values_are_skipped_without_retention() {
    let value =
        parsed("{\"lockFileVersion\":28,\"unknown\":{\"duplicate\":1,\"duplicate\":[2,3]}}");
    assert_eq!(value, BazelLockfileV28::default());
}

#[test]
fn lockfile_v28_tokenizer_preserves_duplicate_order_and_raw_numbers() {
    let mut tokenizer = GsonTokenizer::new("{a:123456789012345678901234567890,a:null}");
    assert!(matches!(
        tokenizer.next_token().unwrap(),
        Some((_, GsonToken::BeginObject))
    ));
    assert!(matches!(
        tokenizer.next_token().unwrap(),
        Some((_, GsonToken::String(name))) if name == "a"
    ));
    assert!(matches!(
        tokenizer.next_token().unwrap(),
        Some((_, GsonToken::Colon))
    ));
    assert!(matches!(
        tokenizer.next_token().unwrap(),
        Some((_, GsonToken::Number(number))) if number == "123456789012345678901234567890"
    ));
    assert!(matches!(
        tokenizer.next_token().unwrap(),
        Some((_, GsonToken::Comma))
    ));
    assert!(matches!(
        tokenizer.next_token().unwrap(),
        Some((_, GsonToken::String(name))) if name == "a"
    ));
}

#[test]
fn lockfile_v28_missing_and_null_top_level_fields_keep_defaults() {
    let missing = parsed("{\"lockFileVersion\":28}");
    let nulls = parsed(
        "{\"lockFileVersion\":28,\"registryFileHashes\":null,\
         \"selectedYankedVersions\":null,\"moduleExtensions\":null,\"facts\":null,\
         \"factsVersions\":null}",
    );
    assert_eq!(missing, nulls);
}

#[test]
fn lockfile_v28_duplicate_top_level_last_non_null_wins() {
    let value = parsed(
        "{\"lockFileVersion\":28,\"factsVersions\":{\"//:ext.bzl%x\":1},\
         \"factsVersions\":null,\"factsVersions\":{\"//:ext.bzl%x\":2}}",
    );
    assert_eq!(*value.facts_versions.values().next().unwrap(), 2);
}

#[test]
fn lockfile_v28_duplicate_fact_object_keys_are_last_wins() {
    let value = parsed(&object(
        ",\"facts\":{\"//:ext.bzl%x\":{\"duplicate\":1,\"duplicate\":2}}",
    ));
    assert!(matches!(
        value.facts.values().next().unwrap().values.get("duplicate"),
        Some(FactValue::Number(FactNumber::Integer(value))) if value == "2"
    ));
}

#[test]
fn lockfile_v28_duplicate_attribute_object_keys_are_last_wins() {
    let value = parsed(&extension_json(
        ",\"generatedRepoSpecs\":{\"r\":{\"attributes\":{\
         \"outer\":1,\"outer\":2,\"nested\":{\"key\":1,\"key\":2}}}}",
    ));
    let attributes = &only_extension(&value)
        .generated_repo_specs
        .values()
        .next()
        .unwrap()
        .attributes
        .as_ref()
        .unwrap()
        .values;
    assert!(matches!(
        attributes.get("outer"),
        Some(AttributeValue::Int(2))
    ));
    assert!(matches!(
        attributes.get("nested"),
        Some(AttributeValue::Dict(values))
            if matches!(values.get(&AttributeKey::String("key".into())), Some(AttributeValue::Int(2)))
    ));
}

#[test]
fn lockfile_v28_strict_next_int_is_separate_from_attribute_narrowing() {
    let value = parsed(
        "{\"gate\":{\"lockFileVersion\":28},\"lockFileVersion\":2.8e1,\
         \"factsVersions\":{\"//:ext.bzl%x\":1e0}}",
    );
    assert_eq!(value.lock_file_version, 28);
    assert_eq!(*value.facts_versions.values().next().unwrap(), 1);

    for source in [
        "{\"gate\":{\"lockFileVersion\":28},\"lockFileVersion\":2147483648}",
        "{\"gate\":{\"lockFileVersion\":28},\"lockFileVersion\":28.5}",
        "{\"lockFileVersion\":28,\"factsVersions\":{\"//:ext.bzl%x\":2147483648}}",
        "{\"lockFileVersion\":28,\"factsVersions\":{\"//:ext.bzl%x\":1.5}}",
    ] {
        assert_eq!(
            parse_error_for(source).surface,
            LockfileParseErrorSurface::CaughtIllegalArgument
        );
    }
}

#[test]
fn lockfile_v28_strict_next_int_fallback_trims_java_ascii_whitespace() {
    let value = parsed(
        "{\"gate\":{\"lockFileVersion\":28},\
         \"lockFileVersion\":\"\\u0000\\t 2.8e1\\r\\n\",\
         \"factsVersions\":{\"//:ext.bzl%x\":\"\\f1e0\\u0020\"}}",
    );
    assert_eq!(value.lock_file_version, 28);
    assert_eq!(*value.facts_versions.values().next().unwrap(), 1);

    assert_eq!(
        parse_error_for(
            "{\"gate\":{\"lockFileVersion\":28},\"lockFileVersion\":\"\\u00a028\\u00a0\"}",
        )
        .surface,
        LockfileParseErrorSurface::CaughtIllegalArgument
    );
}

#[test]
fn lockfile_v28_strict_next_int_fallback_accepts_java_suffix_and_hex_floats() {
    let value = parsed(
        "{\"gate\":{\"lockFileVersion\":28},\"lockFileVersion\":\"2.8e1D\",\
         \"factsVersions\":{\
         \"//:a.bzl%x\":\"0x1.0p0\",\
         \"//:b.bzl%x\":\"+0X1p+4F\",\
         \"//:c.bzl%x\":\"-0x1.0p1d\",\
         \"//:d.bzl%x\":\"1d\",\
         \"//:e.bzl%x\":\"1F\",\
         \"//:f.bzl%x\":\"0x1.00000000000008p0\",\
         \"//:g.bzl%x\":\"0x1p-1075\"}}",
    );
    assert_eq!(value.lock_file_version, 28);
    let versions: Vec<_> = value.facts_versions.values().copied().collect();
    assert_eq!(versions, [1, 16, -2, 1, 1, 1, 0]);

    for spelling in [
        "1.5f",
        "1ff",
        "0x1.8p0",
        "0x1.000000000000081p0",
        "0x1p1024",
        "0x1.0",
        "0x.p0",
    ] {
        let source = format!(
            "{{\"lockFileVersion\":28,\"factsVersions\":{{\"//:ext.bzl%x\":\"{spelling}\"}}}}"
        );
        assert_eq!(
            parse_error_for(&source).surface,
            LockfileParseErrorSurface::CaughtIllegalArgument
        );
    }
}

#[test]
fn lockfile_v28_renderer_emits_six_fields_in_fixed_order_and_newline() {
    assert_eq!(
        render_lockfile_v28(&BazelLockfileV28::default()).unwrap(),
        empty_render()
    );
}

#[test]
fn lockfile_v28_checksum_absence_not_found_and_sha_are_distinct() {
    let value = parsed(&object(
        ",\"registryFileHashes\":{\"a\":\"not found\",\
         \"b\":\"0000000000000000000000000000000000000000000000000000000000000000\"}",
    ));
    assert!(matches!(
        value.registry_file_hashes.get("a"),
        Some(RegistryFileHash::NotFound)
    ));
    assert!(matches!(
        value.registry_file_hashes.get("b"),
        Some(RegistryFileHash::Sha256(_))
    ));
    assert!(value.registry_file_hashes.get("c").is_none());
}

#[test]
fn lockfile_v28_checksum_hex_case_normalizes() {
    let value = parsed(&object(&format!(
        ",\"registryFileHashes\":{{\"u\":\"{}\"}}",
        "AB".repeat(32)
    )));
    assert!(
        render_lockfile_v28(&value)
            .unwrap()
            .contains(&"ab".repeat(32))
    );
}

#[test]
fn lockfile_v28_invalid_checksum_is_direct_adapter_hole() {
    let error = parse_error_for(&object(",\"registryFileHashes\":{\"u\":\"bad\"}"));
    assert_eq!(
        error.surface,
        LockfileParseErrorSurface::DirectAdapterJsonParse
    );
}

#[test]
fn lockfile_v28_standard_base64_accepts_arbitrary_digest_lengths() {
    let value = parsed(&extension_json(""));
    let extension = only_extension(&value);
    assert_eq!(&*extension.bzl_transitive_digest, &[1]);
    assert_eq!(&*extension.usages_digest, &[2, 3]);
}

#[test]
fn lockfile_v28_unpadded_standard_base64_reads_and_renders_padded() {
    let source = extension_json("")
        .replace("\"AQ==\"", "\"AQ\"")
        .replace("\"AgM=\"", "\"AgM\"");
    let value = parsed(&source);
    let rendered = render_lockfile_v28(&value).unwrap();
    assert!(rendered.contains("\"bzlTransitiveDigest\": \"AQ==\""));
    assert!(rendered.contains("\"usagesDigest\": \"AgM=\""));
}

#[test]
fn lockfile_v28_base64_accepts_noncanonical_trailing_bits_and_renders_canonical() {
    let source = extension_json("")
        .replace("\"AQ==\"", "\"AB\"")
        .replace("\"AgM=\"", "\"AB==\"");
    let value = parsed(&source);
    let extension = only_extension(&value);
    assert_eq!(&*extension.bzl_transitive_digest, &[0]);
    assert_eq!(&*extension.usages_digest, &[0]);
    let rendered = render_lockfile_v28(&value).unwrap();
    assert_eq!(rendered.matches("\"AA==\"").count(), 2);
}

#[test]
fn lockfile_v28_invalid_base64_is_caught_syntax_failure() {
    let source = extension_json("").replace("\"AQ==\"", "\"!\"");
    assert_eq!(
        parse_error_for(&source).surface,
        LockfileParseErrorSurface::CaughtIllegalArgument
    );
}

#[test]
fn lockfile_v28_module_key_root_underscore_and_build_suffix() {
    let value = parsed(&object(
        ",\"selectedYankedVersions\":{\"<root>\":\"r\",\"m@_\":\"o\",\
         \"n@1.2+ignored\":\"y\"}",
    ));
    assert!(
        value
            .selected_yanked_versions
            .contains_key(&LockfileModuleKey::Root)
    );
    assert!(value.selected_yanked_versions.keys().any(
        |key| matches!(key, LockfileModuleKey::Module { version, .. } if version.normalized() == "1.2")
    ));
}

#[test]
fn lockfile_v28_module_key_uses_first_two_at_components() {
    let value = parsed(&object(
        ",\"selectedYankedVersions\":{\"m@1.0@ignored\":\"reason\"}",
    ));
    assert!(
        render_lockfile_v28(&value)
            .unwrap()
            .contains("\"m@1.0\": \"reason\"")
    );
}

#[test]
fn lockfile_v28_module_key_root_identity_and_component_order_match_bazel() {
    let root = parse_module_key("<root>".into()).unwrap();
    assert_eq!(root, parse_module_key("@_".into()).unwrap());
    assert_eq!(root, parse_module_key("@".into()).unwrap());
    let empty_name_version = parse_module_key("@1".into()).unwrap();
    assert!(empty_name_version < root);

    let value = parsed(&object(
        ",\"selectedYankedVersions\":{\"a@1\":\"a\",\"<root>\":\"root\",\"@1\":\"empty-name\"}",
    ));
    let rendered = render_lockfile_v28(&value).unwrap();
    assert!(
        rendered.find("\"@1\"").unwrap() < rendered.find("\"<root>\"").unwrap()
            && rendered.find("\"<root>\"").unwrap() < rendered.find("\"a@1\"").unwrap()
    );
}

#[test]
fn lockfile_v28_duplicate_normalized_map_keys_are_caught_syntax() {
    for source in [
        object(",\"selectedYankedVersions\":{\"<root>\":\"a\",\"@_\":\"b\"}"),
        object(",\"selectedYankedVersions\":{\"m@1+a\":\"a\",\"m@1+b\":\"b\"}"),
    ] {
        let error = parse_error_for(&source);
        assert_eq!(error.surface, LockfileParseErrorSurface::CaughtJsonSyntax);
        assert!(matches!(
            error.kind,
            LockfileParseErrorKind::DuplicateNormalizedMapKey
        ));
    }
}

#[test]
fn lockfile_v28_version_numeric_identifier_u64_bounds() {
    parsed(&object(&format!(
        ",\"selectedYankedVersions\":{{\"m@{}\":\"ok\"}}",
        u64::MAX
    )));
    let error = parse_error_for(&object(
        ",\"selectedYankedVersions\":{\"m@18446744073709551616\":\"bad\"}",
    ));
    assert_eq!(
        error.surface,
        LockfileParseErrorSurface::DirectAdapterJsonParse
    );
}

#[test]
fn lockfile_v28_version_and_module_key_order_match_bazel() {
    let value = parsed(&object(
        ",\"selectedYankedVersions\":{\"m@10\":\"ten\",\"m@2\":\"two\",\
         \"m@1\":\"release\",\"m@1-1\":\"pre-one\",\"m@1-01\":\"pre-zero-one\",\
         \"m@_\":\"empty\",\"<root>\":\"root\"}",
    ));
    let rendered = render_lockfile_v28(&value).unwrap();
    let ordered = [
        "\"<root>\"",
        "\"m@1-01\"",
        "\"m@1-1\"",
        "\"m@1\"",
        "\"m@2\"",
        "\"m@10\"",
        "\"m@_\"",
    ];
    let positions: Vec<_> = ordered
        .iter()
        .map(|needle| rendered.find(needle).unwrap())
        .collect();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn lockfile_v28_build_suffix_is_validated_before_discard() {
    assert_eq!(
        parse_error_for(&object(
            ",\"selectedYankedVersions\":{\"m@1.0+bad..build\":\"x\"}"
        ))
        .surface,
        LockfileParseErrorSurface::DirectAdapterJsonParse
    );
}

#[test]
fn lockfile_v28_complete_anchored_version_grammar_is_enforced() {
    for valid in [
        "1",
        "1.alpha.2",
        "1-a",
        "1-a-b.2",
        "1+build-1.2",
        "1-a+build-1.2",
    ] {
        parsed(&object(&format!(
            ",\"selectedYankedVersions\":{{\"m@{valid}\":\"ok\"}}"
        )));
    }
    for invalid in [
        "1-",
        "1+",
        "1-+build",
        "1_release",
        "1..2",
        "1-a..b",
        "1+a..b",
        "1+a+b",
    ] {
        assert_eq!(
            parse_error_for(&object(&format!(
                ",\"selectedYankedVersions\":{{\"m@{invalid}\":\"bad\"}}"
            )))
            .surface,
            LockfileParseErrorSurface::DirectAdapterJsonParse,
            "{invalid}"
        );
    }
}

#[test]
fn lockfile_v28_invalid_version_is_direct_adapter_hole() {
    assert_eq!(
        parse_error_for(&object(
            ",\"selectedYankedVersions\":{\"m@bad..version\":\"x\"}"
        ))
        .surface,
        LockfileParseErrorSurface::DirectAdapterJsonParse
    );
}

#[test]
fn lockfile_v28_extension_id_uses_first_three_percent_components() {
    let source =
        extension_json("").replace("\"//:ext.bzl%x\"", "\"//:ext.bzl%x%m@1+usage%ignored\"");
    let value = parsed(&source);
    assert!(
        render_lockfile_v28(&value)
            .unwrap()
            .contains("\"//:ext.bzl%x%m@1+usage\"")
    );
}

#[test]
fn lockfile_v28_isolation_uses_first_two_plus_components() {
    let source =
        extension_json("").replace("\"//:ext.bzl%x\"", "\"//:ext.bzl%x%m@1+usage+ignored\"");
    assert!(
        render_lockfile_v28(&parsed(&source))
            .unwrap()
            .contains("%m@1+usage\"")
    );
}

#[test]
fn lockfile_v28_missing_delimiters_are_index_holes() {
    let error = parse_error_for(&extension_json("").replace("//:ext.bzl%x", "//:ext.bzl"));
    assert_eq!(
        error.surface,
        LockfileParseErrorSurface::DelimiterIndexOutOfBounds
    );
}

#[test]
fn lockfile_v28_canonical_label_root_shorthand_is_adapter_exact() {
    let value = parsed(&extension_json(""));
    assert_eq!(
        value
            .module_extensions
            .keys()
            .next()
            .unwrap()
            .bzl_file
            .canonical,
        "//:ext.bzl"
    );
}

#[test]
fn lockfile_v28_canonical_label_accepted_forms_normalize_by_adapter() {
    for input in ["//:ext.bzl", "@@//:ext.bzl"] {
        let value = parsed(&extension_json("").replace("//:ext.bzl%x", &format!("{input}%x")));
        assert_eq!(
            value
                .module_extensions
                .keys()
                .next()
                .unwrap()
                .bzl_file
                .canonical,
            "//:ext.bzl"
        );
    }
    for input in ["@repo//pkg:ext.bzl", "@@repo//pkg:ext.bzl"] {
        let value = parsed(&extension_json("").replace("//:ext.bzl%x", &format!("{input}%x")));
        assert_eq!(
            value
                .module_extensions
                .keys()
                .next()
                .unwrap()
                .bzl_file
                .canonical,
            "@@repo//pkg:ext.bzl"
        );
    }
    let value = parsed(&extension_json("").replace("//:ext.bzl%x", "@repo%x"));
    assert_eq!(
        value
            .module_extensions
            .keys()
            .next()
            .unwrap()
            .bzl_file
            .canonical,
        "@@repo//:repo"
    );
}

#[test]
fn lockfile_v28_label_target_suffix_and_domain_error_surfaces_match_bazel() {
    let value = parsed(&extension_json("").replace("//:ext.bzl%x", "//pkg:target/.%x"));
    assert_eq!(
        value
            .module_extensions
            .keys()
            .next()
            .unwrap()
            .bzl_file
            .canonical,
        "//pkg:target/."
    );

    let repo_rule = extension_json("").replace(
        "\"generatedRepoSpecs\":{}",
        "\"generatedRepoSpecs\":{\"r\":{\"repoRuleId\":\"relative%rule\"}}",
    );
    assert_eq!(
        parse_error_for(&repo_rule).surface,
        LockfileParseErrorSurface::CaughtIllegalArgument
    );
    let attribute = extension_json("").replace(
        "\"generatedRepoSpecs\":{}",
        "\"generatedRepoSpecs\":{\"r\":{\"attributes\":{\"x\":\"@@bad repo//:x\"}}}",
    );
    assert_eq!(
        parse_error_for(&attribute).surface,
        LockfileParseErrorSurface::CaughtIllegalArgument
    );
}

#[test]
fn lockfile_v28_canonical_label_rejects_noncanonical_forms() {
    for invalid in [
        "relative%x",
        "@@repo/no-double-slash%x",
        "@@bad repo//:ext.bzl%x",
        "@@..//:ext.bzl%x",
        "//...:ext.bzl%x",
        "//pkg/../bad:ext.bzl%x",
        "//:bad:target%x",
        "//:../bad%x",
    ] {
        assert_eq!(
            parse_error_for(&extension_json("").replace("//:ext.bzl%x", invalid)).surface,
            LockfileParseErrorSurface::DirectAdapterJsonParse
        );
    }
}

#[test]
fn lockfile_v28_general_factor_is_empty() {
    let value = parsed(&extension_json(""));
    let factor = value
        .module_extensions
        .values()
        .next()
        .unwrap()
        .keys()
        .next()
        .unwrap();
    assert_eq!(factor.operating_system, None);
    assert_eq!(factor.architecture, None);
}

#[test]
fn lockfile_v28_factor_os_and_arch_are_last_wins() {
    let source = extension_json("").replace("\"general\"", "\"os:first,arch:a,os:last,arch:b\"");
    let value = parsed(&source);
    let factor = value
        .module_extensions
        .values()
        .next()
        .unwrap()
        .keys()
        .next()
        .unwrap();
    assert_eq!(factor.operating_system.as_deref(), Some("last"));
    assert_eq!(factor.architecture.as_deref(), Some("b"));
}

#[test]
fn lockfile_v28_factor_ignores_unknown_components() {
    let source = extension_json("").replace("\"general\"", "\"other:x,os:linux\"");
    assert!(
        render_lockfile_v28(&parsed(&source))
            .unwrap()
            .contains("\"os:linux\"")
    );
}

#[test]
fn lockfile_v28_factor_render_order_is_os_then_arch() {
    let source = extension_json("").replace("\"general\"", "\"arch:x,os:y\"");
    assert!(
        render_lockfile_v28(&parsed(&source))
            .unwrap()
            .contains("\"os:y,arch:x\"")
    );
}

#[test]
fn lockfile_v28_factor_map_order_matches_to_string_order() {
    let base = "\"bzlTransitiveDigest\":\"AQ==\",\"usagesDigest\":\"AgM=\",\
                \"recordedInputs\":[],\"generatedRepoSpecs\":{}";
    let source = object(
        ",\"moduleExtensions\":{\"//:ext.bzl%x\":{\
         \"os:x\":BASE,\"general\":BASE,\"arch:x\":BASE}}",
    )
    .replace("BASE", &format!("{{{base}}}"));
    let value = parsed(&source);
    let rendered = render_lockfile_v28(&value).unwrap();
    let arch = rendered.find("\"arch:x\"").unwrap();
    let general = rendered.find("\"general\"").unwrap();
    let os = rendered.find("\"os:x\"").unwrap();
    assert!(arch < general && general < os);
}

#[test]
fn lockfile_v28_label_component_order_drives_all_sorted_extension_maps() {
    let base = "\"bzlTransitiveDigest\":\"AQ==\",\"usagesDigest\":\"AgM=\",\
                \"recordedInputs\":[],\"generatedRepoSpecs\":{}";
    let source = object(
        ",\"moduleExtensions\":{\
           \"//a/b:a%x\":{\"general\":BASE},\"//a:b/c%x\":{\"general\":BASE}},\
         \"facts\":{\"//a/b:a%x\":{},\"//a:b/c%x\":{}},\
         \"factsVersions\":{\"//a/b:a%x\":1,\"//a:b/c%x\":1}",
    )
    .replace("BASE", &format!("{{{base}}}"));
    let rendered = render_lockfile_v28(&parsed(&source)).unwrap();
    let before = rendered
        .match_indices("\"//a:b/c%x\"")
        .map(|(index, _)| index);
    let after = rendered
        .match_indices("\"//a/b:a%x\"")
        .map(|(index, _)| index);
    let pairs: Vec<_> = before.zip(after).collect();
    assert_eq!(pairs.len(), 3);
    assert!(pairs.iter().all(|(before, after)| before < after));
}

#[test]
fn lockfile_v28_all_factor_entries_participate_in_equality() {
    let general = parsed(&extension_json(""));
    let platform = parsed(&extension_json("").replace("\"general\"", "\"os:linux\""));
    assert!(!general.semantically_eq(&platform));
}

#[test]
fn lockfile_v28_extension_first_four_properties_are_required() {
    for property in [
        "\"bzlTransitiveDigest\":\"AQ==\",",
        "\"usagesDigest\":\"AgM=\",",
        "\"recordedInputs\":[],",
        "\"generatedRepoSpecs\":{}",
    ] {
        let source = extension_json("").replace(property, "");
        assert!(matches!(
            parse_error_for(&source).kind,
            LockfileParseErrorKind::MissingRequiredProperty { .. }
        ));
    }
}

#[test]
fn lockfile_v28_extension_metadata_defaults_absent() {
    assert!(
        only_extension(&parsed(&extension_json("")))
            .metadata
            .is_none()
    );
}

#[test]
fn lockfile_v28_metadata_null_sets_differ_from_empty_sets() {
    let null = parsed(&extension_json(
        ",\"moduleExtensionMetadata\":{\"explicitRootModuleDirectDeps\":null,\
         \"useAllRepos\":\"NO\"}",
    ));
    let empty = parsed(&extension_json(
        ",\"moduleExtensionMetadata\":{\"explicitRootModuleDirectDeps\":[],\
         \"useAllRepos\":\"NO\"}",
    ));
    assert!(!null.semantically_eq(&empty));
}

#[test]
fn lockfile_v28_metadata_use_all_repos_enum_is_exact() {
    for spelling in ["NO", "REGULAR", "DEV"] {
        parsed(&extension_json(&format!(
            ",\"moduleExtensionMetadata\":{{\"useAllRepos\":\"{spelling}\"}}"
        )));
    }
    assert!(matches!(
        parse_error_for(&extension_json(
            ",\"moduleExtensionMetadata\":{\"useAllRepos\":\"OTHER\"}"
        ))
        .kind,
        LockfileParseErrorKind::MissingMetadataEnum
    ));
}

#[test]
fn lockfile_v28_metadata_missing_reproducible_defaults_false() {
    let value = parsed(&extension_json(
        ",\"moduleExtensionMetadata\":{\"useAllRepos\":\"NO\"}",
    ));
    assert!(
        !only_extension(&value)
            .metadata
            .as_ref()
            .unwrap()
            .reproducible
    );
}

#[test]
fn lockfile_v28_generated_repo_specs_retain_iteration_order() {
    let value = parsed(&extension_json(
        ",\"generatedRepoSpecs\":{\"z\":{},\"a\":{}}",
    ));
    let names: Vec<_> = only_extension(&value)
        .generated_repo_specs
        .keys()
        .map(CompactString::as_str)
        .collect();
    assert_eq!(names, ["z", "a"]);
}

#[test]
fn lockfile_v28_recorded_input_supports_all_five_kinds() {
    let source = extension_json("").replace(
        "\"recordedInputs\":[]",
        "\"recordedInputs\":[\"FILE:@@//a v\",\"DIRENTS:@@//b v\",\
         \"DIRTREE:@@//c v\",\"ENV:X v\",\"REPO_MAPPING:,x y\"]",
    );
    assert_eq!(only_extension(&parsed(&source)).recorded_inputs.len(), 5);
}

#[test]
fn lockfile_v28_directory_tree_excludes_use_java_form_encoding_and_split() {
    let source = extension_json("").replace(
        "\"recordedInputs\":[]",
        "\"recordedInputs\":[\
         \"DIRTREE:@@//dir?/../excludes=a+b,c%2Bd,, digest\"]",
    );
    let value = parsed(&source);
    let RecordedInputKey::DirectoryTree { excludes, .. } =
        &only_extension(&value).recorded_inputs[0].key
    else {
        panic!("expected directory tree");
    };
    assert_eq!(&**excludes, ["a b", "c+d"]);
    assert!(
        render_lockfile_v28(&value)
            .unwrap()
            .contains("DIRTREE:@@//dir?/../excludes=a+b,c%2Bd digest")
    );
}

#[test]
fn lockfile_v28_malformed_directory_tree_percent_is_caught_illegal_argument() {
    let source = extension_json("").replace(
        "\"recordedInputs\":[]",
        "\"recordedInputs\":[\"DIRTREE:@@//dir?/../excludes=%2 digest\"]",
    );
    assert_eq!(
        parse_error_for(&source).surface,
        LockfileParseErrorSurface::CaughtIllegalArgument
    );
}

#[test]
fn lockfile_v28_recorded_input_splits_at_first_space() {
    let source = extension_json("").replace(
        "\"recordedInputs\":[]",
        "\"recordedInputs\":[\"ENV:X value\\\\swith\\\\sspaces\"]",
    );
    assert_eq!(
        only_extension(&parsed(&source)).recorded_inputs[0]
            .value
            .as_deref(),
        Some("value with spaces")
    );
}

#[test]
fn lockfile_v28_recorded_input_unescapes_zero_slash_newline_space() {
    let source = extension_json("").replace(
        "\"recordedInputs\":[]",
        "\"recordedInputs\":[\"ENV:X \\\\0\",\"ENV:Y a\\\\\\\\b\\\\nc\\\\sd\"]",
    );
    let value = parsed(&source);
    assert_eq!(only_extension(&value).recorded_inputs[0].value, None);
    assert_eq!(
        only_extension(&value).recorded_inputs[1].value.as_deref(),
        Some("a\\b\nc d")
    );
}

#[test]
fn lockfile_v28_recorded_input_drops_unknown_escape_slash() {
    let source = extension_json("").replace(
        "\"recordedInputs\":[]",
        "\"recordedInputs\":[\"ENV:X a\\\\qb\"]",
    );
    assert_eq!(
        only_extension(&parsed(&source)).recorded_inputs[0]
            .value
            .as_deref(),
        Some("aqb")
    );
}

#[test]
fn lockfile_v28_recorded_input_drops_terminal_backslash() {
    let source = extension_json("").replace(
        "\"recordedInputs\":[]",
        "\"recordedInputs\":[\"ENV:X tail\\\\\\\\\"]",
    );
    assert_eq!(
        only_extension(&parsed(&source)).recorded_inputs[0]
            .value
            .as_deref(),
        Some("tail\\")
    );
}

#[test]
fn lockfile_v28_recorded_input_values_are_nullable() {
    let source = extension_json("").replace(
        "\"recordedInputs\":[]",
        "\"recordedInputs\":[\"ENV:X \\\\0\"]",
    );
    assert_eq!(
        only_extension(&parsed(&source)).recorded_inputs[0].value,
        None
    );
}

#[test]
fn lockfile_v28_recorded_repository_names_are_not_overvalidated() {
    let source = extension_json("").replace(
        "\"recordedInputs\":[]",
        "\"recordedInputs\":[\"REPO_MAPPING:odd+repo,name value\"]",
    );
    assert!(matches!(
        &only_extension(&parsed(&source)).recorded_inputs[0].key,
        RecordedInputKey::RepositoryMapping { source_repository, .. } if source_repository == "odd+repo"
    ));
}

#[test]
fn lockfile_v28_malformed_inputs_collapse_to_one_sentinel() {
    let source = extension_json("").replace(
        "\"recordedInputs\":[]",
        "\"recordedInputs\":[\"unknown\",\"OTHER:x value\"]",
    );
    let value = parsed(&source);
    assert_eq!(
        only_extension(&value).recorded_inputs[0],
        only_extension(&value).recorded_inputs[1]
    );
}

#[test]
fn lockfile_v28_parse_failure_sentinel_is_not_renderable() {
    let source =
        extension_json("").replace("\"recordedInputs\":[]", "\"recordedInputs\":[\"bad\"]");
    assert_eq!(
        render_lockfile_v28(&parsed(&source)).unwrap_err().kind,
        LockfileRenderErrorKind::RecordedInputParseFailureSentinel
    );
}

#[test]
fn lockfile_v28_repo_spec_fields_are_nullable() {
    let value = parsed(&extension_json(
        ",\"generatedRepoSpecs\":{\"r\":{\"repoRuleId\":null,\"attributes\":null}}",
    ));
    let spec = only_extension(&value)
        .generated_repo_specs
        .values()
        .next()
        .unwrap();
    assert!(spec.repo_rule_id.is_none() && spec.attributes.is_none());
}

#[test]
fn lockfile_v28_repo_rule_id_first_percent_delimits() {
    let value = parsed(&extension_json(
        ",\"generatedRepoSpecs\":{\"r\":{\"repoRuleId\":\"@@//:r.bzl%name%rest\"}}",
    ));
    let id = only_extension(&value)
        .generated_repo_specs
        .values()
        .next()
        .unwrap()
        .repo_rule_id
        .as_ref()
        .unwrap();
    assert_eq!(id.rule_name, "name%rest");
}

#[test]
fn lockfile_v28_repo_rule_id_without_percent_has_null_label() {
    let value = parsed(&extension_json(
        ",\"generatedRepoSpecs\":{\"r\":{\"repoRuleId\":\"rule\"}}",
    ));
    assert!(
        only_extension(&value)
            .generated_repo_specs
            .values()
            .next()
            .unwrap()
            .repo_rule_id
            .as_ref()
            .unwrap()
            .bzl_file
            .is_none()
    );
}

#[test]
fn lockfile_v28_repo_rule_id_null_label_is_not_renderable() {
    let value = parsed(&extension_json(
        ",\"generatedRepoSpecs\":{\"r\":{\"repoRuleId\":\"rule\"}}",
    ));
    assert_eq!(
        render_lockfile_v28(&value).unwrap_err().kind,
        LockfileRenderErrorKind::RepoRuleIdWithoutLabel
    );
}

#[test]
fn lockfile_v28_attribute_numbers_use_get_as_int_narrowing() {
    let value = parsed(&extension_json(
        ",\"generatedRepoSpecs\":{\"r\":{\"attributes\":{\"x\":2147483648}}}",
    ));
    let attributes = only_extension(&value)
        .generated_repo_specs
        .values()
        .next()
        .unwrap()
        .attributes
        .as_ref()
        .unwrap();
    assert!(matches!(
        attributes.values.get("x"),
        Some(AttributeValue::Int(i32::MIN))
    ));
}

#[test]
fn lockfile_v28_attribute_arbitrary_decimal_and_exponent_narrow_exactly() {
    let value = parsed(&extension_json(
        ",\"generatedRepoSpecs\":{\"r\":{\"attributes\":{\
         \"huge\":18446744073709551617,\"exponent\":4.294967297e9,\
         \"fraction\":4294967297.99}}}",
    ));
    let attributes = &only_extension(&value)
        .generated_repo_specs
        .values()
        .next()
        .unwrap()
        .attributes
        .as_ref()
        .unwrap()
        .values;
    for name in ["huge", "exponent", "fraction"] {
        assert!(matches!(attributes.get(name), Some(AttributeValue::Int(1))));
    }
}

#[test]
fn lockfile_v28_attribute_strings_keep_extra_quote_layer() {
    let value = parsed(&extension_json(
        ",\"generatedRepoSpecs\":{\"r\":{\"attributes\":{\"a\":\"''x''\",\"b\":\"''''\"}}}",
    ));
    let rendered = render_lockfile_v28(&value).unwrap();
    assert!(rendered.contains("\"a\": \"''x''\""));
    assert!(rendered.contains("\"b\": \"''''\""));
}

#[test]
fn lockfile_v28_attribute_labels_are_distinct_from_strings() {
    let value = parsed(&extension_json(
        ",\"generatedRepoSpecs\":{\"r\":{\"attributes\":{\"label\":\"@@//:x\",\
         \"string\":\"'@@//:x'\"}}}",
    ));
    let attrs = &only_extension(&value)
        .generated_repo_specs
        .values()
        .next()
        .unwrap()
        .attributes
        .as_ref()
        .unwrap()
        .values;
    assert!(matches!(attrs.get("label"), Some(AttributeValue::Label(_))));
    assert!(matches!(
        attrs.get("string"),
        Some(AttributeValue::String(_))
    ));
}

#[test]
fn lockfile_v28_attribute_sequences_are_ordered() {
    let a = parsed(&extension_json(
        ",\"generatedRepoSpecs\":{\"r\":{\"attributes\":{\"x\":[1,2]}}}",
    ));
    let b = parsed(&extension_json(
        ",\"generatedRepoSpecs\":{\"r\":{\"attributes\":{\"x\":[2,1]}}}",
    ));
    assert!(!a.semantically_eq(&b));
}

#[test]
fn lockfile_v28_lenient_arrays_inject_missing_attribute_nulls() {
    let value = parsed(&extension_json(
        ",\"generatedRepoSpecs\":{\"r\":{\"attributes\":{\
         \"trailing\":[1,],\"middle\":[1,,2],\"leading\":[,1]}}}",
    ));
    let attributes = &only_extension(&value)
        .generated_repo_specs
        .values()
        .next()
        .unwrap()
        .attributes
        .as_ref()
        .unwrap()
        .values;
    assert!(matches!(
        attributes.get("trailing"),
        Some(AttributeValue::Sequence(values))
            if matches!(values.as_ref(), [AttributeValue::Int(1), AttributeValue::None])
    ));
    assert!(matches!(
        attributes.get("middle"),
        Some(AttributeValue::Sequence(values))
            if matches!(
                values.as_ref(),
                [
                    AttributeValue::Int(1),
                    AttributeValue::None,
                    AttributeValue::Int(2)
                ]
            )
    ));
    assert!(matches!(
        attributes.get("leading"),
        Some(AttributeValue::Sequence(values))
            if matches!(values.as_ref(), [AttributeValue::None, AttributeValue::Int(1)])
    ));
}

#[test]
fn lockfile_v28_attribute_dicts_retain_iteration_order() {
    let value = parsed(&extension_json(
        ",\"generatedRepoSpecs\":{\"r\":{\"attributes\":{\"x\":{\"z\":1,\"a\":2}}}}",
    ));
    let rendered = render_lockfile_v28(&value).unwrap();
    assert!(rendered.find("\"z\": 1").unwrap() < rendered.find("\"a\": 2").unwrap());
}

#[test]
fn lockfile_v28_facts_require_object_root() {
    assert!(matches!(
        parse_error_for(&object(",\"facts\":{\"//:ext.bzl%x\":[]}")).kind,
        LockfileParseErrorKind::InvalidFacts
    ));
}

#[test]
fn lockfile_v28_facts_recursively_sort_every_dictionary() {
    let value = parsed(&object(
        ",\"facts\":{\"//:ext.bzl%x\":{\"z\":{\"z\":1,\"a\":2},\"a\":0}}",
    ));
    let rendered = render_lockfile_v28(&value).unwrap();
    assert!(rendered.find("\"a\": 0").unwrap() < rendered.find("\"z\": {").unwrap());
    assert!(rendered.rfind("\"a\": 2").unwrap() < rendered.rfind("\"z\": 1").unwrap());
}

#[test]
fn lockfile_v28_facts_depth_seven_passes_and_eight_fails() {
    let seven = format!("{}0{}", "[".repeat(6), "]".repeat(6));
    parsed(&object(&format!(
        ",\"facts\":{{\"//:ext.bzl%x\":{{\"x\":{seven}}}}}"
    )));
    let eight = format!("{}0{}", "[".repeat(7), "]".repeat(7));
    assert!(matches!(
        parse_error_for(&object(&format!(
            ",\"facts\":{{\"//:ext.bzl%x\":{{\"x\":{eight}}}}}"
        )))
        .kind,
        LockfileParseErrorKind::InvalidFacts
    ));
}

#[test]
fn lockfile_v28_facts_retain_arbitrary_integers() {
    let integer = "1234567890123456789012345678901234567890";
    let value = parsed(&object(&format!(
        ",\"facts\":{{\"//:ext.bzl%x\":{{\"x\":{integer}}}}}"
    )));
    assert!(render_lockfile_v28(&value).unwrap().contains(integer));
}

#[test]
fn lockfile_v28_facts_reject_non_finite_floats() {
    assert!(matches!(
        parse_error_for(&object(",\"facts\":{\"//:ext.bzl%x\":{\"x\":NaN}}")).kind,
        LockfileParseErrorKind::InvalidFacts
    ));
}

#[test]
fn lockfile_v28_facts_use_starlark_integer_float_equality() {
    let integer = parsed(&object(
        ",\"facts\":{\"//:ext.bzl%x\":{\"x\":9007199254740992}}",
    ));
    let float = parsed(&object(
        ",\"facts\":{\"//:ext.bzl%x\":{\"x\":9007199254740992.0}}",
    ));
    assert!(integer.semantically_eq(&float));
}

#[test]
fn lockfile_v28_facts_treat_positive_and_negative_zero_equal() {
    let positive = parsed(&object(",\"facts\":{\"//:ext.bzl%x\":{\"x\":0.0}}"));
    let negative = parsed(&object(",\"facts\":{\"//:ext.bzl%x\":{\"x\":-0.0}}"));
    assert!(positive.semantically_eq(&negative));
}

#[test]
fn lockfile_v28_lenient_arrays_inject_missing_fact_nulls() {
    let value = parsed(&object(
        ",\"facts\":{\"//:ext.bzl%x\":{\
         \"trailing\":[1,],\"middle\":[1,,2],\"leading\":[,1]}}",
    ));
    let rendered = render_lockfile_v28(&value).unwrap();
    assert!(rendered.contains("\"trailing\": [\n        1,\n        null\n"));
    assert!(rendered.contains("\"middle\": [\n        1,\n        null,\n        2\n"));
    assert!(rendered.contains("\"leading\": [\n        null,\n        1\n"));
}

#[test]
fn lockfile_v28_fact_float_render_matches_cleaned_java_percent_17g() {
    let value = parsed(&object(
        ",\"facts\":{\"//:ext.bzl%x\":{\
         \"ordinary\":1.1,\"fixed_low\":1e-4,\"scientific_low\":1e-5,\
         \"fixed_high\":1e16,\"scientific_high\":1e17,\"negative_zero\":-0.0}}",
    ));
    let rendered = render_lockfile_v28(&value).unwrap();
    assert!(rendered.contains("\"ordinary\": 1.1000000000000001"));
    assert!(rendered.contains("\"fixed_low\": 0.0001"));
    assert!(rendered.contains("\"scientific_low\": 1.0000000000000001e-05"));
    assert!(rendered.contains("\"fixed_high\": 10000000000000000.0"));
    assert!(rendered.contains("\"scientific_high\": 1e+17"));
    assert!(rendered.contains("\"negative_zero\": -0.0"));
}

#[test]
fn lockfile_v28_gson_render_escapes_unicode_line_separators() {
    let value = parsed(&object(
        ",\"facts\":{\"//:ext.bzl%x\":{\"x\":\"\u{2028}\u{2029}\"}}",
    ));
    let rendered = render_lockfile_v28(&value).unwrap();
    assert!(rendered.contains("\"x\": \"\\u2028\\u2029\""));
    assert!(!rendered.contains('\u{2028}'));
    assert!(!rendered.contains('\u{2029}'));
}

#[test]
fn lockfile_v28_explicit_empty_facts_are_retained() {
    let value = parsed(&object(",\"facts\":{\"//:ext.bzl%x\":{}}"));
    assert_eq!(value.facts.len(), 1);
}

#[test]
fn lockfile_v28_fact_versions_are_signed_i32() {
    let value = parsed(&object(",\"factsVersions\":{\"//:ext.bzl%x\":-2147483648}"));
    assert_eq!(*value.facts_versions.values().next().unwrap(), i32::MIN);
}

#[test]
fn lockfile_v28_explicit_zero_fact_version_is_retained() {
    let value = parsed(&object(",\"factsVersions\":{\"//:ext.bzl%x\":0}"));
    assert_eq!(value.facts_versions.len(), 1);
}

#[test]
fn lockfile_v28_map_and_set_equality_ignore_insertion_order() {
    let a = parsed(&extension_json(
        ",\"moduleExtensionMetadata\":{\"explicitRootModuleDirectDeps\":[\"a\",\"b\"],\
         \"useAllRepos\":\"NO\"},\"generatedRepoSpecs\":{\"a\":{},\"b\":{}}",
    ));
    let b = parsed(&extension_json(
        ",\"moduleExtensionMetadata\":{\"explicitRootModuleDirectDeps\":[\"b\",\"a\"],\
         \"useAllRepos\":\"NO\"},\"generatedRepoSpecs\":{\"b\":{},\"a\":{}}",
    ));
    assert!(a.semantically_eq(&b));
}

#[test]
fn lockfile_v28_list_equality_preserves_order() {
    let a = extension_json("").replace(
        "\"recordedInputs\":[]",
        "\"recordedInputs\":[\"ENV:A x\",\"ENV:B y\"]",
    );
    let b = extension_json("").replace(
        "\"recordedInputs\":[]",
        "\"recordedInputs\":[\"ENV:B y\",\"ENV:A x\"]",
    );
    assert!(!parsed(&a).semantically_eq(&parsed(&b)));
}

#[test]
fn lockfile_v28_render_preserves_only_semantic_producer_order() {
    let value = parsed(&extension_json(
        ",\"generatedRepoSpecs\":{\"z\":{},\"a\":{}}",
    ));
    let rendered = render_lockfile_v28(&value).unwrap();
    assert!(rendered.find("\"z\": {}").unwrap() < rendered.find("\"a\": {}").unwrap());
}

#[test]
fn lockfile_v28_caught_and_uncaught_error_classes_remain_distinct() {
    let malformed = parse_error_for("{\"lockFileVersion\":28");
    let checksum = parse_error_for(&object(",\"registryFileHashes\":{\"x\":\"bad\"}"));
    let delimiter = parse_error_for(&extension_json("").replace("//:ext.bzl%x", "//:ext.bzl"));
    assert_eq!(
        malformed.surface,
        LockfileParseErrorSurface::CaughtJsonSyntax
    );
    assert_eq!(
        checksum.surface,
        LockfileParseErrorSurface::DirectAdapterJsonParse
    );
    assert_eq!(
        delimiter.surface,
        LockfileParseErrorSurface::DelimiterIndexOutOfBounds
    );
}

#[test]
fn lockfile_v28_parse_render_parse_is_semantically_idempotent() {
    let a = parsed(&extension_json(
        ",\"moduleExtensionMetadata\":{\"explicitRootModuleDirectDeps\":[],\
         \"useAllRepos\":\"NO\"}",
    ));
    let b = parsed(&render_lockfile_v28(&a).unwrap());
    let c = parsed(&render_lockfile_v28(&b).unwrap());
    assert!(a.semantically_eq(&b));
    assert!(b.semantically_eq(&c));
}

#[test]
fn lockfile_v28_accepted_oracle_comprehensive_manifest_slice_renders_exactly() {
    // This is the discriminating extension/Facts slice of the accepted oracle
    // manifest. The hundreds of registry probe rows are intentionally reduced
    // to one hash and one not-found value so the source-derived assertion stays
    // reviewable while covering every retained semantic domain.
    let source = r#"{
      "lockFileVersion": 28,
      "registryFileHashes": {
        "sha": "SHA_UPPER",
        "missing": "not found"
      },
      "selectedYankedVersions": {
        "subject@1.0.0+discarded": "schema oracle"
      },
      "moduleExtensions": {
        "@@//:ext.bzl%schema": {
          "arch:amd64,os:linux": {
            "bzlTransitiveDigest": "AQ==",
            "usagesDigest": "AgM=",
            "recordedInputs": [
              "ENV:LOCKFILE_SCHEMA_ENV value",
              "FILE:@@//input.txt digest",
              "REPO_MAPPING:,subject subject+"
            ],
            "generatedRepoSpecs": {
              "alpha": {
                "repoRuleId": "//:ext.bzl%typed_repo",
                "attributes": {
                  "bool_value": true,
                  "dict_value": {"z": "last", "a": "first"},
                  "int_value": 4294967297,
                  "label_value": "@@subject+//:probe",
                  "list_value": ["z", "a"],
                  "message": "tagged"
                }
              }
            },
            "moduleExtensionMetadata": {
              "explicitRootModuleDirectDeps": ["alpha", "beta"],
              "explicitRootModuleDirectDevDeps": [],
              "useAllRepos": "NO",
              "reproducible": false
            }
          }
        }
      },
      "facts": {
        "@@//:ext.bzl%schema": {
          "z": {"nested": [{"b": 2, "a": 1}, true, null]},
          "a": "first"
        }
      },
      "factsVersions": {
        "@@//:ext.bzl%schema": 7
      }
    }"#
    .replace("SHA_UPPER", &"AB".repeat(32));
    let expected = r#"{
  "lockFileVersion": 28,
  "registryFileHashes": {
    "missing": "not found",
    "sha": "SHA_LOWER"
  },
  "selectedYankedVersions": {
    "subject@1.0.0": "schema oracle"
  },
  "moduleExtensions": {
    "//:ext.bzl%schema": {
      "os:linux,arch:amd64": {
        "bzlTransitiveDigest": "AQ==",
        "usagesDigest": "AgM=",
        "recordedInputs": [
          "ENV:LOCKFILE_SCHEMA_ENV value",
          "FILE:@@//input.txt digest",
          "REPO_MAPPING:,subject subject+"
        ],
        "generatedRepoSpecs": {
          "alpha": {
            "repoRuleId": "@@//:ext.bzl%typed_repo",
            "attributes": {
              "bool_value": true,
              "dict_value": {
                "z": "last",
                "a": "first"
              },
              "int_value": 1,
              "label_value": "@@subject+//:probe",
              "list_value": [
                "z",
                "a"
              ],
              "message": "tagged"
            }
          }
        },
        "moduleExtensionMetadata": {
          "explicitRootModuleDirectDeps": [
            "alpha",
            "beta"
          ],
          "explicitRootModuleDirectDevDeps": [],
          "useAllRepos": "NO",
          "reproducible": false
        }
      }
    }
  },
  "facts": {
    "//:ext.bzl%schema": {
      "a": "first",
      "z": {
        "nested": [
          {
            "a": 1,
            "b": 2
          },
          true,
          null
        ]
      }
    }
  },
  "factsVersions": {
    "//:ext.bzl%schema": 7
  }
}
"#
    .replace("SHA_LOWER", &"ab".repeat(32));
    let value = parsed(&source);
    assert_eq!(render_lockfile_v28(&value).unwrap(), expected);
    assert!(value.semantically_eq(&parsed(&expected)));
}

#[test]
fn lockfile_v28_deep_values_are_clone_and_allocative() {
    fn assert_clone_allocative<T: Clone + allocative::Allocative>() {}
    assert_clone_allocative::<BazelLockfileV28>();
    assert_clone_allocative::<FactValue>();
    assert_clone_allocative::<AttributeValue>();
    let _: Arc<[FactValue]> = Arc::from([]);
    let _: SmallMap<CompactString, AttributeValue> = SmallMap::new();
    let _: SmallSet<CompactString> = SmallSet::new();
    let _: SortedMap<CompactString, FactValue> = SortedMap::new();
}
