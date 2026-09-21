use slug_identity_v2::CanonicalLabel;

use super::*;
use crate::AnalysisArtifact;

fn strings(values: &[&str]) -> Vec<CompactString> {
    values.iter().map(|s| (*s).into()).collect()
}
fn set(values: &[&str]) -> CcSet<CompactString> {
    strings(values).into_iter().collect()
}
fn feature(name: &str) -> CcFeature {
    CcFeature {
        name: name.into(),
        enabled: false,
        flag_sets: Box::new([]),
        env_sets: Box::new([]),
        requires: Box::new([]),
        implies: Box::new([]),
        provides: Box::new([]),
    }
}
fn action(name: &str) -> CcActionConfig {
    CcActionConfig {
        action_name: name.into(),
        enabled: false,
        tools: Box::new([]),
        flag_sets: Box::new([]),
        implies: Box::new([]),
    }
}
fn model(features: Vec<CcFeature>, actions: Vec<CcActionConfig>) -> CcToolchainFeatures {
    CcToolchainFeatures::new(features, actions, vec![], "tools", CcExecOs::Linux).unwrap()
}
fn group(flag: &str) -> CcFlagGroup {
    CcFlagGroup {
        expandables: vec![CcExpandable::Flag(parse_cc_string(flag).unwrap().into())].into(),
        iterate_over: None,
        expand_if_available: None,
        expand_if_not_available: None,
        expand_if_true: None,
        expand_if_false: None,
        expand_if_equal: None,
    }
}

#[test]
fn cc_features_defaults_requested_membership_and_occurrences() {
    let mut a = feature("a");
    a.enabled = true;
    let mut compile = action("compile");
    compile.enabled = true;
    let graph = model(vec![a.clone()], vec![compile.clone()]);
    assert_eq!(
        graph.default_features_and_action_configs(),
        strings(&["a", "compile"])
    );
    let empty = graph.configure_features(vec![]).unwrap();
    assert!(!empty.is_enabled("a"));
    let requested = graph
        .configure_features(strings(&["missing", "compile", "a", "missing"]))
        .unwrap();
    assert!(requested.is_requested("missing"));
    assert!(!requested.is_enabled("missing"));
    assert!(!requested.is_enabled("compile"));
    assert!(requested.is_enabled("a"));
    assert_eq!(
        requested.model().requested,
        set(&["missing", "compile", "a"])
    );
    assert_eq!(requested.model().enabled_action_configs, set(&["compile"]));
    assert_eq!(graph, graph.clone());
    let other = model(vec![a], vec![compile]);
    assert_ne!(graph, other);
    assert_eq!(graph.model(), other.model());
    let again = graph
        .configure_features(strings(&["missing", "compile", "a"]))
        .unwrap();
    assert_ne!(requested, again);
    assert_eq!(requested.model(), again.model());
    let reordered = graph
        .configure_features(strings(&["a", "compile", "missing"]))
        .unwrap();
    assert_ne!(requested.model(), reordered.model());
}

#[test]
fn cc_features_requires_or_of_and_does_not_enable_dependencies() {
    let mut a = feature("a");
    a.requires = vec![set(&["b", "c"]), set(&["d"])].into();
    let graph = model(vec![a, feature("b"), feature("c"), feature("d")], vec![]);
    for (request, enabled) in [
        (vec!["a"], false),
        (vec!["a", "b"], false),
        (vec!["a", "b", "c"], true),
        (vec!["a", "d"], true),
    ] {
        let config = graph.configure_features(strings(&request)).unwrap();
        assert_eq!(config.is_enabled("a"), enabled, "{request:?}");
        assert_eq!(config.is_enabled("d"), request.contains(&"d"));
    }
    let mut vacuous = feature("vacuous");
    vacuous.requires = vec![set(&[])].into();
    assert!(
        model(vec![vacuous], vec![])
            .configure_features(strings(&["vacuous"]))
            .unwrap()
            .is_enabled("vacuous")
    );
}

#[test]
fn cc_features_implication_pruning_cycles_and_action_edges() {
    let mut a = feature("a");
    a.implies = strings(&["b"]).into();
    let mut b = feature("b");
    b.implies = strings(&["a"]).into();
    let graph = model(vec![a.clone(), b.clone()], vec![]);
    let config = graph.configure_features(strings(&["a"])).unwrap();
    assert_eq!(config.model().enabled_features, set(&["a", "b"]));
    b.requires = vec![set(&["missing_requirement"])].into();
    let graph = model(vec![a, b, feature("missing_requirement")], vec![]);
    assert!(
        graph
            .configure_features(strings(&["a"]))
            .unwrap()
            .model()
            .enabled_features
            .is_empty()
    );

    let mut parent = feature("parent");
    parent.implies = strings(&["child", "unsupported"]).into();
    let mut unsupported = feature("unsupported");
    unsupported.requires = vec![set(&["required"])].into();
    let mut compile = action("compile");
    compile.implies = strings(&["parent"]).into();
    let graph = model(
        vec![parent, feature("child"), unsupported, feature("required")],
        vec![compile],
    );
    let config = graph.configure_features(strings(&["compile"])).unwrap();
    assert!(config.model().enabled_features.is_empty());
    assert!(config.model().enabled_action_configs.is_empty());
    // Independently requested child survives its parent's removal.
    assert_eq!(
        graph
            .configure_features(strings(&["compile", "child"]))
            .unwrap()
            .model()
            .enabled_features,
        set(&["child"])
    );
}

