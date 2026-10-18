//! Literal vectors relocated from the removed BuildAPI recipe renderer/status APIs.
//! Product execution is always an actual frozen original; no recipe/type fallback.
use BuildInfoTransformKind::Info;
use BuildInfoTransformKind::Version;
use slug_build_api_v2::BuildInfoTransformKind;

use super::*;
fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
fn original(
    kind: BuildInfoTransformKind,
    captured: Option<&[(&str, &str)]>,
) -> StarlarkBuildInfoRuntime {
    let prefix = captured.map_or_else(String::new, |pairs| {
        format!(
            "captured = {{{}}}\n",
            pairs
                .iter()
                .map(|(k, v)| format!("{}:{}", quote(k), quote(v)))
                .collect::<Vec<_>>()
                .join(",")
        )
    });
    let assignment = if captured.is_some() {
        "    s = captured\n"
    } else {
        ""
    };
    let body = match kind {
        Info => {
            "    return {'{BUILD_EMBED_LABEL}':s.get('BUILD_EMBED_LABEL',''),'{BUILD_HOST}':s.get('BUILD_HOST','hostname'),'{BUILD_USER}':s.get('BUILD_USER','username')}\n"
        }
        Version => {
            "    if 'BUILD_TIMESTAMP' not in s: fail('timestamp information is missing from workspace status file, BUILD_TIMESTAMP key not found.')\n    return {'{BUILD_SCM_REVISION}':s.get('BUILD_SCM_REVISION','0'),'{BUILD_SCM_STATUS}':s.get('BUILD_SCM_STATUS',''),'{BUILD_TIMESTAMP}':s['BUILD_TIMESTAMP']}\n"
        }
    };
    runtime(&format!("{prefix}def transform(s):\n{assignment}{body}"))
}
fn missing(error: Error) {
    assert!(matches!(error, Error::Callback(_)));
    assert!(error.to_string().contains("timestamp information is missing from workspace status file, BUILD_TIMESTAMP key not found."),"{error}");
}
#[test]
fn legacy_full_default_string_table_and_missing_timestamp_matrix_use_originals() {
    let cases: &[(BuildInfoTransformKind, &[(&str, &str)], &str, &str)] = &[
        (
            Info,
            &[],
            "#define BUILD_EMBED_LABEL \"{BUILD_EMBED_LABEL}\"\n#define BUILD_HOST \"{BUILD_HOST}\"\n#define BUILD_USER \"{BUILD_USER}\"\n",
            "#define BUILD_EMBED_LABEL \"\"\n#define BUILD_HOST \"hostname\"\n#define BUILD_USER \"username\"\n",
        ),
        (
            Version,
            &[("BUILD_TIMESTAMP", "123")],
            "#define BUILD_SCM_REVISION \"{BUILD_SCM_REVISION}\"\n#define BUILD_SCM_STATUS \"{BUILD_SCM_STATUS}\"\n#define BUILD_TIMESTAMP {BUILD_TIMESTAMP}\n",
            "#define BUILD_SCM_REVISION \"0\"\n#define BUILD_SCM_STATUS \"\"\n#define BUILD_TIMESTAMP 123\n",
        ),
        (
            Info,
            &[
                ("BUILD_USER", ""),
                ("BUILD_HOST", ""),
                ("BUILD_EMBED_LABEL", ""),
                ("OTHER", "ignored"),
            ],
            "{BUILD_EMBED_LABEL}{BUILD_HOST}{BUILD_USER}|{OTHER}|{BUILD_TIMESTAMP}",
            "|{OTHER}|{BUILD_TIMESTAMP}",
        ),
        (
            Version,
            &[
                ("BUILD_TIMESTAMP", ""),
                ("BUILD_SCM_REVISION", ""),
                ("BUILD_SCM_STATUS", ""),
            ],
            "{BUILD_SCM_REVISION}{BUILD_SCM_STATUS}{BUILD_TIMESTAMP}|{BUILD_HOST}",
            "|{BUILD_HOST}",
        ),
        (
            Info,
            &[
                ("BUILD_USER", "u{BUILD_EMBED_LABEL}{BUILD_USER}"),
                ("BUILD_HOST", "{BUILD_USER}"),
                ("BUILD_EMBED_LABEL", "{BUILD_HOST}"),
            ],
            "{BUILD_EMBED_LABEL}|{BUILD_HOST}|{BUILD_USER}|{BUILD_EMBED_LABEL}",
            "u{BUILD_EMBED_LABEL}{BUILD_USER}|u{BUILD_EMBED_LABEL}{BUILD_USER}|u{BUILD_EMBED_LABEL}{BUILD_USER}|u{BUILD_EMBED_LABEL}{BUILD_USER}",
        ),
        (
            Version,
            &[
                ("BUILD_TIMESTAMP", "t{BUILD_SCM_REVISION}{BUILD_TIMESTAMP}"),
                ("BUILD_SCM_STATUS", "{BUILD_TIMESTAMP}"),
                ("BUILD_SCM_REVISION", "{BUILD_SCM_STATUS}"),
            ],
            "{BUILD_SCM_REVISION}|{BUILD_SCM_STATUS}|{BUILD_TIMESTAMP}|{BUILD_SCM_REVISION}",
            "t{BUILD_SCM_REVISION}{BUILD_TIMESTAMP}|t{BUILD_SCM_REVISION}{BUILD_TIMESTAMP}|t{BUILD_SCM_REVISION}{BUILD_TIMESTAMP}|t{BUILD_SCM_REVISION}{BUILD_TIMESTAMP}",
        ),
        (
            Info,
            &[("BUILD_EMBED_LABEL", "{BUILD_"), ("BUILD_HOST", "later")],
            "{BUILD_EMBED_LABEL}HOST}|{BUILD_EMBED_LABEL}EMBED_LABEL}",
            "later|{BUILD_EMBED_LABEL}",
        ),
        (
            Version,
            &[
                ("BUILD_SCM_REVISION", "{BUILD_"),
                ("BUILD_SCM_STATUS", "later"),
                ("BUILD_TIMESTAMP", "present"),
            ],
            "{BUILD_SCM_REVISION}SCM_STATUS}|{BUILD_SCM_REVISION}SCM_REVISION}",
            "later|{BUILD_SCM_REVISION}",
        ),
        (
            Info,
            &[
                ("BUILD_EMBED_LABEL", "{BUILD_EMBED_LABEL}"),
                ("BUILD_HOST", "{BUILD_HOST}"),
                ("BUILD_USER", "{BUILD_USER}"),
            ],
            "{BUILD_EMBED_LABEL}{BUILD_EMBED_LABEL}|{BUILD_HOST}|{BUILD_USER}",
            "{BUILD_EMBED_LABEL}{BUILD_EMBED_LABEL}|{BUILD_HOST}|{BUILD_USER}",
        ),
        (
            Version,
            &[
                ("BUILD_TIMESTAMP", "{BUILD_TIMESTAMP}"),
                ("BUILD_SCM_REVISION", "{BUILD_SCM_REVISION}"),
                ("BUILD_SCM_STATUS", "{BUILD_SCM_STATUS}"),
            ],
            "{BUILD_SCM_REVISION}|{BUILD_SCM_STATUS}|{BUILD_TIMESTAMP}{BUILD_TIMESTAMP}",
            "{BUILD_SCM_REVISION}|{BUILD_SCM_STATUS}|{BUILD_TIMESTAMP}{BUILD_TIMESTAMP}",
        ),
        (Info, &[], "plain {UNKNOWN}\n", "plain {UNKNOWN}\n"),
        (Info, &[], "", ""),
        (Version, &[("BUILD_TIMESTAMP", "")], "", ""),
        (
            Version,
            &[("BUILD_TIMESTAMP", " not-a-number \r\n")],
            "T={BUILD_TIMESTAMP};T={BUILD_TIMESTAMP}",
            "T= not-a-number \r\n;T= not-a-number \r\n",
        ),
    ];
    for &(kind, pairs, template, expected) in cases {
        // Captured original input retains the old direct-string API's embedded LF
        // values; supplied status bytes below separately test real parse semantics.
        let pairs_snapshot = pairs.to_vec();
        let template = template.to_owned();
        let snapshot = template.clone();
        let callback = original(kind, Some(pairs));
        for _ in 0..2 {
            assert_eq!(
                callback
                    .render(&[], template.as_bytes(), &Consumer::default())
                    .unwrap(),
                expected.as_bytes()
            );
        }
        assert_eq!(pairs, pairs_snapshot);
        assert_eq!(template, snapshot);
    }
    let callback = original(Version, None);
    for template in ["{BUILD_TIMESTAMP}", "", "no recognized placeholder"] {
        missing(
            callback
                .render(
                    b"BUILD_SCM_REVISION 9\nUNKNOWN x\n",
                    template.as_bytes(),
                    &Consumer::default(),
                )
                .unwrap_err(),
        );
    }
}
#[test]
fn legacy_supplied_full_header_raw_values_and_missing_matrix_use_originals() {
    let info_header=b"#define BUILD_EMBED_LABEL \"{BUILD_EMBED_LABEL}\"\n#define BUILD_HOST \"{BUILD_HOST}\"\n#define BUILD_USER \"{BUILD_USER}\"\n";
    let version_header=b"#define BUILD_SCM_REVISION \"{BUILD_SCM_REVISION}\"\n#define BUILD_SCM_STATUS \"{BUILD_SCM_STATUS}\"\n#define BUILD_TIMESTAMP {BUILD_TIMESTAMP}\n";
    let cases: &[(BuildInfoTransformKind, &[u8], &[u8], &[u8])] = &[
        (Info, b"", info_header, b"#define BUILD_EMBED_LABEL \"\"\n#define BUILD_HOST \"hostname\"\n#define BUILD_USER \"username\"\n"),
        (Version, b"BUILD_TIMESTAMP 123\n", version_header, b"#define BUILD_SCM_REVISION \"0\"\n#define BUILD_SCM_STATUS \"\"\n#define BUILD_TIMESTAMP 123\n"),
        (Info, b"BUILD_EMBED_LABEL caf\xE9\nBUILD_HOST \xC3\xA9\nBUILD_USER \xFF\x80\0\r\n", info_header, b"#define BUILD_EMBED_LABEL \"caf\xE9\"\n#define BUILD_HOST \"\xC3\xA9\"\n#define BUILD_USER \"\xFF\x80\0\r\"\n"),
        (Version, b"BUILD_SCM_REVISION r\xE9\nBUILD_SCM_STATUS \xFF\0\r\nBUILD_TIMESTAMP first\nBUILD_TIMESTAMP  not numeric \r\nBUILD_TIMESTAMP\n", version_header, b"#define BUILD_SCM_REVISION \"r\xE9\"\n#define BUILD_SCM_STATUS \"\xFF\0\r\"\n#define BUILD_TIMESTAMP  not numeric \r\n"),
        (Info, b"BUILD_EMBED_LABEL old\nBUILD_EMBED_LABEL \nBUILD_EMBED_LABEL\nBUILD_HOST \nBUILD_USER \n", b"[{BUILD_EMBED_LABEL}][{BUILD_HOST}][{BUILD_USER}]\xFE\0\r", b"[][][]\xFE\0\r"),
        (Version, b"BUILD_TIMESTAMP \n", version_header, b"#define BUILD_SCM_REVISION \"0\"\n#define BUILD_SCM_STATUS \"\"\n#define BUILD_TIMESTAMP \n"),
        (Version, b"BUILD_TIMESTAMP \n", b"", b""),
        (Info, b"", b"", b""),
    ];
    for &(kind, status, template, expected) in cases {
        let callback = original(kind, None);
        let status = status.to_vec();
        let template = template.to_vec();
        let snapshot = (status.clone(), template.clone());
        for _ in 0..2 {
            assert_eq!(
                callback
                    .render(&status, &template, &Consumer::default())
                    .unwrap(),
                expected
            );
        }
        assert_eq!((&status, &template), (&snapshot.0, &snapshot.1));
    }
    let callback = original(Version, None);
    for status in [b"".as_slice(), b"BUILD_TIMESTAMP\nBUILD_TIMESTAMP\t123\n"] {
        for template in [
            b"{BUILD_TIMESTAMP}".as_slice(),
            b"",
            b"no recognized key\xFF\0\r",
        ] {
            missing(
                callback
                    .render(status, template, &Consumer::default())
                    .unwrap_err(),
            );
        }
    }
}
#[test]
fn legacy_supplied_ordered_cross_boundary_and_all_byte_disposal_vectors_use_originals() {
    let cases: &[(BuildInfoTransformKind, &[u8], &[u8], &[u8])] = &[
        (Info, b"BUILD_EMBED_LABEL {BUILD_HOST}\nBUILD_HOST {BUILD_USER}\nBUILD_USER \xE9{BUILD_EMBED_LABEL}{BUILD_USER}\n", b"{BUILD_EMBED_LABEL}|{BUILD_HOST}|{BUILD_USER}", b"\xE9{BUILD_EMBED_LABEL}{BUILD_USER}|\xE9{BUILD_EMBED_LABEL}{BUILD_USER}|\xE9{BUILD_EMBED_LABEL}{BUILD_USER}"),
        (Version, b"BUILD_SCM_REVISION {BUILD_SCM_STATUS}\nBUILD_SCM_STATUS {BUILD_TIMESTAMP}\nBUILD_TIMESTAMP \xFF{BUILD_SCM_REVISION}{BUILD_TIMESTAMP}\n", b"{BUILD_SCM_REVISION}|{BUILD_SCM_STATUS}|{BUILD_TIMESTAMP}", b"\xFF{BUILD_SCM_REVISION}{BUILD_TIMESTAMP}|\xFF{BUILD_SCM_REVISION}{BUILD_TIMESTAMP}|\xFF{BUILD_SCM_REVISION}{BUILD_TIMESTAMP}"),
        (Info, b"BUILD_EMBED_LABEL {BUILD_\nBUILD_HOST later\x80\n", b"{BUILD_EMBED_LABEL}HOST}|{BUILD_EMBED_LABEL}EMBED_LABEL}", b"later\x80|{BUILD_EMBED_LABEL}"),
        (Version, b"BUILD_SCM_REVISION {BUILD_\nBUILD_SCM_STATUS later\xFE\nBUILD_TIMESTAMP \n", b"{BUILD_SCM_REVISION}SCM_STATUS}|{BUILD_SCM_REVISION}SCM_REVISION}", b"later\xFE|{BUILD_SCM_REVISION}"),
        (Info, b"BUILD_EMBED_LABEL {BUILD_EMBED_LABEL}\nBUILD_HOST {BUILD_HOST}\nBUILD_USER {BUILD_USER}\n", b"{BUILD_EMBED_LABEL}{BUILD_EMBED_LABEL}|{BUILD_HOST}|{BUILD_USER}|{OTHER}", b"{BUILD_EMBED_LABEL}{BUILD_EMBED_LABEL}|{BUILD_HOST}|{BUILD_USER}|{OTHER}"),
        (Version, b"BUILD_SCM_REVISION {BUILD_SCM_REVISION}\nBUILD_SCM_STATUS {BUILD_SCM_STATUS}\nBUILD_TIMESTAMP {BUILD_TIMESTAMP}\n", b"{BUILD_SCM_REVISION}|{BUILD_SCM_STATUS}|{BUILD_TIMESTAMP}{BUILD_TIMESTAMP}", b"{BUILD_SCM_REVISION}|{BUILD_SCM_STATUS}|{BUILD_TIMESTAMP}{BUILD_TIMESTAMP}"),
    ];
    for &(kind, status, template, expected) in cases {
        let callback = original(kind, None);
        for _ in 0..2 {
            assert_eq!(
                callback
                    .render(status, template, &Consumer::default())
                    .unwrap(),
                expected
            );
        }
    }
    let (matched, unchanged_info, unchanged_version) = {
        let info = original(Info, None);
        let version = original(Version, None);
        let mut status = b"BUILD_TIMESTAMP 1\nBUILD_HOST ".to_vec();
        status.extend(std::iter::repeat_n(0xE9, 512));
        status.extend_from_slice(b"\0\r\n");
        let template = b"\xFF[{BUILD_HOST}][{BUILD_HOST}]\0\r".to_vec();
        let all_bytes = (0..=255u8).collect::<Vec<_>>();
        let snapshots = (status.clone(), template.clone(), all_bytes.clone());
        let matched = info
            .render(&status, &template, &Consumer::default())
            .unwrap();
        let unchanged_info = info
            .render(&status, &all_bytes, &Consumer::default())
            .unwrap();
        let unchanged_version = version
            .render(&status, &all_bytes, &Consumer::default())
            .unwrap();
        assert_eq!(unchanged_info, all_bytes);
        assert_eq!(unchanged_version, all_bytes);
        assert_eq!(
            info.render(&status, &template, &Consumer::default())
                .unwrap(),
            matched
        );
        assert_eq!(
            (&status, &template, &all_bytes),
            (&snapshots.0, &snapshots.1, &snapshots.2)
        );
        drop(snapshots);
        drop(status);
        drop(template);
        drop(all_bytes);
        drop(info);
        drop(version);
        (matched, unchanged_info, unchanged_version)
    };
    let mut expected = b"\xFF[".to_vec();
    expected.extend(std::iter::repeat_n(0xE9, 512));
    expected.extend_from_slice(b"\0\r][");
    expected.extend(std::iter::repeat_n(0xE9, 512));
    expected.extend_from_slice(b"\0\r]\0\r");
    assert_eq!(matched, expected);
    assert_eq!(unchanged_info, (0..=255u8).collect::<Vec<_>>());
    assert_eq!(unchanged_version, (0..=255u8).collect::<Vec<_>>());
}

