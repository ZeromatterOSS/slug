/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 * You may select, at your option, one of the above-listed licenses.
 */

use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use slug_identity_v2::CanonicalLabel;
use starlark::values::Freeze;
use starlark::values::Trace;
use starlark_map::small_set::SmallSet;

use crate::attrs::AttributeValue;
use crate::attrs::CoercedAttributeValue;

#[derive(Debug, Clone, Allocative, Trace, Freeze)]
pub(crate) enum RuleOutputsDefinitionGen<V> {
    Static(
        #[trace(unsafe_ignore)]
        #[freeze(identity)]
        Arc<[(CompactString, CompactString)]>,
    ),
    Callback(V),
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct PredeclaredOutput {
    pub key: CompactString,
    pub label: CanonicalLabel,
}
pub(crate) fn resolve_output_names(
    entries: &[(CompactString, CompactString)],
    target_name: &str,
    attributes: &[AttributeValue],
) -> anyhow::Result<Vec<(CompactString, CompactString)>> {
    entries
        .iter()
        .map(|(key, template)| {
            substitute_template(key, template, target_name, attributes)
                .map(|name| (key.clone(), name))
        })
        .collect()
}

fn substitute_template(
    key: &str,
    template: &str,
    target_name: &str,
    attributes: &[AttributeValue],
) -> anyhow::Result<CompactString> {
    let mut remaining = template;
    let mut output = String::with_capacity(template.len());
    while let Some(start) = remaining.find("%{") {
        output.push_str(&remaining[..start]);
        let after = &remaining[start + 2..];
        let Some(end) = after.find('}') else {
            output.push_str(&remaining[start..]);
            return Ok(output.into());
        };
        let placeholder = &after[..end];
        let value = single_value(placeholder, target_name, attributes)?.ok_or_else(|| {
            anyhow::anyhow!("For attribute '{key}' in outputs: Invalid placeholder(s) in template")
        })?;
        output.push_str(&value);
        remaining = &after[end + 1..];
    }
    output.push_str(remaining);
    Ok(output.into())
}

fn single_value(
    name: &str,
    target_name: &str,
    attributes: &[AttributeValue],
) -> anyhow::Result<Option<CompactString>> {
    if let Some(value) = target_placeholder(name, target_name) {
        return Ok(Some(value.into()));
    }
    let Some(value) = attributes
        .iter()
        .find(|attribute| attribute.declaration_name == name)
    else {
        return Ok(None);
    };
    let mut values = projected_values(name, value.value.as_ref())?;
    Ok((values.len() == 1).then(|| values.pop().unwrap()))
}

fn target_placeholder<'a>(name: &str, target: &'a str) -> Option<&'a str> {
    match name {
        "name" => Some(target),
        "dirname" => Some(target.rfind('/').map_or("", |slash| &target[..=slash])),
        "basename" => target.rsplit('/').next(),
        _ => None,
    }
}

fn projected_values(
    name: &str,
    value: &CoercedAttributeValue,
) -> anyhow::Result<Vec<CompactString>> {
    let remove_extension = |label: &CanonicalLabel| {
        let target = label.target().as_str();
        let slash = target.rfind('/').map_or(0, |index| index + 1);
        target[slash..]
            .rfind('.')
            .map_or(target, |dot| &target[..slash + dot])
            .into()
    };
    let values = match value {
        CoercedAttributeValue::String(value) => vec![value.clone()],
        CoercedAttributeValue::StringList(values) => values.to_vec(),
        CoercedAttributeValue::Label(label) => vec![remove_extension(label)],
        CoercedAttributeValue::LabelList(labels) => labels.iter().map(remove_extension).collect(),
        CoercedAttributeValue::Output(label) => vec![label.target().as_str().into()],
        CoercedAttributeValue::OutputList(labels) => labels
            .iter()
            .map(|label| CompactString::new(label.target().as_str()))
            .collect(),
        CoercedAttributeValue::Selector { .. } | CoercedAttributeValue::Concatenation(_, _) => {
            anyhow::bail!("Attribute {name} is configurable and cannot be used in outputs");
        }
        CoercedAttributeValue::None => return Ok(Vec::new()),
        CoercedAttributeValue::Boolean(_) => return unsupported(name, "boolean"),
        CoercedAttributeValue::Integer(_) => return unsupported(name, "integer"),
        CoercedAttributeValue::StringListDict(_)
        | CoercedAttributeValue::StringDict(_)
        | CoercedAttributeValue::StringKeyedLabelDict(_)
        | CoercedAttributeValue::LabelKeyedStringDict(_)
        | CoercedAttributeValue::LabelListDict(_) => {
            return unsupported(name, "dictionary");
        }
    };
    let mut seen = SmallSet::new();
    Ok(values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect())
}

