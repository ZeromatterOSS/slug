use std::path::PathBuf;
use std::sync::Arc;

use sha2::Digest;
use sha2::Sha256;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use starlark::environment::FrozenModule;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::eval::FileLoader;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::FrozenValue;
use starlark::values::Value;

use super::*;
use crate::package::loading_globals;
use crate::provider::BzlEvaluationContext;
use crate::subrule_invocation::AnalysisActionSink;
use crate::subrule_invocation::AnalysisArtifactValue;
use crate::subrule_invocation::AnalysisEvaluationContext;
use crate::subrule_invocation::AnalysisRunRequest;

#[derive(Debug)]
struct NoopActionSink;

impl AnalysisActionSink for NoopActionSink {
    fn declare_file(&self, _path: &str) -> anyhow::Result<AnalysisArtifactValue> {
        unreachable!("caller-authentication proof declares no actions")
    }

    fn write(
        &self,
        _output: Value<'_>,
        _content: Value<'_>,
        _is_executable: bool,
    ) -> anyhow::Result<()> {
        unreachable!("caller-authentication proof declares no actions")
    }

    fn run_shell(
        &self,
        _outputs: Value<'_>,
        _command: &str,
        _arguments: Value<'_>,
    ) -> anyhow::Result<()> {
        unreachable!("caller-authentication proof declares no actions")
    }

    fn run(&self, _request: AnalysisRunRequest<'_>) -> anyhow::Result<()> {
        unreachable!("caller-authentication proof declares no actions")
    }

    fn artifact_symlink(
        &self,
        _output: Value<'_>,
        _target_file: Value<'_>,
        _is_executable: bool,
        _progress_message: Option<&str>,
    ) -> anyhow::Result<()> {
        unreachable!("caller-authentication proof declares no actions")
    }

    fn absolute_symlink(
        &self,
        _output: Value<'_>,
        _target_path: &str,
        _progress_message: Option<&str>,
    ) -> anyhow::Result<()> {
        unreachable!("caller-authentication proof declares no actions")
    }
}

fn identity(repo: &str, package: &str, mapping: &[(&str, &str)]) -> BzlModuleIdentity {
    BzlModuleIdentity {
        label: CanonicalLabel::parse(&format!("@@{repo}//{package}:defs.bzl")).unwrap(),
        workspace_path: PathBuf::from(format!("/workspace/{package}/defs.bzl")),
        repository_mapping: mapping
            .iter()
            .map(|(apparent, canonical)| {
                (
                    ApparentRepoName::new(*apparent).unwrap(),
                    if canonical.is_empty() {
                        CanonicalRepoName::root()
                    } else {
                        CanonicalRepoName::new(*canonical).unwrap()
                    },
                )
            })
            .collect::<Vec<_>>()
            .into(),
    }
}

#[test]
fn pinned_allowlist_inventory_and_repository_branches_are_exact() {
    assert_eq!(
        INTERNAL_STARLARK_API_ALLOWLIST
            .iter()
            .filter(|entry| entry.apparent_repo.is_empty())
            .count(),
        18
    );
    assert_eq!(
        INTERNAL_STARLARK_API_ALLOWLIST
            .iter()
            .filter(|entry| !entry.apparent_repo.is_empty())
            .count(),
        11
    );

    for entry in INTERNAL_STARLARK_API_ALLOWLIST {
        let package = if entry.package_prefix.is_empty() {
            "child"
        } else {
            entry.package_prefix
        };
        let candidate = if entry.apparent_repo.is_empty() {
            identity("", package, &[])
        } else if entry.apparent_repo == "bazel_tools" {
            identity("bazel_tools", package, &[])
        } else {
            identity(&format!("{}+1.2.3", entry.apparent_repo), package, &[])
        };
        assert!(allows(&candidate), "{}/{package}", entry.apparent_repo);
    }

    assert!(allows(&identity("_builtins", "private", &[])));
    assert!(allows(&identity(
        "",
        "rust/private/toolchain",
        &[("rules_rust", "")]
    )));
    assert!(!allows(&identity("rules_cc_evil+1.0", "cc", &[])));
    assert!(!allows(&identity("rules_rust+1.0", "rust/public", &[])));
    assert!(allows(&identity(
        "bazel_tools+1.0",
        "tools/build_defs/build_info",
        &[]
    )));
    assert!(!allows(&identity(
        "bazel_tools_evil+1.0",
        "tools/build_defs/build_info",
        &[]
    )));
    assert!(!allows(&identity("", "tools/build_defs/cc_evil", &[])));
}

#[test]
fn caller_manifest_participates_structurally() {
    let first: Arc<[(CompactString, BzlModuleIdentity)]> =
        Arc::from([("defs.bzl".into(), identity("", "user", &[("rules_cc", "")]))]);
    let second: Arc<[(CompactString, BzlModuleIdentity)]> = Arc::from([(
        "defs.bzl".into(),
        identity("", "user", &[("rules_rust", "")]),
    )]);
    assert_ne!(first, second);
    assert!(allows(&first[0].1));
    assert!(!allows(&second[0].1));
    assert!(allows(&first[0].1));
}

struct OneModuleLoader {
    path: &'static str,
    module: FrozenModule,
}

impl FileLoader for OneModuleLoader {
    fn load(&self, path: &str) -> starlark::Result<FrozenModule> {
        if path == self.path {
            Ok(self.module.clone())
        } else {
            Err(starlark::Error::new_other(anyhow::anyhow!(
                "unexpected test load {path}"
            )))
        }
    }
}

