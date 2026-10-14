//! Directory expansion uses only the capability supplied by the consumer.
use std::cell::RefCell;
use std::fmt;

use allocative::Allocative;
use slug_build_api_v2::ArgsDirectoryContext;
use slug_build_api_v2::ArgsExpansionError;
use slug_loading_v2::subrule_invocation::AnalysisArtifactValue;
use starlark::any::ProvidesStaticType;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::list::AllocList;
use starlark::values::starlark_value;

/// Borrowed host state contains no interpreter values or ambient IO authority.
#[derive(ProvidesStaticType)]
pub(super) struct DirectoryRuntime<'a> {
    pub(super) context: &'a dyn ArgsDirectoryContext,
    pub(super) maximum_children: usize,
    pub(super) maximum_bytes: usize,
    pub(super) error: RefCell<Option<ArgsExpansionError>>,
}
impl DirectoryRuntime<'_> {
    fn fail<T>(&self, error: ArgsExpansionError) -> anyhow::Result<T> {
        if self.error.borrow().is_none() {
            *self.error.borrow_mut() = Some(error.clone());
        }
        Err(error.into())
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub(super) struct DirectoryExpander;
impl fmt::Display for DirectoryExpander {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DirectoryExpander")
    }
}
starlark_simple_value!(DirectoryExpander);
#[starlark_value(type = "DirectoryExpander")]
impl<'v> StarlarkValue<'v> for DirectoryExpander {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(directory_methods)
    }
}

#[starlark_module]
fn directory_methods(builder: &mut MethodsBuilder) {
    fn list<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] file: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        anyhow::ensure!(
            DirectoryExpander::from_value(this).is_some(),
            "invalid DirectoryExpander receiver"
        );
        let runtime = eval
            .extra
            .and_then(|extra| extra.downcast_ref::<DirectoryRuntime>())
            .ok_or_else(|| {
                anyhow::anyhow!("DirectoryExpander has no supplied directory capability")
            })?;
        let Some(file) = AnalysisArtifactValue::from_starlark(file) else {
            return runtime.fail(ArgsExpansionError::Callback(
                "DirectoryExpander.list requires a File".into(),
            ));
        };
        let children = match runtime.context.list(file.artifact()) {
            Ok(children) => children,
            Err(error) => return runtime.fail(error),
        };
        if children.len() > runtime.maximum_children {
            return runtime.fail(ArgsExpansionError::Limit("directory child count".into()));
        }
        let bytes = children
            .iter()
            .try_fold(0usize, |total, child| total.checked_add(child.path().len()));
        if bytes.is_none_or(|bytes| bytes > runtime.maximum_bytes) {
            return runtime.fail(ArgsExpansionError::Limit("directory child bytes".into()));
        }
        Ok(eval.heap().alloc(AllocList(
            children
                .into_iter()
                .map(|child| eval.heap().alloc_simple(AnalysisArtifactValue::new(child))),
        )))
    }
}
