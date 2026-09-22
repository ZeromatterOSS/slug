//! Ordinary immutable data projected from the already-validated prerequisites.
//!
//! Bazel's StarlarkAttributesCollection stores plain StructImpl values here.
//! Only the ctx getter is a capability: captured structs/lists outlive ctx.

use slug_loading_v2::attrs::AttributeKind;
use slug_loading_v2::attrs::AttributeSchema;

use super::*;

pub(super) fn materialize(
    schema: &[AttributeSchema],
    dependencies: &[PreparedDependency],
    materializer: &mut AnalysisValueMaterializer<'_>,
) -> Result<(FrozenValue, FrozenValue), String> {
    // Index the existing ordered edges once. Do not include runfiles or the
    // executable independently of the dependency's DefaultInfo.files.
    let mut children: SmallMap<&str, Vec<AnalysisDepset>> = SmallMap::new();
    for dependency in dependencies
        .iter()
        .filter(|dependency| !dependency.filtered)
    {
        // Presence is distinct from an absent optional prerequisite. Native
        // getPrerequisiteArtifact rejects a present target with zero Files,
        // even when NO_FILE bypassed earlier file-type/cardinality validation.
        let files = children.entry(dependency.attribute.as_str()).or_default();
        if let Some(default) = dependency.providers.default_info() {
            files.push(default.files().clone());
        }
    }
    let mut files = SmallMap::new();
    let mut single_files = SmallMap::new();
    for attribute in schema {
        if !matches!(
            attribute.kind(),
            AttributeKind::Label
                | AttributeKind::LabelList
                | AttributeKind::StringKeyedLabelDict
                | AttributeKind::LabelKeyedStringDict
                | AttributeKind::LabelListDict
        ) {
            continue;
        }
        let name = attribute.declaration_name();
        let prerequisites = children.shift_remove(name);
        let present = prerequisites.is_some();
        // Stable parent order delegates traversal and structural deduplication
        // to the existing depset owner, including non-default child orders.
        let merged = AnalysisDepset::new(
            DepsetOrder::Default,
            Vec::new(),
            prerequisites.unwrap_or_default(),
        )
        .map_err(|error| error.to_string())?;
        let values = merged.to_list();
        if attribute.file_admissibility().single_artifact() {
            let value = match values.as_slice() {
                [] if !present => AnalysisValue::none(),
                [value] => value.clone(),
                _ => return Err(format!("ctx.file.{name} requires exactly one File")),
            };
            single_files.insert(name, value);
        }
        files.insert(name, AnalysisValue::list(values));
    }
    Ok((
        materializer.value(&AnalysisValue::strukt(single_files))?,
        materializer.value(&AnalysisValue::strukt(files))?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_attribute_files_are_ordinary_publishable_structs() {
        let module = Module::new();
        let (file, files) = materialize(
            &[],
            &[],
            &mut AnalysisValueMaterializer::new(module.frozen_heap()),
        )
        .unwrap();
        for value in [file, files] {
            assert_eq!(value.to_value().get_type(), "struct");
            assert!(value.to_value().dir_attr().is_empty());
            assert!(
                value
                    .to_value()
                    .get_attr("absent", module.heap())
                    .unwrap()
                    .is_none()
            );
            let retained = AnalysisValueLowerer::from_module(&module)
                .lower(value.to_value(), "empty attribute files")
                .unwrap();
            assert!(
                matches!(retained.kind(), AnalysisValueKind::Struct(fields) if fields.is_empty())
            );
        }
    }
}
