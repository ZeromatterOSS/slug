//! Declared rule admission for the genuine native executable environment.
use slug_loading_v2::RuleCapability;
use slug_loading_v2::TestRuleKind;

pub(super) fn validate_declaration(capability: Option<&RuleCapability>) -> Result<(), String> {
    // StarlarkRuleConfiguredTargetUtil.java291–302: declaration, not File presence.
    if capability.is_some_and(|capability| {
        capability.executable || capability.test_kind == Some(TestRuleKind::Test)
    }) {
        Ok(())
    } else {
        Err(
            "Returning RunEnvironmentInfo from a non-executable, non-test target has no effect"
                .to_owned(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn declared_executable_or_test_admits_but_suite_plain_and_missing_refuse() {
        for (executable, test_kind, admitted) in [
            (false, None, false),
            (true, None, true),
            (false, Some(TestRuleKind::Test), true),
            (true, Some(TestRuleKind::Test), true),
            (false, Some(TestRuleKind::Suite), false),
        ] {
            let capability = RuleCapability {
                rule_class: "unrelated".into(),
                executable,
                test_kind,
            };
            assert_eq!(validate_declaration(Some(&capability)).is_ok(), admitted);
        }
        assert!(validate_declaration(None).is_err());
    }
}
