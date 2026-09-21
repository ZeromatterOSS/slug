use super::*;

fn make_model(patterns: Vec<CcArtifactNamePattern>) -> CcToolchainFeatures {
    CcToolchainFeatures::new(vec![], vec![], patterns, "tools", CcExecOs::Linux).unwrap()
}

#[test]
fn naming_uses_all_twenty_two_pinned_category_defaults_and_exact_enum_spelling() {
    let model = make_model(vec![]);
    for (category, expected) in [
        ("STATIC_LIBRARY", "libunit.a"),
        ("ALWAYSLINK_STATIC_LIBRARY", "libunit.lo"),
        ("DYNAMIC_LIBRARY", "libunit.so"),
        ("EXECUTABLE", "unit"),
        ("INTERFACE_LIBRARY", "libunit.ifso"),
        ("PIC_FILE", "unit.pic"),
        ("INCLUDED_FILE_LIST", "unit.d"),
        ("SERIALIZED_DIAGNOSTICS_FILE", "unit.dia"),
        ("OBJECT_FILE", "unit.o"),
        ("PIC_OBJECT_FILE", "unit.pic.o"),
        ("CPP_MODULE", "unit.pcm"),
        ("CPP_MODULES_INFO", "unit.CXXModules.json"),
        ("CPP_MODULES_DDI", "unit.ddi"),
        ("CPP_MODULES_MODMAP", "unit.modmap"),
        ("CPP_MODULES_MODMAP_INPUT", "unit.modmap.input"),
        ("GENERATED_ASSEMBLY", "unit.s"),
        ("PROCESSED_HEADER", "unit.processed"),
        ("GENERATED_HEADER", "unit.h"),
        ("PREPROCESSED_C_SOURCE", "unit.i"),
        ("PREPROCESSED_CPP_SOURCE", "unit.ii"),
        ("COVERAGE_DATA_FILE", "unit.gcno"),
        ("CLIF_OUTPUT_PROTO", "unit.opb"),
    ] {
        assert_eq!(
            model
                .artifact_name_for_category_posix(category, "nested/unit")
                .unwrap(),
            format!("nested/{expected}")
        );
        assert!(
            model
                .artifact_name_for_category_posix(&category.to_ascii_lowercase(), "unit")
                .is_err()
        );
    }
    for category in [
        "",
        "Object_File",
        "OBJECT_FILE ",
        "NOT_A_CATEGORY",
        "ÖBJECT_FILE",
    ] {
        assert!(
            model
                .artifact_name_for_category_posix(category, "unit")
                .is_err()
        );
    }
}

#[test]
fn naming_normalizes_before_parent_and_child_composition_without_extension_replacement() {
    let model = make_model(vec![
        CcArtifactNamePattern::new("object_file", "pre_", ".obj").unwrap(),
    ]);
    for (input, expected) in [
        ("unit.cc", "pre_unit.cc.obj"),
        ("dir//sub/.././unit/", "dir/pre_unit.obj"),
        ("../../dir/../unit", "../../pre_unit.obj"),
        ("/../../dir//unit", "/dir/pre_unit.obj"),
        ("/unit", "/pre_unit.obj"),
        (".hidden", "pre_.hidden.obj"),
        ("dir/..hidden", "dir/pre_..hidden.obj"),
        ("a/../../..", "../pre_...obj"),
        ("C:\\dir\\unit", "pre_C:\\dir\\unit.obj"),
        ("dir/λ", "dir/pre_λ.obj"),
    ] {
        assert_eq!(
            model
                .artifact_name_for_category_posix("OBJECT_FILE", input)
                .unwrap(),
            expected,
            "{input}"
        );
    }
    let unchanged = model
        .artifact_name_for_category_posix("INCLUDED_FILE_LIST", "dir/unit")
        .unwrap();
    assert_eq!(unchanged, "dir/unit.d");
}

#[test]
fn naming_rejects_parentless_fragments_and_invalid_composed_children() {
    let model = make_model(vec![]);
    for input in ["", ".", "./", "a/..", "/", "///", "/a/..", "a\0b"] {
        assert!(
            model
                .artifact_name_for_category_posix("OBJECT_FILE", input)
                .is_err(),
            "{input:?}"
        );
    }
    assert!(
        model
            .artifact_name_for_category_posix("EXECUTABLE", "..")
            .is_err()
    );
    assert_eq!(
        model
            .artifact_name_for_category_posix("OBJECT_FILE", "..")
            .unwrap(),
        "...o"
    );
    for prefix in ["bad/", "nul\0"] {
        let model = make_model(vec![
            CcArtifactNamePattern::new("object_file", prefix, ".o").unwrap(),
        ]);
        assert!(
            model
                .artifact_name_for_category_posix("OBJECT_FILE", "unit")
                .is_err()
        );
    }
    let model = make_model(vec![
        CcArtifactNamePattern::new("executable", ".", "").unwrap(),
    ]);
    assert!(
        model
            .artifact_name_for_category_posix("EXECUTABLE", ".")
            .is_err()
    );
    // A backslash in the pattern is not a POSIX separator either.
    let model = make_model(vec![
        CcArtifactNamePattern::new("object_file", "pre\\", ".o").unwrap(),
    ]);
    assert_eq!(
        model
            .artifact_name_for_category_posix("OBJECT_FILE", "unit")
            .unwrap(),
        "pre\\unit.o"
    );
}
