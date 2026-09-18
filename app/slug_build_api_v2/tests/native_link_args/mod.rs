use slug_build_api_v2::RetainedRustNativeLinkArgs;
use slug_build_api_v2::RustNativeLinkArgMapper as Mapper;

use super::*;

fn file(path: &str) -> AnalysisValue {
    AnalysisValue::artifact(source_artifact(path))
}

fn library(slots: [Option<&str>; 4], always: bool) -> AnalysisValue {
    AnalysisValue::strukt([
        (
            "static_library",
            slots[0].map_or_else(AnalysisValue::none, file),
        ),
        (
            "pic_static_library",
            slots[1].map_or_else(AnalysisValue::none, file),
        ),
        (
            "interface_library",
            slots[2].map_or_else(AnalysisValue::none, file),
        ),
        (
            "dynamic_library",
            slots[3].map_or_else(AnalysisValue::none, file),
        ),
        ("alwayslink", AnalysisValue::boolean(always)),
    ])
}

fn row(
    libraries: Vec<AnalysisValue>,
    pic: bool,
    include: bool,
    ambiguous: &[(&str, &str)],
    unused: AnalysisValue,
) -> AnalysisValue {
    AnalysisValue::tuple(vec![
        AnalysisValue::strukt([
            ("libraries", AnalysisValue::list(libraries)),
            (
                "user_link_flags",
                AnalysisValue::tuple(vec![
                    AnalysisValue::string("-pthread"),
                    AnalysisValue::string("two words"),
                ]),
            ),
            ("unused", unused),
        ]),
        AnalysisValue::boolean(pic),
        AnalysisValue::dictionary(
            ambiguous
                .iter()
                .map(|(from, to)| (AnalysisValue::string(*from), file(to))),
        )
        .unwrap(),
        AnalysisValue::boolean(include),
    ])
}

fn recipe(rows: Vec<AnalysisValue>, mapper: Mapper) -> RetainedArgsRecipe {
    let mut options = default_vector_options();
    if mapper == Mapper::Directories {
        options.format_each = Some("-Lnative=%s".into());
        options.uniquify = true;
    }
    RetainedArgsRecipe::new(
        vec![RetainedArgCall::AddAll(RetainedVectorArg::new(
            RetainedVectorSource::RulesRustNativeLinks(
                RetainedRustNativeLinkArgs::new(rows, mapper).unwrap(),
            ),
            options,
        ))],
        RetainedParamFileFormat::Multiline,
    )
}

#[test]
fn preferred_library_flags_and_directory_order_follow_pinned_source() {
    let libraries = vec![
        library(
            [
                Some("s/libplain.a"),
                Some("p/libpic.pic.a"),
                Some("i/libiface.so"),
                Some("d/libdyn.so"),
            ],
            false,
        ),
        library(
            [None, None, Some("i/libapi.ifso"), Some("d/libdyn.so")],
            false,
        ),
        library([None, None, None, Some("d/libvers.so.1.20")], false),
        library([Some("lib/rustlib/libstd-hash.a"), None, None, None], false),
        library([Some("s/libalways.a"), None, None, None], true),
    ];
    let make = |pic, include| {
        row(
            libraries.clone(),
            pic,
            include,
            &[
                ("pkg/p/libpic.pic.a", "alias/librenamed.a"),
                ("pkg/s/libalways.a", "alias/libignored.a"),
            ],
            AnalysisValue::none(),
        )
    };
    assert_eq!(
        recipe(
            vec![make(true, true), make(true, true)],
            Mapper::Directories
        )
        .render(),
        [
            "-Lnative=pkg/p",
            "-Lnative=pkg/i",
            "-Lnative=pkg/d",
            "-Lnative=pkg/lib/rustlib",
            "-Lnative=pkg/s"
        ]
    );
    assert_eq!(
        recipe(vec![make(true, true)], Mapper::DefaultIndirect).render(),
        [
            "-lstatic=renamed",
            "-Clink-arg=-lrenamed",
            "-ldylib=api",
            "-ldylib=vers",
            "-lstatic=std-hash",
            "-Clink-arg=-Wl,--whole-archive",
            "-Clink-arg=pkg/s/libalways.a",
            "-Clink-arg=-Wl,--no-whole-archive",
            "--codegen=link-arg=-pthread",
            "--codegen=link-arg=two words"
        ]
    );
    let direct = recipe(vec![make(false, true)], Mapper::DefaultDirect).render();
    assert_eq!(&direct[..2], ["-lstatic=plain", "-Clink-arg=-lplain"]);
    assert!(direct.windows(3).any(|args| args
        == [
            "-Clink-arg=--whole-archive",
            "-Clink-arg=pkg/s/libalways.a",
            "-Clink-arg=--no-whole-archive"
        ]));
    assert_eq!(
        recipe(vec![make(true, false)], Mapper::DefaultDirect).render(),
        [
            "-Clink-arg=--whole-archive",
            "-Clink-arg=pkg/s/libalways.a",
            "-Clink-arg=--no-whole-archive",
            "--codegen=link-arg=-pthread",
            "--codegen=link-arg=two words"
        ]
    );
    let fallback = row(
        vec![library([None, Some("p/libonly.a"), None, None], false)],
        false,
        true,
        &[],
        AnalysisValue::none(),
    );
    assert_eq!(
        recipe(vec![fallback], Mapper::DefaultDirect).render()[0],
        "-lstatic=only"
    );
    for (path, name) in [
        ("libfoo.so.1.2", "foo"),
        ("foo.a", "foo"),
        ("libfoo.v2.a", "foo.v2"),
        ("libfoo.so.²", "foo.so"),
        ("no_extension", ""),
        ("libfoo..1", "foo"),
    ] {
        let args = recipe(
            vec![row(
                vec![library([None, None, None, Some(path)], false)],
                false,
                true,
                &[],
                AnalysisValue::none(),
            )],
            Mapper::DefaultDirect,
        )
        .render();
        assert_eq!(args[0], format!("-ldylib={name}"), "{path}");
    }
}

