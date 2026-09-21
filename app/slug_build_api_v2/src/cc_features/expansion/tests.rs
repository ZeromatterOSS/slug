//! Pure expansion discriminators from CcToolchainFeatures.java. The authored
//! adapter isolates expansion; Loading tests own native variable/cache semantics.
use slug_identity_v2::CanonicalLabel;

use super::*;
use crate::AnalysisArtifact;

#[derive(Clone)]
enum Val {
    Text(&'static str),
    Bool(bool),
    Sequence(Vec<Val>),
    None,
    Unsupported,
}
#[derive(Clone, Default)]
struct Vars(SmallMap<CompactString, Val>);
impl Vars {
    fn new(values: &[(&str, Val)]) -> Self {
        Self(
            values
                .iter()
                .map(|(k, v)| (CompactString::new(*k), v.clone()))
                .collect(),
        )
    }
}
impl CcVariableAccess for Vars {
    type Value = Val;
    type Sequence = std::iter::Map<std::vec::IntoIter<Val>, fn(Val) -> Result<Val, CcFeatureError>>;
    fn lookup(&mut self, name: &str, _: bool) -> Result<Option<Val>, CcFeatureError> {
        Ok(self
            .0
            .get(name)
            .filter(|v| !matches!(v, Val::None))
            .cloned())
    }
    fn string(&mut self, value: &Val, name: &str) -> Result<CompactString, CcFeatureError> {
        match value {
            Val::Text(value) => Ok((*value).into()),
            Val::Bool(value) => Ok(if *value { "1" } else { "0" }.into()),
            _ => Err(invalid(format!(
                "Cannot expand variable '{name}': expected string"
            ))),
        }
    }
    fn sequence(&mut self, value: &Val, name: &str) -> Result<Self::Sequence, CcFeatureError> {
        match value {
            Val::Sequence(values) => Ok(values.clone().into_iter().map(adapt_element)),
            _ => Err(invalid(format!(
                "Cannot expand variable '{name}': expected sequence"
            ))),
        }
    }
    fn truthy(&mut self, value: &Val) -> Result<bool, CcFeatureError> {
        Ok(match value {
            Val::Text(value) => !value.is_empty(),
            Val::Bool(value) => *value,
            Val::Sequence(value) => !value.is_empty(),
            Val::None => false,
            Val::Unsupported => return Err(invalid("unsupported sequence element")),
        })
    }
    fn with_binding(&self, name: &str, value: Val) -> Self {
        let mut result = self.clone();
        result.0.insert(name.into(), value);
        result
    }
}
fn adapt_element(value: Val) -> Result<Val, CcFeatureError> {
    if matches!(value, Val::Unsupported) {
        Err(invalid("unsupported sequence element"))
    } else {
        Ok(value)
    }
}
fn strings(values: &[&str]) -> Vec<CompactString> {
    values.iter().map(|value| (*value).into()).collect()
}
fn set(values: &[&str]) -> CcSet<CompactString> {
    strings(values).into_iter().collect()
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
fn flags(action: &str, groups: Vec<CcFlagGroup>) -> CcFlagSet {
    CcFlagSet {
        actions: set(&[action]),
        with_features: CcSet::new(),
        flag_groups: groups.into(),
    }
}
fn feature(name: &str, groups: Vec<CcFlagGroup>) -> CcFeature {
    CcFeature {
        name: name.into(),
        enabled: false,
        flag_sets: vec![flags("compile", groups)].into(),
        env_sets: Box::new([]),
        requires: Box::new([]),
        implies: Box::new([]),
        provides: Box::new([]),
    }
}
fn action(name: &str, tools: Vec<CcTool>, groups: Vec<CcFlagGroup>) -> CcActionConfig {
    CcActionConfig {
        action_name: name.into(),
        enabled: false,
        tools: tools.into(),
        flag_sets: vec![flags(name, groups)].into(),
        implies: Box::new([]),
    }
}
fn configure(
    features: Vec<CcFeature>,
    actions: Vec<CcActionConfig>,
    requested: &[&str],
) -> CcFeatureConfiguration {
    CcToolchainFeatures::new(features, actions, vec![], "toolchain", CcExecOs::Linux)
        .unwrap()
        .configure_features(strings(requested))
        .unwrap()
}
fn tool(path: &str, predicates: CcSet<CcWithFeatureSet>) -> CcTool {
    CcTool::new(
        Some(path),
        None,
        predicates,
        set(&["first", "second", "first"]),
        CcExecOs::Linux,
    )
    .unwrap()
}
fn predicate(yes: &[&str], no: &[&str]) -> CcWithFeatureSet {
    CcWithFeatureSet {
        features: set(yes),
        not_features: set(no),
    }
}

#[test]
fn command_expansion_orders_action_before_features_and_gates_only_action_flags() {
    let config = configure(
        vec![
            feature("a", vec![group("A")]),
            feature("b", vec![group("B")]),
        ],
        vec![action(
            "compile",
            vec![tool("cc", CcSet::new())],
            vec![group("ACTION")],
        )],
        &["b", "compile", "a"],
    );
    assert!(config.action_is_enabled("compile"));
    assert_eq!(
        config
            .get_command_line("compile", &mut Vars::default())
            .unwrap(),
        strings(&["ACTION", "A", "B"])
    );
    let disabled = configure(
        vec![feature("a", vec![group("A")])],
        vec![action(
            "compile",
            vec![tool("cc", CcSet::new())],
            vec![group("%{missing}")],
        )],
        &["a"],
    );
    assert!(!disabled.action_is_enabled("compile"));
    assert_eq!(
        disabled.get_tool_for_action("compile").unwrap(),
        "toolchain/cc"
    );
    assert_eq!(
        disabled
            .get_command_line("compile", &mut Vars::default())
            .unwrap(),
        strings(&["A"])
    );
    let no_action = configure(vec![feature("a", vec![group("A")])], vec![], &["a"]);
    assert_eq!(
        no_action
            .get_command_line("compile", &mut Vars::default())
            .unwrap(),
        strings(&["A"])
    );
    assert!(
        no_action
            .get_command_line("other", &mut Vars::default())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn tool_lookup_uses_first_matching_predicate_origins_and_ordered_requirements() {
    let tools = vec![
        tool("skip", [predicate(&["absent"], &[])].into_iter().collect()),
        tool(
            "../bin/./cc",
            [predicate(&["a"], &["b"]), predicate(&["other"], &[])]
                .into_iter()
                .collect(),
        ),
        tool("fallback", CcSet::new()),
    ];
    let config = configure(
        vec![feature("a", vec![])],
        vec![action("compile", tools, vec![])],
        &["a"],
    );
    assert_eq!(config.get_tool_for_action("compile").unwrap(), "bin/cc");
    assert_eq!(
        config.get_execution_requirements("compile").unwrap(),
        strings(&["first", "second"])
    );
    assert!(
        config
            .get_tool_for_action("missing")
            .unwrap_err()
            .to_string()
            .contains("does not have an enabled configuration")
    );
    let none = configure(
        vec![],
        vec![action(
            "compile",
            vec![tool(
                "cc",
                [predicate(&["absent"], &[])].into_iter().collect(),
            )],
            vec![],
        )],
        &[],
    );
    assert!(
        none.get_tool_for_action("compile")
            .unwrap_err()
            .to_string()
            .contains("Matching tool for action compile not found")
    );
    let artifact = AnalysisArtifact::Source(CanonicalLabel::parse("@@tools+//bin:cc").unwrap());
    let artifact_path = artifact.path().into_owned();
    let artifact_tool = CcTool::new(
        None,
        Some(artifact),
        CcSet::new(),
        CcSet::new(),
        CcExecOs::Linux,
    )
    .unwrap();
    let origins = configure(
        vec![],
        vec![
            action("absolute", vec![tool("/usr//bin/cc", CcSet::new())], vec![]),
            action("artifact", vec![artifact_tool], vec![]),
            action("empty_join", vec![tool("..", CcSet::new())], vec![]),
        ],
        &[],
    );
    assert_eq!(
        origins.get_tool_for_action("absolute").unwrap(),
        "/usr/bin/cc"
    );
    assert_eq!(
        origins.get_tool_for_action("artifact").unwrap(),
        artifact_path
    );
    assert_eq!(origins.get_tool_for_action("empty_join").unwrap(), ".");
}

#[test]
fn flag_predicates_are_or_of_and_features_not_action_names() {
    let mut f = feature("a", vec![group("KEEP")]);
    f.flag_sets[0].with_features = [predicate(&["absent"], &[]), predicate(&["a"], &["b"])]
        .into_iter()
        .collect();
    let mut blocked = flags("compile", vec![group("%{missing}")]);
    blocked.with_features = [predicate(&["compile"], &[])].into_iter().collect();
    f.flag_sets = vec![f.flag_sets[0].clone(), blocked].into();
    let config = configure(
        vec![f],
        vec![action("compile", vec![], vec![])],
        &["compile", "a"],
    );
    assert_eq!(
        config
            .get_command_line("compile", &mut Vars::default())
            .unwrap(),
        strings(&["KEEP"])
    );
}

#[test]
fn group_guards_precede_iteration_and_distinguish_missing_false_and_equal() {
    let mut available = group("available");
    available.expand_if_available = Some("present".into());
    let mut absent = group("absent");
    absent.expand_if_not_available = Some("none".into());
    let mut yes = group("true");
    yes.expand_if_true = Some("yes".into());
    let mut no = group("false");
    no.expand_if_false = Some("no".into());
    let mut equal = group("equal");
    equal.expand_if_equal = Some(CcVariableWithValue {
        name: "yes".into(),
        value: "1".into(),
    });
    let mut skip = group("%{unavailable}");
    skip.expand_if_available = Some("missing".into());
    skip.iterate_over = Some("also_missing".into());
    let mut missing_false = group("unexpected");
    missing_false.expand_if_false = Some("missing".into());
    let config = configure(
        vec![feature(
            "a",
            vec![available, absent, yes, no, equal, skip, missing_false],
        )],
        vec![],
        &["a"],
    );
    let mut vars = Vars::new(&[
        ("present", Val::Text("")),
        ("none", Val::None),
        ("yes", Val::Bool(true)),
        ("no", Val::Bool(false)),
    ]);
    assert_eq!(
        config.get_command_line("compile", &mut vars).unwrap(),
        strings(&["available", "absent", "true", "false", "equal"])
    );
    vars.0.insert("none".into(), Val::Text("exists"));
    vars.0.insert("yes".into(), Val::Bool(false));
    vars.0.insert("no".into(), Val::Bool(true));
    assert_eq!(
        config.get_command_line("compile", &mut vars).unwrap(),
        strings(&["available"])
    );
}

#[test]
fn nested_iteration_shadows_same_name_without_changing_parent_variables() {
    let mut inner = group("%{rows}:%{suffix}");
    inner.iterate_over = Some("rows".into());
    let mut outer = group("unused");
    outer.iterate_over = Some("rows".into());
    outer.expandables = vec![CcExpandable::Group(inner)].into();
    let config = configure(vec![feature("a", vec![outer])], vec![], &["a"]);
    let mut vars = Vars::new(&[
        ("suffix", Val::Text("S")),
        (
            "rows",
            Val::Sequence(vec![
                Val::Sequence(vec![Val::Text("a"), Val::Text("b")]),
                Val::Sequence(vec![]),
                Val::Sequence(vec![Val::Text("c")]),
            ]),
        ),
    ]);
    for _ in 0..2 {
        assert_eq!(
            config.get_command_line("compile", &mut vars).unwrap(),
            strings(&["a:S", "b:S", "c:S"])
        );
    }
    vars.0.insert("rows".into(), Val::Text("not-a-sequence"));
    assert!(
        config
            .get_command_line("compile", &mut vars)
            .unwrap_err()
            .to_string()
            .contains("expected sequence")
    );
}

#[test]
fn iteration_adapts_lazily_after_prior_expansion_but_before_child_guards() {
    let mut loop_group = group("%{missing}");
    loop_group.iterate_over = Some("items".into());
    let first = configure(vec![feature("a", vec![loop_group.clone()])], vec![], &["a"]);
    let mut vars = Vars::new(&[(
        "items",
        Val::Sequence(vec![Val::Text("first"), Val::Unsupported]),
    )]);
    assert!(
        first
            .get_command_line("compile", &mut vars)
            .unwrap_err()
            .to_string()
            .contains("Cannot find variable named 'missing'")
    );

    let mut skipped_child = group("%{items}");
    skipped_child.expand_if_available = Some("absent".into());
    loop_group.expandables = vec![CcExpandable::Group(skipped_child)].into();
    let skipped = configure(vec![feature("a", vec![loop_group])], vec![], &["a"]);
    // The second element still converts before its child guard could skip it.
    assert!(
        skipped
            .get_command_line("compile", &mut vars)
            .unwrap_err()
            .to_string()
            .contains("unsupported sequence element")
    );
}

#[test]
fn chunks_keep_empty_flags_percent_and_noop_relative_paths() {
    let config = configure(
        vec![feature(
            "a",
            vec![group(""), group("%%:%{path:a/../b}:%{yes}:%{no}")],
        )],
        vec![],
        &["a"],
    );
    let mut vars = Vars::new(&[("yes", Val::Bool(true)), ("no", Val::Bool(false))]);
    assert_eq!(
        config.get_command_line("compile", &mut vars).unwrap(),
        strings(&["", "%:b:1:0"])
    );
    vars.0.insert("yes".into(), Val::Sequence(vec![]));
    assert!(
        config
            .get_command_line("compile", &mut vars)
            .unwrap_err()
            .to_string()
            .contains("expected string")
    );
    let missing = configure(
        vec![feature("a", vec![group("%{missing}")])],
        vec![],
        &["a"],
    );
    assert!(
        missing
            .get_command_line("compile", &mut vars)
            .unwrap_err()
            .to_string()
            .contains("Cannot find variable named 'missing'")
    );
}

fn env(key: &str, value: &str) -> CcEnvEntry {
    CcEnvEntry {
        key: key.into(),
        value: parse_cc_string(value).unwrap().into(),
        expand_if_available: None,
    }
}
fn env_feature(entries: Vec<CcEnvEntry>) -> CcFeature {
    let mut f = feature("a", vec![]);
    f.env_sets = vec![CcEnvSet {
        actions: set(&["compile"]),
        env_entries: entries.into(),
        with_features: [predicate(&["a"], &["b"])].into_iter().collect(),
    }]
    .into();
    f
}

#[test]
fn environment_keeps_order_guards_and_rejects_even_equal_duplicate_keys() {
    let mut skip = env("SKIP", "%{missing}");
    skip.expand_if_available = Some("missing".into());
    let config = configure(
        vec![env_feature(vec![
            env("Z", "literal"),
            skip,
            env("A", "%{value}"),
        ])],
        vec![],
        &["a"],
    );
    let mut vars = Vars::new(&[("value", Val::Text("value"))]);
    assert_eq!(
        config
            .get_environment_variables("compile", &mut vars)
            .unwrap(),
        vec![("Z".into(), "literal".into()), ("A".into(), "value".into())]
    );
    assert!(
        config
            .get_environment_variables("other", &mut vars)
            .unwrap()
            .is_empty()
    );
    let duplicate = configure(
        vec![env_feature(vec![env("A", "same"), env("A", "same")])],
        vec![],
        &["a"],
    );
    assert!(
        duplicate
            .get_environment_variables("compile", &mut vars)
            .unwrap_err()
            .to_string()
            .contains("Multiple entries with same key")
    );
    let later_error = configure(
        vec![env_feature(vec![
            env("A", "same"),
            env("A", "same"),
            env("B", "%{missing}"),
        ])],
        vec![],
        &["a"],
    );
    assert!(
        later_error
            .get_environment_variables("compile", &mut vars)
            .unwrap_err()
            .to_string()
            .contains("Cannot find variable named 'missing'")
    );
}
