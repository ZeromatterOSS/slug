use super::*;

fn attribute(name: &str, kind: AttributeKind) -> AttributeSchema {
    AttributeSchema::new(name, kind, false, true, None)
}

#[test]
fn validation_propagation_requires_an_explicit_ordinary_dependency() {
    for kind in [
        AttributeKind::Label,
        AttributeKind::LabelList,
        AttributeKind::StringKeyedLabelDict,
        AttributeKind::LabelKeyedStringDict,
        AttributeKind::LabelListDict,
    ] {
        assert!(attribute("dep", kind).propagates_validations(false));
        assert!(!attribute("_implicit", kind).propagates_validations(false));
        assert!(attribute("_late_bound", kind).propagates_validations(true));
        assert!(!attribute("$builtin", kind).propagates_validations(false));
    }
    for kind in [
        AttributeKind::Output,
        AttributeKind::OutputList,
        AttributeKind::String,
        AttributeKind::StringList,
        AttributeKind::StringListDict,
        AttributeKind::Boolean,
        AttributeKind::Integer,
        AttributeKind::IntegerList,
        AttributeKind::StringDict,
    ] {
        assert!(!attribute("value", kind).propagates_validations(false));
        assert!(!attribute("_late_bound", kind).propagates_validations(true));
    }
    assert!(
        !AttributeSchema::builtin(
            "nondependency",
            AttributeKind::Label,
            false,
            true,
            None,
            false,
            false
        )
        .propagates_validations(false)
    );
}

#[test]
fn validation_propagation_excludes_skip_flags_and_tool_transitions() {
    for flag in [
        AttributePropertyFlag::SkipValidations,
        AttributePropertyFlag::IsToolDependency,
    ] {
        let mut flags = AttributePropertyFlags::default();
        flags.insert(flag);
        assert!(
            !attribute("dep", AttributeKind::Label)
                .with_flags(flags)
                .propagates_validations(false)
        );
        assert!(
            !attribute("_late_bound", AttributeKind::Label)
                .with_flags(flags)
                .propagates_validations(true)
        );
    }
    for configuration in [
        AttributeDependencyConfiguration::Exec,
        AttributeDependencyConfiguration::ExecGroup("named".into()),
    ] {
        assert!(
            !attribute("dep", AttributeKind::Label)
                .with_dependency_configuration(configuration.clone(), false)
                .propagates_validations(false)
        );
        assert!(
            !attribute("_late_bound", AttributeKind::Label)
                .with_dependency_configuration(configuration, false)
                .propagates_validations(true)
        );
    }
    assert!(
        attribute("dep", AttributeKind::Label)
            .with_dependency_configuration(AttributeDependencyConfiguration::Target, true)
            .propagates_validations(false)
    );
    let mut flags = AttributePropertyFlags::default();
    flags.insert(AttributePropertyFlag::Executable);
    flags.insert(AttributePropertyFlag::SilentRuleclassFilter);
    assert!(
        attribute("dep", AttributeKind::Label)
            .with_flags(flags)
            .propagates_validations(false)
    );
}
