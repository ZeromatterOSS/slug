/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use kuro_core::pattern::pattern::ParsedPattern;
use kuro_core::pattern::pattern_type::TargetPatternExtra;
use kuro_core::provider::label::NonDefaultProvidersName;
use kuro_core::provider::label::ProvidersLabel;
use kuro_core::provider::label::ProvidersName;
use kuro_core::target::label::label::TargetLabel;
use kuro_query::query::syntax::simple::functions::QueryLiteralVisitor;
use kuro_query_parser::Expr;
use kuro_query_parser::spanned::Spanned;
use kuro_util::arc_str::ArcSlice;
use kuro_util::arc_str::ArcStr;

use super::coerced_attr::CoercedAttr;
use crate::attrs::coerced_path::CoercedPath;
use crate::configuration::resolved::ConfigurationSettingKey;

#[derive(kuro_error::Error, Debug)]
#[kuro(tag = Input)]
enum AttrCoercionContextError {
    #[error("Expected target label without name. Got `{0}`")]
    UnexpectedProvidersName(String),
}

/// The context for attribute coercion. Mostly just contains information about
/// the current package (to support things like parsing targets from strings).
pub trait AttrCoercionContext {
    fn coerce_target_label(&self, value: &str) -> kuro_error::Result<TargetLabel> {
        let label = self.coerce_providers_label(value)?;

        match label.name() {
            ProvidersName::NonDefault(flavor) => {
                if let NonDefaultProvidersName::Named(_) = flavor.as_ref() {
                    return Err(AttrCoercionContextError::UnexpectedProvidersName(
                        value.to_owned(),
                    )
                    .into());
                }
            }
            _ => {}
        }

        Ok(label.into_parts().0)
    }

    /// Attempt to convert a string into a label
    fn coerce_providers_label(&self, value: &str) -> kuro_error::Result<ProvidersLabel>;

    /// Reuse previously allocated string if possible.
    fn intern_str(&self, value: &str) -> ArcStr;

    // Reuse previously allocated slices if possible.
    fn intern_list(&self, value: Vec<CoercedAttr>) -> ArcSlice<CoercedAttr>;

    // Reuse previously allocated selects if possible.
    fn intern_select(
        &self,
        value: Vec<(ConfigurationSettingKey, CoercedAttr)>,
    ) -> ArcSlice<(ConfigurationSettingKey, CoercedAttr)>;

    // Reuse previously allocated dicts if possible.
    fn intern_dict(
        &self,
        value: Vec<(CoercedAttr, CoercedAttr)>,
    ) -> ArcSlice<(CoercedAttr, CoercedAttr)>;

    /// If the given filename is a predeclared output file of some other
    /// target in the current package (registered via `attr.output` /
    /// `attr.output_list`), return the producing target's label. Returns
    /// `None` if the path does not refer to a registered output — caller
    /// should then try source-file resolution. Default impl returns `None`
    /// for contexts that don't track a package-level output map.
    ///
    /// Bazel handles this by routing a source-label reference to a declared
    /// output file to the file's declaring target automatically. Kuro
    /// reproduces that in `SourceAttrType::coerce_item` so that patterns
    /// like `cc_library(srcs = ["foo.inc"])` resolve to the gentbl_rule
    /// that generates `foo.inc` when no source file of that name exists.
    fn output_file_target(&self, _value: &str) -> Option<ProvidersLabel> {
        None
    }

    /// Attempt to convert a string into a BuckPath
    fn coerce_path(&self, value: &str, allow_directory: bool) -> kuro_error::Result<CoercedPath>;

    fn coerce_target_pattern(
        &self,
        pattern: &str,
    ) -> kuro_error::Result<ParsedPattern<TargetPatternExtra>>;

    fn visit_query_function_literals<'q>(
        &self,
        visitor: &mut dyn QueryLiteralVisitor<'q>,
        expr: &Spanned<Expr<'q>>,
        query: &'q str,
    ) -> kuro_error::Result<()>;
}