#[test]
fn native_link_rows_reject_invalid_shapes_and_keep_unused_identity() {
    let good = row(
        vec![library([Some("liba.a"), None, None, None], false)],
        false,
        true,
        &[],
        AnalysisValue::none(),
    );
    let slug_build_api_v2::AnalysisValueKind::Tuple(parts) = good.kind() else {
        unreachable!()
    };
    for (index, value) in [
        (1, AnalysisValue::string("false")),
        (3, AnalysisValue::string("true")),
        (2, AnalysisValue::list(vec![])),
    ] {
        let mut parts = parts.to_vec();
        parts[index] = value;
        assert!(
            RetainedRustNativeLinkArgs::new(
                vec![AnalysisValue::tuple(parts)],
                Mapper::DefaultDirect
            )
            .is_err()
        );
    }
    assert!(
        RetainedRustNativeLinkArgs::new(
            vec![AnalysisValue::list(parts.to_vec())],
            Mapper::Directories
        )
        .is_err()
    );
    assert!(
        RetainedRustNativeLinkArgs::new(
            vec![row(
                vec![library([None; 4], false)],
                false,
                true,
                &[],
                AnalysisValue::none()
            )],
            Mapper::Directories
        )
        .is_err()
    );
    for (field, value) in [
        (
            "static_library",
            AnalysisValue::artifact(derived_artifact("tree", ActionOutputKind::Directory)),
        ),
        ("dynamic_library", AnalysisValue::string("not a File")),
        ("alwayslink", AnalysisValue::string("false")),
    ] {
        let base = library([Some("liba.a"), None, None, None], false);
        let slug_build_api_v2::AnalysisValueKind::Struct(fields) = base.kind() else {
            unreachable!()
        };
        let invalid = AnalysisValue::strukt(fields.iter().map(|(name, old)| {
            (
                name.clone(),
                if name == field {
                    value.clone()
                } else {
                    old.clone()
                },
            )
        }));
        assert!(
            RetainedRustNativeLinkArgs::new(
                vec![row(vec![invalid], false, true, &[], AnalysisValue::none())],
                Mapper::DefaultDirect
            )
            .is_err()
        );
    }
    // Validation covers non-rendered fields too, so later branches cannot discover bad data.
    for (name, bad) in [
        (
            "user_link_flags",
            AnalysisValue::tuple(vec![AnalysisValue::boolean(false)]),
        ),
        ("libraries", AnalysisValue::string("not a sequence")),
    ] {
        let slug_build_api_v2::AnalysisValueKind::Struct(fields) = parts[0].kind() else {
            unreachable!()
        };
        let mut changed = parts.to_vec();
        changed[0] = AnalysisValue::strukt(fields.iter().map(|(field, old)| {
            (
                field.clone(),
                if field == name {
                    bad.clone()
                } else {
                    old.clone()
                },
            )
        }));
        assert!(
            RetainedRustNativeLinkArgs::new(
                vec![AnalysisValue::tuple(changed)],
                Mapper::Directories
            )
            .is_err()
        );
    }
    for entries in [
        vec![(AnalysisValue::boolean(true), file("liba.a"))],
        vec![(AnalysisValue::string("pkg/liba.a"), AnalysisValue::none())],
    ] {
        let mut changed = parts.to_vec();
        changed[2] = AnalysisValue::dictionary(entries).unwrap();
        assert!(
            RetainedRustNativeLinkArgs::new(
                vec![AnalysisValue::tuple(changed)],
                Mapper::DefaultDirect
            )
            .is_err()
        );
    }
    let other = row(
        vec![library([Some("liba.a"), None, None, None], false)],
        false,
        true,
        &[],
        AnalysisValue::string("unused change"),
    );
    let a = recipe(vec![good], Mapper::DefaultDirect);
    let b = recipe(vec![other], Mapper::DefaultDirect);
    assert_eq!(a.render(), b.render());
    assert_ne!(a, b);
}

