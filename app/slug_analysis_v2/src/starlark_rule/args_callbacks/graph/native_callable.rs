//! Native rows describe actual wrappers and receiver/default edges; execution is unchanged.
use slug_build_api_v2::CallableIntrinsic;

use super::*;

impl<'v> ArgsCallableGraphBuilder<'v> {
    pub(super) fn native_callable(
        &mut self,
        value: Value<'v>,
        lowerer: &mut AnalysisValueLowerer<'v>,
        depth: usize,
    ) -> Result<Option<CallableNode>, String> {
        let (api, parameters, receiver) = if let Some(native) = inspect_native_function(value) {
            (
                self.natives.certify(native)?,
                self.parameters(native.parameters(), lowerer, depth)?,
                None,
            )
        } else if let Some(method) = inspect_native_bound_method(value) {
            let api = NativeApis::certify_bound_method(method)?;
            let parameters = self.parameters(method.parameters(), lowerer, depth)?;
            let receiver = self.value(method.receiver(), lowerer, depth)?;
            (api, parameters, Some(receiver))
        } else if let Some(api) = slug_loading_v2::analysis_builtin_callable_api(value) {
            // No stored parameter metadata exists on this custom class.
            // This describes absence of instance state, not zero arity.
            (NativeApis::certify_analysis_builtin(api)?, Vec::new(), None)
        } else {
            return Ok(None);
        };
        Ok(Some(CallableNode::Intrinsic(CallableIntrinsic {
            api,
            parameters: parameters.into(),
            return_annotation: None,
            receiver,
        })))
    }
}
