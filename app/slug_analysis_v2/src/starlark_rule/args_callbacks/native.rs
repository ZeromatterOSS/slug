//! Trusted native wrapper certificates describe actual values; never replace execution.
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::AnalysisValueKind;
use slug_build_api_v2::NativeCallableApiId;
use starlark::environment::Globals;
use starlark::environment::GlobalsBuilder;
use starlark::eval::NativeBoundMethodInspection;
use starlark::eval::NativeFunctionInspection;
use starlark::eval::inspect_native_bound_method;
use starlark::eval::inspect_native_function;
use starlark::values::ValueLike;

#[cfg(test)]
#[path = "native/label_budget_tests.rs"]
mod label_budget_tests;

pub(super) struct NativeApis {
    globals: Globals,
    loading_globals: Globals,
}
impl NativeApis {
    pub(super) fn new() -> Self {
        let mut builder = GlobalsBuilder::new();
        slug_starlark_v2::populate_universe(&mut builder);
        starlark::environment::LibraryExtension::StructType.add(&mut builder);
        Self {
            globals: builder.build(),
            loading_globals: slug_loading_v2::package::callable_registration_globals(),
        }
    }
    pub(super) fn certify_analysis_builtin(
        actual: slug_loading_v2::AnalysisBuiltinCallableApi,
    ) -> Result<NativeCallableApiId, String> {
        // Versioned identity of the complete fixed Rust class state. Never
        // replace invoke or invent a ParametersSpec for manual argument binding.
        let api = match actual {
            slug_loading_v2::AnalysisBuiltinCallableApi::Depset => {
                "slug.loading.analysis_builtin.depset"
            }
            slug_loading_v2::AnalysisBuiltinCallableApi::DefaultInfo => {
                "slug.loading.analysis_builtin.DefaultInfo"
            }
            slug_loading_v2::AnalysisBuiltinCallableApi::ToolchainInfo => {
                "slug.loading.analysis_builtin.ToolchainInfo"
            }
        };
        NativeCallableApiId::new(api, 1).map_err(|error| error.to_string())
    }
    pub(super) fn certify_bound_method(
        actual: NativeBoundMethodInspection<'_>,
    ) -> Result<NativeCallableApiId, String> {
        let receiver = actual.receiver();
        let class = if receiver.unpack_str().is_some() {
            "string"
        } else if starlark::values::list::ListRef::from_value(receiver).is_some() {
            "list"
        } else if starlark::values::dict::DictRef::from_value(receiver).is_some() {
            "dict"
        } else if let Some(module) = slug_loading_v2::cc_common_api(receiver) {
            match module {
                slug_loading_v2::CcModuleApi::Public => "cc_common",
                slug_loading_v2::CcModuleApi::Internal => "cc_internal",
            }
        } else if slug_loading_v2::cc_toolchain_features_ref(receiver).is_some() {
            "CcToolchainFeatures"
        } else if slug_loading_v2::cc_feature_configuration_ref(receiver).is_some() {
            "CcFeatureConfiguration"
        } else if slug_loading_v2::cc_variables_header(receiver).is_some() {
            "CcToolchainVariables"
        } else if slug_loading_v2::analysis_fragments::cpp_fragment_ref(receiver).is_some() {
            "CppFragment"
        } else if crate::package_specification::from_value_ref(receiver).is_some() {
            "PackageSpecificationInfo"
        } else if slug_loading_v2::dotted_version_ref(receiver).is_some() {
            "DottedVersion"
        } else {
            return Err(
                "bound native method receiver class has no complete typed certificate".into(),
            );
        };
        if !actual.matches_receiver_wrapper() {
            return Err("Args bound native method does not match its registered wrapper".into());
        }
        NativeCallableApiId::new(&format!("starlark.{class}.{}", actual.name()), 1)
            .map_err(|error| error.to_string())
    }
    pub(super) fn certify(
        &self,
        actual: NativeFunctionInspection<'_>,
    ) -> Result<NativeCallableApiId, String> {
        for (name, value) in self.globals.iter().chain(self.loading_globals.iter()) {
            let Some(canonical) = inspect_native_function(value.to_value()) else {
                continue;
            };
            if actual.matches_canonical_wrapper(canonical) {
                return NativeCallableApiId::new(&format!("starlark.universe.{name}"), 1)
                    .map_err(|e| e.to_string());
            }
        }
        Err("Args callback references an unsupported native callable wrapper".into())
    }
}