#[test]
fn legacy_unicode_captured_values_and_results_outlive_input_and_original_owners() {
    // Same old string inputs; byte-template UTF8 is literal Latin1 data, while
    // inserted Unicode is encoded scalar-wise only after all replacement passes.
    let (info, version, unchanged) = {
        let status = [
            ("BUILD_EMBED_LABEL", "  é😀\"\\\r\n\0  "),
            ("BUILD_HOST", "主机"),
            ("BUILD_USER", ""),
            ("BUILD_SCM_REVISION", "rév😀"),
            ("BUILD_SCM_STATUS", "\"\\\r\n\0"),
            ("BUILD_TIMESTAMP", " not numeric 😀 "),
        ];
        let i = original(Info, Some(&status));
        let v = original(Version, Some(&status));
        let template =
            "前[{BUILD_EMBED_LABEL}][{BUILD_EMBED_LABEL}]|{BUILD_HOST}|{BUILD_USER}|{OTHER}\n"
                .to_owned();
        let vt = "前{BUILD_SCM_REVISION}|{BUILD_SCM_STATUS}|{BUILD_TIMESTAMP}\n".to_owned();
        let ut = "unchanged é😀\0\r\n{OTHER}".to_owned();
        let snapshot = (status, template.clone(), vt.clone(), ut.clone());
        let info = i
            .render(&[], template.as_bytes(), &Consumer::default())
            .unwrap();
        let version = v.render(&[], vt.as_bytes(), &Consumer::default()).unwrap();
        let unchanged = i.render(&[], ut.as_bytes(), &Consumer::default()).unwrap();
        assert_eq!(
            info,
            b"\xE5\x89\x8D[  \xE9?\"\\\r\n\0  ][  \xE9?\"\\\r\n\0  ]|??||{OTHER}\n"
        );
        assert_eq!(version, b"\xE5\x89\x8Dr\xE9v?|\"\\\r\n\0| not numeric ? \n");
        assert_eq!(unchanged, ut.as_bytes());
        assert_eq!(
            i.render(&[], template.as_bytes(), &Consumer::default())
                .unwrap(),
            info
        );
        assert_eq!(
            v.render(&[], vt.as_bytes(), &Consumer::default()).unwrap(),
            version
        );
        assert_eq!(
            (&status, &template, &vt, &ut),
            (&snapshot.0, &snapshot.1, &snapshot.2, &snapshot.3)
        );
        let growing = "界😀".repeat(64);
        let growing_callback = original(Info, Some(&[("BUILD_HOST", growing.as_str())]));
        let grown = growing_callback
            .render(&[], b"{BUILD_HOST}{BUILD_HOST}", &Consumer::default())
            .unwrap();
        assert_eq!(grown, b"?".repeat(256));
        drop(growing_callback);
        drop(growing);
        drop(grown);
        drop(snapshot);
        drop(template);
        drop(vt);
        drop(ut);
        drop(i);
        drop(v);
        (info, version, unchanged)
    };
    assert_eq!(
        info,
        b"\xE5\x89\x8D[  \xE9?\"\\\r\n\0  ][  \xE9?\"\\\r\n\0  ]|??||{OTHER}\n"
    );
    assert_eq!(version, b"\xE5\x89\x8Dr\xE9v?|\"\\\r\n\0| not numeric ? \n");
    assert_eq!(unchanged, "unchanged é😀\0\r\n{OTHER}".as_bytes());
}
#[test]
fn legacy_status_decoder_literal_rows_and_ownership_matrix_use_frozen_inputs() {
    for maximum in [127u8, 255] {
        let bytes = (0..=maximum).collect::<Vec<_>>();
        let ledger = RefCell::new(Budget::default());
        let decoded = render::latin1(&bytes, &ledger, &|| Ok(())).unwrap();
        assert_eq!(decoded.chars().count(), usize::from(maximum) + 1);
        for (i, c) in decoded.chars().enumerate() {
            assert_eq!(u32::from(c), i as u32);
        }
    }
    for (raw, want) in [
        (b"".as_slice(), ""),
        (b"\xC3\xA9", "\u{00C3}\u{00A9}"),
        (b"\xFF\x80", "\u{00FF}\u{0080}"),
    ] {
        assert_eq!(
            render::latin1(raw, &RefCell::new(Budget::default()), &|| Ok(())).unwrap(),
            want
        );
    }
    let cases: &[(&[u8], &[(&str, &str)])] = &[
        (b"", &[]),
        (b"no-separator\n\t\r\n", &[]),
        (b" ", &[("", "")]),
        (
            b"\nseparatorless\nA first\nA  last \r\nA\nEMPTY \n empty first\n empty last\nTAB\tKEY v\t x\nCR\r value\r\nNUL\0KEY value\0\nLAST end",
            &[
                ("", "empty last"),
                ("A", " last \r"),
                ("CR\r", "value\r"),
                ("EMPTY", ""),
                ("LAST", "end"),
                ("NUL\0KEY", "value\0"),
                ("TAB\tKEY", "v\t x"),
            ],
        ),
        (
            b"UTF8 \xC3\xA9\nBAD \xFF\xFE\n\xFF \x80\0\r\n",
            &[
                ("BAD", "\u{00FF}\u{00FE}"),
                ("UTF8", "\u{00C3}\u{00A9}"),
                ("\u{00FF}", "\u{0080}\0\r"),
            ],
        ),
    ];
    for &(bytes, expected) in cases {
        let input = bytes.to_vec();
        let snapshot = input.clone();
        let module = Module::new();
        let ledger = RefCell::new(Budget::default());
        let value = render::input_dict(&input, &module, &ledger, &|| Ok(())).unwrap();
        let dict = starlark::values::dict::DictRef::from_value(value).unwrap();
        assert_eq!(
            dict.iter()
                .map(|(k, v)| (k.unpack_str().unwrap(), v.unpack_str().unwrap()))
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(input, snapshot);
        let again = render::input_dict(&input, &module, &ledger, &|| Ok(())).unwrap();
        assert_eq!(again.to_repr(), value.to_repr());
    }
    let retained = {
        let key = "LONG_KEY_".repeat(64);
        let mut input = key.as_bytes().to_vec();
        input.push(b' ');
        input.extend(std::iter::repeat_n(0xE9, 512));
        input.extend_from_slice(b"\0\r");
        let before = input.clone();
        let module = Module::new();
        let ledger = RefCell::new(Budget::default());
        let value = render::input_dict(&input, &module, &ledger, &|| Ok(())).unwrap();
        module.set("values", value);
        let owner = module.freeze().unwrap().get("values").unwrap();
        assert_eq!(input, before);
        drop(before);
        drop(input);
        drop(key);
        owner
    };
    let dict = starlark::values::dict::DictRef::from_value(retained.value()).unwrap();
    let mut pairs = dict.iter();
    let (key, value) = pairs.next().unwrap();
    assert!(pairs.next().is_none());
    assert_eq!(
        key.unpack_str().unwrap().as_bytes(),
        "LONG_KEY_".repeat(64).as_bytes()
    );
    assert_eq!(
        value.unpack_str().unwrap().as_bytes(),
        format!("{}\0\r", "é".repeat(512)).as_bytes()
    );
}
#[test]
fn legacy_status_parse_composes_with_actual_info_and_version_originals() {
    let info = original(Info, None);
    let version = original(Version, None);
    let info_template=b"#define BUILD_EMBED_LABEL \"{BUILD_EMBED_LABEL}\"\n#define BUILD_HOST \"{BUILD_HOST}\"\n#define BUILD_USER \"{BUILD_USER}\"\n";
    let version_template=b"#define BUILD_SCM_REVISION \"{BUILD_SCM_REVISION}\"\n#define BUILD_SCM_STATUS \"{BUILD_SCM_STATUS}\"\n#define BUILD_TIMESTAMP {BUILD_TIMESTAMP}\n";
    let raw=b"BUILD_EMBED_LABEL caf\xE9\nBUILD_HOST builder\r\nBUILD_USER user\nBUILD_SCM_REVISION abc\nBUILD_SCM_STATUS clean\nBUILD_TIMESTAMP 1\nBUILD_TIMESTAMP 2\r\nBUILD_TIMESTAMP\n";
    let snapshot = raw.to_vec();
    assert_eq!(info.render(raw,info_template,&Consumer::default()).unwrap(),b"#define BUILD_EMBED_LABEL \"caf\xE9\"\n#define BUILD_HOST \"builder\r\"\n#define BUILD_USER \"user\"\n");
    assert_eq!(version.render(raw,version_template,&Consumer::default()).unwrap(),b"#define BUILD_SCM_REVISION \"abc\"\n#define BUILD_SCM_STATUS \"clean\"\n#define BUILD_TIMESTAMP 2\r\n");
    assert_eq!(raw, &snapshot[..]);
    assert_eq!(version.render(b"BUILD_TIMESTAMP \n",version_template,&Consumer::default()).unwrap(),b"#define BUILD_SCM_REVISION \"0\"\n#define BUILD_SCM_STATUS \"\"\n#define BUILD_TIMESTAMP \n");
    for template in [
        version_template.as_slice(),
        b"",
        b"no timestamp placeholder",
    ] {
        missing(
            version
                .render(
                    b"BUILD_TIMESTAMP\nBUILD_TIMESTAMP\t123\n",
                    template,
                    &Consumer::default(),
                )
                .unwrap_err(),
        );
    }
}
