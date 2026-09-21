//! Authored negative callers of the authentic cc_internal export. These are
//! capability/lifetime boundary tests, not production toolchain construction.
use super::*;

const BOUNDARY_SUBJECT: &str = r#"
load('//cc/private:cc_internal.bzl', 'cc_internal')

def _inspect(subctx, parent, field):
    if field == 'exec_os':
        cc_internal.exec_os(parent)
    elif field == 'features':
        parent.features
    elif field == 'disabled_features':
        parent.disabled_features
    elif field == 'configuration':
        parent.configuration
    else:
        fail('unknown boundary field')

inspect = subrule(implementation = _inspect)

def _impl(ctx):
    if ctx.attr.mode == 'fake':
        cc_internal.exec_os(struct(features = [], configuration = struct(coverage_enabled = False)))
    elif ctx.attr.mode == 'keyword':
        cc_internal.exec_os(ctx = ctx)
    elif ctx.attr.mode == 'call_features':
        ctx.features()
    elif ctx.attr.mode == 'call_disabled_features':
        ctx.disabled_features()
    elif ctx.attr.mode == 'call_configuration':
        ctx.configuration()
    else:
        # A real root ctx is inactive while its declared subrule executes.
        inspect(ctx, ctx.attr.mode)
    fail('boundary operation unexpectedly succeeded')

subject = rule(implementation = _impl, subrules = [inspect], attrs = {'mode': attr.string()})
"#;

fn boundary_workspace(targets: &str) -> PathBuf {
    let root = workspace(targets);
    fs::write(root.join("rules_cc/cc/private/proof.bzl"), BOUNDARY_SUBJECT).unwrap();
    root
}

#[tokio::test]
async fn exec_os_rejects_fabricated_context_and_named_argument() {
    let root = boundary_workspace(
        "subject(name='fake',mode='fake')\nsubject(name='keyword',mode='keyword')",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let fake = request(&dice, &root, "fake", config(&[]))
        .await
        .unwrap_err();
    assert!(
        fake.contains("exec_os requires an active rule ctx"),
        "{fake}"
    );
    let keyword = request(&dice, &root, "keyword", config(&[]))
        .await
        .unwrap_err();
    assert!(keyword.contains("ctx"), "{keyword}");
    assert!(keyword.contains("extra named parameter"), "{keyword}");
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn cc_context_projections_reject_parent_context_during_subrule() {
    let fields = ["exec_os", "features", "disabled_features", "configuration"];
    let targets = fields
        .iter()
        .map(|field| format!("subject(name='{field}',mode='{field}')\n"))
        .collect::<String>();
    let root = boundary_workspace(&targets);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    for field in fields {
        let error = request(&dice, &root, field, config(&[])).await.unwrap_err();
        let diagnostic = if field == "exec_os" {
            "outside of its own implementation function"
        } else {
            "has no attribute"
        };
        assert!(error.contains(diagnostic), "{field}: {error}");
        assert!(error.contains(field), "{field}: {error}");
        assert!(
            !error.contains("unexpectedly succeeded"),
            "{field}: {error}"
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn cc_context_public_attributes_are_values_not_callable_getters() {
    let fields = ["features", "disabled_features", "configuration"];
    let targets = fields
        .iter()
        .map(|field| format!("subject(name='{field}',mode='call_{field}')\n"))
        .collect::<String>();
    let root = boundary_workspace(&targets);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    for field in fields {
        let error = request(&dice, &root, field, config(&[])).await.unwrap_err();
        assert!(
            error.contains("Operation `call()` not supported"),
            "{field}: {error}"
        );
        assert!(error.contains(field), "{field}: {error}");
        assert!(!error.contains("has no attribute"), "{field}: {error}");
        assert!(
            !error.contains("unexpectedly succeeded"),
            "{field}: {error}"
        );
    }
    fs::remove_dir_all(root).unwrap();
}