/// Borrow actual containers under the caller's shared work/byte budget before
/// the native lowerer snapshots any values. Completed aliases reuse this audit.
pub(super) fn pure_candidate<'v>(
    module: &'v starlark::environment::Module,
    graph: &mut slug_build_api_v2::published_value::PublishedValueArenaBuilder,
    remaining: &mut usize,
    value: starlark::values::Value<'v>,
    depth: usize,
) -> Result<bool, String> {
    use starlark::values::Value;
    use starlark::values::ValueIdentity;
    use starlark_map::unordered_map::UnorderedMap;
    struct Preflight<'a, 'v> {
        module: &'v starlark::environment::Module,
        graph: &'a mut slug_build_api_v2::published_value::PublishedValueArenaBuilder,
        remaining: &'a mut usize,
        seen: UnorderedMap<ValueIdentity<'v>, Option<bool>>,
    }
    impl<'v> Preflight<'_, 'v> {
        fn membership(&mut self, count: usize) -> Result<(), String> {
            if count > *self.remaining {
                return Err("Args borrowed container membership limit".into());
            }
            self.graph
                .charge_bytes(count.saturating_mul(std::mem::size_of::<Value<'v>>()))
                .map_err(|e| e.to_string())
        }
        fn label(&mut self, label: &slug_identity_v2::CanonicalLabel) -> Result<(), String> {
            self.graph
                .charge_bytes(
                    label
                        .package()
                        .repo()
                        .as_str()
                        .len()
                        .saturating_add(label.package().package().as_str().len())
                        .saturating_add(label.target().as_str().len())
                        .saturating_add(label.mapping_id().map_or(0, |id| id.as_str().len())),
                )
                .map_err(|e| e.to_string())
        }
        fn value(&mut self, value: Value<'v>, depth: usize) -> Result<bool, String> {
            self.graph.observe_depth(depth).map_err(|e| e.to_string())?;
            *self.remaining = self
                .remaining
                .checked_sub(1)
                .ok_or("Args borrowed native capture visit limit")?;
            if starlark::eval::inspect_def(value, Some(self.module)).is_some()
                || inspect_native_function(value).is_some()
                || inspect_native_bound_method(value).is_some()
                || slug_loading_v2::analysis_builtin_callable_api(value).is_some()
            {
                return Ok(false);
            }
            if let Some(specification) = crate::package_specification::from_value_ref(value) {
                self.graph
                    .audit_package_specification_at_depth(specification, depth)
                    .map_err(|error| error.to_string())?;
                return Ok(true);
            }
            if let Some(version) = slug_loading_v2::dotted_version_ref(value) {
                self.graph
                    .audit_dotted_version_at_depth(version, depth)
                    .map_err(|error| error.to_string())?;
                return Ok(true);
            }
            // These trusted wrappers clone only immutable native handles.
            // The native builder audits their full borrowed payload before row
            // retention; constructors cannot attach evaluator capabilities.
            if crate::analysis_value::published::runfiles_value(value).is_some()
                || slug_loading_v2::provider::run_environment_info_ref(value).is_some()
                || slug_loading_v2::cc_header_info_ref(value).is_some()
                || slug_loading_v2::template_variable_info_ref(value).is_some()
                || slug_loading_v2::cc_toolchain_features_ref(value).is_some()
                || slug_loading_v2::cc_feature_configuration_ref(value).is_some()
                || slug_loading_v2::analysis_fragments::cpp_fragment_ref(value).is_some()
            {
                self.graph
                    .charge_bytes(std::mem::size_of::<AnalysisValue>())
                    .map_err(|e| e.to_string())?;
                return Ok(true);
            }
            if let Some(pure) = self.seen.get(&value.identity()) {
                return Ok(pure.unwrap_or(false));
            }
            self.seen.insert(value.identity(), None);
            if let Some(integer) = value.downcast_ref::<starlark::values::StarlarkBigInt>() {
                let bound = integer
                    .magnitude_allocation_upper_bound()
                    .and_then(|n| n.checked_mul(3))
                    .ok_or("Args integer magnitude allocation limit")?;
                self.graph.charge_bytes(bound).map_err(|e| e.to_string())?;
            }

            let label = slug_loading_v2::provider::starlark_label_ref(value);
            if let Some(label) = label {
                self.label(label)?;
            }
            let file =
                slug_loading_v2::subrule_invocation::AnalysisArtifactValue::from_starlark(value);
            if let Some(file) = file {
                match file.artifact() {
                    slug_build_api_v2::AnalysisArtifact::Source(label) => self.label(label)?,
                    slug_build_api_v2::AnalysisArtifact::Derived { owner, output } => {
                        self.label(owner.label())?;
                        if let Some(platform) = owner.toolchain_execution_platform() {
                            self.label(platform)?;
                        }
                        self.graph
                            .charge_bytes(output.path().len())
                            .map_err(|e| e.to_string())?;
                    }
                    slug_build_api_v2::AnalysisArtifact::WorkspaceStatus(_) => {}
                }
            }
            let root = slug_loading_v2::artifact_root_value_ref(value);
            if let Some(slug_build_api_v2::AnalysisArtifactRoot::Source(repository)) = root {
                self.graph
                    .charge_bytes(repository.as_str().len())
                    .map_err(|e| e.to_string())?;
            }
            let mut pure = value.is_none()
                || value.unpack_bool().is_some()
                || value.unpack_i32().is_some()
                || value
                    .downcast_ref::<starlark::values::StarlarkBigInt>()
                    .is_some()
                || value
                    .downcast_ref::<starlark::values::float::StarlarkFloat>()
                    .is_some()
                || label.is_some()
                || file.is_some()
                || root.is_some();
            if let Some(text) = value.unpack_str() {
                pure = true;
                self.graph
                    .charge_bytes(text.len())
                    .map_err(|e| e.to_string())?;
            } else if let Some(list) = starlark::values::list::ListRef::from_value(value) {
                pure = true;
                self.membership(list.content().len())?;
                for child in list.iter() {
                    pure = self.value(child, depth + 1)? && pure;
                }
            } else if let Some(tuple) = starlark::values::tuple::TupleRef::from_value(value) {
                pure = true;
                self.membership(tuple.len())?;
                for child in tuple.iter() {
                    pure = self.value(child, depth + 1)? && pure;
                }
            } else if let Some(dict) = starlark::values::dict::DictRef::from_value(value) {
                pure = true;
                self.membership(dict.iter().len().saturating_mul(2))?;
                for (k, v) in dict.iter() {
                    pure = self.value(k, depth + 1)? && pure;
                    pure = self.value(v, depth + 1)? && pure;
                }
            } else if let Some(fields) = starlark::values::structs::StructRef::from_value(value) {
                pure = true;
                self.membership(fields.iter().len())?;
                for (name, v) in fields.iter() {
                    self.graph
                        .charge_bytes(name.as_str().len())
                        .map_err(|e| e.to_string())?;
                    pure = self.value(v, depth + 1)? && pure;
                }
            } else if slug_loading_v2::provider::StarlarkDepset::visit_metadata_from_value(
                value,
                &mut |element_type, count, _retained| {
                    if let Some(name) = element_type {
                        self.graph
                            .charge_bytes(name.len())
                            .map_err(|e| e.to_string())?;
                    }
                    self.membership(count)
                },
            )? {
                pure = true;
                slug_loading_v2::provider::StarlarkDepset::visit_successors_from_value(
                    value,
                    &mut |child| {
                        pure = self.value(child, depth + 1)? && pure;
                        Ok(())
                    },
                )?;
            } else if !pure {
                let mut fields_pure = true;
                let recognized = slug_loading_v2::provider::visit_starlark_provider_fields(
                    value,
                    &mut |name, child| {
                        self.graph
                            .charge_bytes(name.len().saturating_add(std::mem::size_of::<(
                                compact_str::CompactString,
                                Value<'v>,
                            )>(
                            )))
                            .map_err(|e| e.to_string())?;
                        fields_pure = self.value(child, depth + 1)? && fields_pure;
                        Ok(())
                    },
                )?;
                pure = recognized && fields_pure;
            }
            self.seen.insert(value.identity(), Some(pure));
            Ok(pure)
        }
    }
    Preflight {
        module,
        graph,
        remaining,
        seen: UnorderedMap::new(),
    }
    .value(value, depth)
}
