//! Native rows describe actual wrappers and receiver/default edges; execution is unchanged.
use slug_build_api_v2::published_value::PublishedIntrinsic;

use super::*;

impl<'v> PublicationBuilder<'v> {
    pub(super) fn native_callable(
        &mut self,
        value: Value<'v>,
        lowerer: &mut AnalysisValueLowerer<'v>,
        depth: usize,
    ) -> Result<Option<PublishedNode>, String> {
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
        } else if let Some(api) = slug_loading_v2::cc_common_api(value) {
            let canonical = match api {
                slug_loading_v2::CcModuleApi::Public => "slug.loading.cc_common.public_module",
                slug_loading_v2::CcModuleApi::Internal => "slug.loading.cc_common.internal_module",
            };
            (
                slug_build_api_v2::NativeCallableApiId::new(canonical, 1)
                    .map_err(|e| e.to_string())?,
                Vec::new(),
                None,
            )
        } else if let Some((class, identity)) =
            slug_loading_v2::provider::native_provider_token(value)
        {
            let canonical = format!("slug.loading.provider_token.{class}.{}", identity.name());
            (
                slug_build_api_v2::NativeCallableApiId::new(&canonical, 1)
                    .map_err(|e| e.to_string())?,
                Vec::new(),
                None,
            )
        } else {
            return Ok(None);
        };
        Ok(Some(PublishedNode::Intrinsic(PublishedIntrinsic {
            api,
            parameters: parameters.into(),
            return_annotation: None,
            receiver: receiver.map(slug_build_api_v2::published_value::PublishedEdge::Local),
        })))
    }
}