#[test]
fn cc_features_symbol_validation_and_provides_multiplicity_order() {
    let mut a = feature("a");
    a.implies = strings(&["unknown"]).into();
    assert!(
        CcToolchainFeatures::new(vec![a], vec![], vec![], "", CcExecOs::Linux)
            .unwrap_err()
            .to_string()
            .contains("not defined")
    );
    assert!(
        CcToolchainFeatures::new(
            vec![feature("a")],
            vec![action("a")],
            vec![],
            "",
            CcExecOs::Linux
        )
        .is_err()
    );
    assert!(
        CcToolchainFeatures::new(
            vec![],
            vec![action("a"), action("a")],
            vec![],
            "",
            CcExecOs::Linux
        )
        .unwrap_err()
        .to_string()
        .contains("multiple action configs")
    );
    for name in ["", "UPPER", "newline\n", "café", "space name"] {
        assert!(
            CcToolchainFeatures::new(vec![feature(name)], vec![], vec![], "", CcExecOs::Linux)
                .is_err()
        );
    }
    let mut nameless = feature("");
    nameless.enabled = true;
    assert_eq!(
        model(vec![nameless], vec![]).default_features_and_action_configs(),
        strings(&[""])
    );
    let mut twice = feature("twice");
    twice.provides = strings(&["x", "x"]).into();
    assert!(
        model(vec![twice], vec![])
            .configure_features(strings(&["twice"]))
            .unwrap_err()
            .to_string()
            .contains("twice twice")
    );
    let mut disabled = feature("disabled");
    disabled.provides = strings(&["x"]).into();
    let mut y1 = feature("y1");
    y1.provides = strings(&["y"]).into();
    let mut y2 = feature("y2");
    y2.provides = strings(&["y"]).into();
    let mut x1 = feature("x1");
    x1.provides = strings(&["x"]).into();
    let mut x2 = feature("x2");
    x2.provides = strings(&["x"]).into();
    let graph = model(vec![disabled, y1, y2, x1, x2], vec![]);
    assert!(
        graph
            .configure_features(strings(&["y1", "y2", "x1", "x2"]))
            .unwrap_err()
            .to_string()
            .contains("Symbol x is provided")
    );
}

#[test]
fn cc_features_string_chunks_preserve_native_boundaries_and_fail_closed() {
    use CcStringChunk::*;
    assert_eq!(
        parse_cc_string("abc%%def%{var}%%").unwrap(),
        vec![
            Literal("abc".into()),
            Literal("%def".into()),
            Variable("var".into()),
            Literal("%".into())
        ]
    );
    assert_eq!(
        parse_cc_string("é%{path:a/../b//c}😀").unwrap(),
        vec![
            Literal("é".into()),
            RelativePath("b/c".into()),
            Literal("😀".into())
        ]
    );
    assert!(parse_cc_string("").unwrap().is_empty());
    for value in [
        "%",
        "%x",
        "%{}",
        "%{unclosed",
        "%{path:}",
        "%{path:/abs}",
        "%{path:a\0b}",
    ] {
        assert!(parse_cc_string(value).is_err(), "{value:?}");
    }
}

#[test]
fn cc_features_tool_paths_origins_and_artifact_ownership_are_retained() {
    let tool = |path| {
        CcTool::new(
            Some(path),
            None,
            CcSet::new(),
            CcSet::new(),
            CcExecOs::Linux,
        )
    };
    assert_eq!(tool("a/../bin//cc").unwrap().path(), "bin/cc");
    assert_eq!(
        tool("bin/cc").unwrap().origin(),
        CcToolPathOrigin::CrosstoolPackage
    );
    assert_eq!(tool("/../usr/./bin/cc").unwrap().path(), "/usr/bin/cc");
    assert_eq!(
        tool("/usr/bin/cc").unwrap().origin(),
        CcToolPathOrigin::FilesystemRoot
    );
    assert_eq!(tool("../../cc").unwrap().path(), "../../cc");
    for path in ["", ".", "a/..", "bad\0path"] {
        assert!(tool(path).is_err());
    }
    assert!(CcExecOs::parse("WINDOWS").is_err());
    assert!(CcExecOs::parse("linux").is_err());
    for os in ["LINUX", "DARWIN", "FREEBSD", "OPENBSD"] {
        assert!(CcExecOs::parse(os).is_ok());
    }
    let artifact = AnalysisArtifact::Source(CanonicalLabel::parse("@@repo+//tools:cc").unwrap());
    let retained = CcTool::new(
        None,
        Some(artifact.clone()),
        CcSet::new(),
        CcSet::new(),
        CcExecOs::Linux,
    )
    .unwrap();
    assert_eq!(retained.origin(), CcToolPathOrigin::WorkspaceRoot);
    assert_eq!(retained.artifact(), Some(&artifact));
    assert_eq!(retained.path(), "external/repo+/tools/cc");
    assert!(
        CcTool::new(
            Some("cc"),
            Some(artifact),
            CcSet::new(),
            CcSet::new(),
            CcExecOs::Linux
        )
        .is_err()
    );
    assert!(CcTool::new(None, None, CcSet::new(), CcSet::new(), CcExecOs::Linux).is_err());
}