#[test]
fn native_link_rows_preserve_shared_depset_publication_identity() {
    let make_depset =
        || AnalysisDepset::new(DepsetOrder::Default, vec![file("hidden")], Vec::new()).unwrap();
    let make_row = |deps| {
        row(
            vec![library([Some("liba.a"), None, None, None], false)],
            false,
            true,
            &[],
            AnalysisValue::depset(deps),
        )
    };
    let shared = make_depset();
    let a = recipe(
        vec![make_row(shared.clone()), make_row(shared)],
        Mapper::DefaultDirect,
    );
    let b = recipe(
        vec![make_row(make_depset()), make_row(make_depset())],
        Mapper::DefaultDirect,
    );
    assert_eq!(a.render(), b.render());
    assert_ne!(a, b);
    let shared = make_depset();
    assert_eq!(
        a,
        recipe(
            vec![make_row(shared.clone()), make_row(shared)],
            Mapper::DefaultDirect
        )
    );
    let spawn = |retained: AnalysisDepset, inputs: AnalysisDepset| {
        SpawnSpec::new(
            RetainedSpawnInvocation::Executable(SpawnExecutable::Path(
                NormalizedBazelPath::new(HostPathFlavor::Unix, "tool").unwrap(),
            )),
            RetainedCommandLine::new(vec![RetainedCommandLineSegment::ArgsSnapshot(
                RetainedSpawnArgsSnapshot::new(
                    recipe(vec![make_row(retained)], Mapper::DefaultDirect),
                    None,
                ),
            )]),
            ArtifactInputs::new(vec![ArtifactInputSource::Depset(
                RetainedArtifactInputs::new(inputs).unwrap(),
            )]),
            ArtifactInputs::new(Vec::new()),
            vec![ActionOutput::new("out", ActionOutputKind::File)],
            None,
            RetainedActionEnvironment::default(),
            CanonicalStringMap::default(),
            "Link",
            None::<&str>,
        )
    };
    let shared = make_depset();
    let aliased = spawn(shared.clone(), shared);
    let split = spawn(make_depset(), make_depset());
    assert_eq!(aliased.render_argv(), split.render_argv());
    assert_ne!(aliased, split);
}

#[test]
fn external_libraries_use_short_keys_and_execution_paths() {
    let external =
        AnalysisArtifact::Source(CanonicalLabel::parse("@@native+//lib:libsame.a").unwrap());
    let make = |key: &str, always| {
        let library = AnalysisValue::strukt([
            ("static_library", AnalysisValue::artifact(external.clone())),
            ("pic_static_library", AnalysisValue::none()),
            ("interface_library", AnalysisValue::none()),
            ("dynamic_library", AnalysisValue::none()),
            ("alwayslink", AnalysisValue::boolean(always)),
        ]);
        row(
            vec![library],
            false,
            true,
            &[(key, "alias/librenamed.a")],
            AnalysisValue::none(),
        )
    };
    let short_key = "../native+/lib/libsame.a";
    assert_eq!(
        recipe(vec![make(short_key, false)], Mapper::DefaultDirect).render(),
        [
            "-lstatic=renamed",
            "-Clink-arg=-lrenamed",
            "--codegen=link-arg=-pthread",
            "--codegen=link-arg=two words"
        ]
    );
    assert_eq!(
        recipe(
            vec![make("external/native+/lib/libsame.a", false)],
            Mapper::DefaultDirect
        )
        .render()[0],
        "-lstatic=same"
    );
    assert_eq!(
        recipe(vec![make(short_key, true)], Mapper::DefaultDirect).render()[..3],
        [
            "-Clink-arg=--whole-archive",
            "-Clink-arg=external/native+/lib/libsame.a",
            "-Clink-arg=--no-whole-archive"
        ]
    );
    assert_eq!(
        recipe(vec![make(short_key, false)], Mapper::Directories).render(),
        ["-Lnative=external/native+/lib"]
    );
}
