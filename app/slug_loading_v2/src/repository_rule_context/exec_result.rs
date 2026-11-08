//! Immutable nominal result of a normal repository child exit.
use slug_bzlmod_v2::RepositoryExecuteResult;

use super::*;
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub(super) struct ExecResult {
    result: RepositoryExecuteResult,
}
impl ExecResult {
    pub(super) fn new(result: RepositoryExecuteResult) -> Self {
        Self { result }
    }
}
impl fmt::Display for ExecResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "exec_result(return_code={})", self.result.return_code())
    }
}
starlark::starlark_simple_value!(ExecResult);
#[starlark_value(type = "exec_result")]
impl<'v> StarlarkValue<'v> for ExecResult {
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match name {
            "return_code" => Some(heap.alloc(self.result.return_code())),
            "stdout" => Some(heap.alloc(latin1_text(self.result.stdout()))),
            "stderr" => Some(heap.alloc(latin1_text(self.result.stderr()))),
            _ => None,
        }
    }
    fn dir_attr(&self) -> Vec<String> {
        ["return_code", "stderr", "stdout"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    }
}