#[test]
fn cc_features_artifact_pattern_defaults_extensions_and_duplicate_overrides() {
    let default = CcArtifactNamePattern::new("static_library", "lib", ".a").unwrap();
    let override_ = CcArtifactNamePattern::new("static_library", "", ".lib").unwrap();
    let construct =
        |patterns| CcToolchainFeatures::new(vec![], vec![], patterns, "", CcExecOs::Linux);
    assert!(
        construct(vec![default.clone(), default.clone()])
            .unwrap()
            .model()
            .artifact_name_patterns
            .is_empty()
    );
    assert_eq!(
        construct(vec![default.clone(), override_.clone(), default])
            .unwrap()
            .model()
            .artifact_name_patterns
            .as_ref(),
        &[override_.clone()]
    );
    assert!(construct(vec![override_.clone(), override_]).is_err());
    assert!(CcArtifactNamePattern::new("unknown", "", "").is_err());
    assert!(CcArtifactNamePattern::new("object_file", "", ".exe").is_err());
    assert!(CcArtifactNamePattern::new("executable", "", "").is_ok());
    assert!(CcArtifactNamePattern::new("dynamic_library", "lib", ".wasm").is_ok());
}

#[test]
fn cc_features_eager_recursive_metadata_and_predicate_normalization() {
    let mut a = feature("a");
    let flags = CcFlagSet {
        actions: set(&["compile"]),
        with_features: CcSet::new(),
        flag_groups: vec![group("%{unused}")].into(),
    };
    a.flag_sets = vec![flags.clone()].into();
    let first = model(vec![a.clone()], vec![]);
    a.flag_sets[0].flag_groups[0].iterate_over = Some("items".into());
    let changed = model(vec![a.clone()], vec![]);
    assert_ne!(first.model(), changed.model());
    assert_eq!(
        first
            .configure_features(vec![])
            .unwrap()
            .model()
            .enabled_features,
        changed
            .configure_features(vec![])
            .unwrap()
            .model()
            .enabled_features
    );
    a.flag_sets[0].flag_groups[0].expandables = vec![
        CcExpandable::Flag(Box::new([])),
        CcExpandable::Group(group("x")),
    ]
    .into();
    assert!(CcToolchainFeatures::new(vec![a], vec![], vec![], "", CcExecOs::Linux).is_err());
    let mut empty_group = group("x");
    empty_group.expandables = Box::new([]);
    let mut a = feature("a");
    let mut flags = flags;
    flags.flag_groups = vec![empty_group].into();
    a.flag_sets = vec![flags].into();
    assert!(CcToolchainFeatures::new(vec![a], vec![], vec![], "", CcExecOs::Linux).is_err());
    let predicates = cc_with_features_set([
        CcWithFeatureSet {
            features: set(&["a", "b"]),
            not_features: set(&["c", "d"]),
        },
        CcWithFeatureSet {
            features: set(&["b", "a"]),
            not_features: set(&["d", "c"]),
        },
    ]);
    assert_eq!(predicates.len(), 1);
    assert_eq!(predicates.iter().next().unwrap().features, set(&["a", "b"]));
}

#[test]
fn cc_features_structural_hashes_include_order_and_nonselected_metadata() {
    fn hash(value: &impl Hash) -> u64 {
        let mut h = std::hash::DefaultHasher::new();
        value.hash(&mut h);
        h.finish()
    }
    let a = model(vec![feature("a")], vec![]);
    let b = model(vec![feature("a")], vec![]);
    assert_eq!(a.model(), b.model());
    assert_eq!(hash(a.model()), hash(b.model()));
    let request1 = a.configure_features(strings(&["x", "y"])).unwrap();
    let request2 = a.configure_features(strings(&["y", "x"])).unwrap();
    assert_ne!(hash(request1.model()), hash(request2.model()));
    let mut altered = feature("a");
    altered.provides = strings(&["unused"]).into();
    let altered = model(vec![altered], vec![]);
    assert_ne!(hash(a.model()), hash(altered.model()));
}