fn freeze_restriction_source(
    filename: &str,
    source: &str,
    context: &BzlEvaluationContext,
    loader: Option<&dyn FileLoader>,
) -> FrozenModule {
    let ast = AstModule::parse(filename, source.to_owned(), &Dialect::Bazel).unwrap();
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(context);
    if let Some(loader) = loader {
        eval.set_loader(loader);
    }
    eval.eval_module(ast, &loading_globals()).unwrap();
    drop(eval);
    module.freeze().unwrap()
}

fn invoke_restriction_function(
    module: &FrozenModule,
    name: &str,
    context: &BzlEvaluationContext,
) -> Result<(), String> {
    let function = module.get(name).unwrap();
    let heap = Module::new();
    let mut eval = Evaluator::new(&heap);
    eval.extra = Some(context);
    eval.eval_function(function.value(), &[], &[])
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn invoke_configured_restriction_function(
    module: &FrozenModule,
    name: &str,
    context: &AnalysisEvaluationContext,
) -> Result<(), String> {
    let function = module.get(name).unwrap();
    let heap = Module::new();
    let mut eval = Evaluator::new(&heap);
    eval.extra = Some(context);
    eval.eval_function(function.value(), &[], &[])
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[test]
fn custom_private_api_checks_tuple_coercion_depth_and_caller_identity() {
    let allowed = identity("rules_cc+0.2.17", "cc/private", &[]);
    let denied = identity("consumer+1.0", "app", &[]);
    let identities: Arc<[(CompactString, BzlModuleIdentity)]> = Arc::from([
        ("allowed.bzl".into(), allowed.clone()),
        ("denied.bzl".into(), denied),
    ]);
    let context = BzlEvaluationContext::macro_runtime_context(allowed, identities);
    let allowed_module = freeze_restriction_source(
        "allowed.bzl",
        concat!(
            "_cc_internal = cc_common.internal_DO_NOT_USE()\n",
            "_allow = [(\"rules_cc\", \"cc/private\")]\n",
            "module_scope_ok = _cc_internal.check_private_api(allowlist = _allow, depth = 0)\n",
            "def checked_default():\n",
            "    return _cc_internal.check_private_api(allowlist = _allow)\n",
            "def checked_zero():\n",
            "    return _cc_internal.check_private_api(allowlist = _allow, depth = 0)\n",
            "def checked_two():\n",
            "    return _cc_internal.check_private_api(allowlist = _allow, depth = 2)\n",
            "def allowed_default():\n",
            "    return checked_default()\n",
            "def allowed_middle():\n",
            "    return checked_two()\n",
            "def allowed_outer():\n",
            "    return allowed_middle()\n",
            "def negative():\n",
            "    return _cc_internal.check_private_api(allowlist = _allow, depth = -1)\n",
            "def malformed():\n",
            "    return _cc_internal.check_private_api(allowlist = [(\"rules_cc\",)])\n",
        ),
        &context,
        None,
    );

    for function in ["checked_zero", "allowed_default", "allowed_outer"] {
        invoke_restriction_function(&allowed_module, function, &context).unwrap();
    }
    let configured_context = AnalysisEvaluationContext::new(
        Arc::from([]),
        std::iter::empty(),
        CanonicalLabel::parse("@@//:configured").unwrap(),
        Arc::new(NoopActionSink),
        FrozenValue::new_none(),
        context.source_identities_by_filename(),
    );
    invoke_configured_restriction_function(&allowed_module, "allowed_default", &configured_context)
        .unwrap();
    assert!(
        invoke_restriction_function(&allowed_module, "negative", &context)
            .unwrap_err()
            .contains("depth must be nonnegative")
    );
    assert!(
        invoke_restriction_function(&allowed_module, "malformed", &context)
            .unwrap_err()
            .contains("two-string tuples")
    );

    let loader = OneModuleLoader {
        path: ":allowed.bzl",
        module: allowed_module,
    };
    let denied_module = freeze_restriction_source(
        "denied.bzl",
        concat!(
            "load(\":allowed.bzl\", \"checked_default\")\n",
            "def denied_caller():\n",
            "    return checked_default()\n",
        ),
        &context,
        Some(&loader),
    );
    let error = invoke_restriction_function(&denied_module, "denied_caller", &context).unwrap_err();
    assert!(error.contains("@@consumer+1.0//app:defs.bzl"), "{error}");
    let error = invoke_configured_restriction_function(
        &denied_module,
        "denied_caller",
        &configured_context,
    )
    .unwrap_err();
    assert!(error.contains("@@consumer+1.0//app:defs.bzl"), "{error}");
}

#[test]
#[ignore = "requires the realized rules_cc 0.2.17 BCR source tree"]
fn authenticated_rules_cc_fdo_context_has_the_reviewed_fragment_call_ledger() {
    let source_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../target/v2o/ob/registry-yanked-lockfile-mode/bazel/external/rules_cc+/cc/private/rules_impl/fdo/fdo_context.bzl",
    );
    let source = std::fs::read(&source_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", source_path.display()));
    assert_eq!(
        format!("{:x}", Sha256::digest(&source)),
        "91b7b46c515b4773d5a241e699027212f679ab93160cc79218bd687eac51d5b7"
    );
    let source = std::str::from_utf8(&source).unwrap();
    let mut cursor = 0;
    for method in [
        "compilation_mode()",
        "propeller_optimize_absolute_cc_profile()",
        "propeller_optimize_absolute_ld_profile()",
        "fdo_path()",
        "cs_fdo_path()",
        "proto_profile()",
    ] {
        let offset = source[cursor..]
            .find(method)
            .unwrap_or_else(|| panic!("missing reviewed C++ fragment call {method}"));
        cursor += offset + method.len();
    }
    let first_action = source.find("ctx.actions.args()").unwrap();
    assert!(cursor < first_action);
}