fn unsupported<T>(name: &str, kind: &'static str) -> anyhow::Result<T> {
    anyhow::bail!(
        "For attribute '{name}' in outputs: Attributes of type {kind} cannot be used in an outputs substitution template"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attrs::AttributeProvenance;

    fn label(target: &str) -> CanonicalLabel {
        CanonicalLabel::parse(&format!("@@//pkg:{target}")).unwrap()
    }

    fn attribute(name: &str, value: CoercedAttributeValue) -> AttributeValue {
        AttributeValue {
            declaration_name: name.into(),
            provenance: AttributeProvenance::Explicit,
            value: Arc::new(value),
        }
    }

    fn entry(key: &str, template: &str) -> (CompactString, CompactString) {
        (key.into(), template.into())
    }

    #[test]
    fn templates_preserve_order_literals_specials_and_distinct_single_values() {
        let source = label("dir/source.tar.gz");
        let output = label("generated.bin");
        #[rustfmt::skip]
        let attributes = vec![
            attribute("text", CoercedAttributeValue::String("value".into())),
            attribute("words", CoercedAttributeValue::StringList(Arc::from(["same".into(), "same".into()]))),
            attribute("src", CoercedAttributeValue::Label(source.clone())),
            attribute("srcs", CoercedAttributeValue::LabelList(Arc::from([source.clone(), source]))),
            attribute("out", CoercedAttributeValue::Output(output.clone())),
            attribute("outs", CoercedAttributeValue::OutputList(Arc::from([output.clone(), output]))),
        ];
        let entries = [
            entry("literal", "100%-%{name}-%{tail"),
            entry("special", "%{dirname}%{basename}"),
            entry("repeated", "%{text}-%{text}"),
            entry("string-list", "%{words}"),
            entry("label", "%{src}"),
            entry("label-list", "%{srcs}"),
            entry("output", "%{out}"),
            entry("output-list", "%{outs}"),
        ];
        assert_eq!(
            resolve_output_names(&entries, "dir/probe", &attributes).unwrap(),
            [
                entry("literal", "100%-dir/probe-%{tail"),
                entry("special", "dir/probe"),
                entry("repeated", "value-value"),
                entry("string-list", "same"),
                entry("label", "dir/source.tar"),
                entry("label-list", "dir/source.tar"),
                entry("output", "generated.bin"),
                entry("output-list", "generated.bin"),
            ]
        );
    }

    #[test]
    fn templates_reject_missing_unsupported_configurable_and_non_single_values() {
        let condition = label("condition");
        #[rustfmt::skip]
        let attributes = vec![
            attribute("absent", CoercedAttributeValue::None),
            attribute("boolean", CoercedAttributeValue::Boolean(true)),
            attribute("integer", CoercedAttributeValue::Integer(1)),
            attribute("dictionary", CoercedAttributeValue::StringDict(Arc::from([]))),
            attribute("empty", CoercedAttributeValue::StringList(Arc::from([]))),
            attribute("multiple", CoercedAttributeValue::StringList(Arc::from(["one".into(), "two".into()]))),
            attribute("selected", CoercedAttributeValue::Selector { branches: Arc::from([(condition, Arc::new(CoercedAttributeValue::String("value".into())))]), default: None }),
        ];
        for (placeholder, expected) in [
            ("unknown", "Invalid placeholder(s)"),
            ("absent", "Invalid placeholder(s)"),
            ("boolean", "type boolean"),
            ("integer", "type integer"),
            ("dictionary", "type dictionary"),
            ("empty", "Invalid placeholder(s)"),
            ("multiple", "Invalid placeholder(s)"),
            ("selected", "is configurable"),
        ] {
            let error = resolve_output_names(
                &[entry("result", &format!("%{{{placeholder}}}"))],
                "probe",
                &attributes,
            )
            .unwrap_err()
            .to_string();
            assert!(error.contains(expected), "{placeholder}: {error}");
        }
    }
}
