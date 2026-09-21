//! Bazel FeatureSet scope composition over owned repository/package inputs.
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use starlark_map::small_map::SmallMap;

pub(crate) fn unpack_feature_strings<'v>(
    value: starlark::values::Value<'v>,
    eval: &mut starlark::eval::Evaluator<'v, '_, '_>,
) -> anyhow::Result<Arc<[CompactString]>> {
    use starlark::values::UnpackValue;
    use starlark::values::dict::DictRef;
    use starlark::values::list::ListRef;
    use starlark::values::range::Range;
    use starlark::values::set::SetRef;
    use starlark::values::tuple::TupleRef;
    let value = if crate::provider::StarlarkDepset::parts_from_value(value).is_some() {
        let method = value
            .get_attr("to_list", eval.heap())
            .map_err(starlark::Error::into_anyhow)?
            .expect("native depset has to_list");
        eval.eval_function(method, &[], &[])
            .map_err(starlark::Error::into_anyhow)?
    } else {
        value
    };
    if ListRef::from_value(value).is_none()
        && TupleRef::from_value(value).is_none()
        && Range::from_value(value).is_none()
        && SetRef::unpack_value_opt(value).is_none()
        && DictRef::from_value(value).is_none()
    {
        anyhow::bail!("package features must be a list of strings");
    }
    value
        .iterate(eval.heap())
        .map_err(starlark::Error::into_anyhow)?
        .map(|value| {
            value
                .unpack_str()
                .map(CompactString::new)
                .ok_or_else(|| anyhow::anyhow!("package features must contain strings"))
        })
        .collect()
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Allocative, Dupe)]
pub struct PackageFeatureDefaults {
    repository: Arc<[CompactString]>,
    package: Option<Arc<[CompactString]>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Allocative, Dupe)]
pub struct EffectiveFeatures {
    enabled: Arc<[CompactString]>,
    disabled: Arc<[CompactString]>,
}

impl PackageFeatureDefaults {
    pub fn new(repository: Arc<[CompactString]>) -> Self {
        Self {
            repository,
            package: None,
        }
    }

    /// The package() boundary checks its once-per-BUILD lifecycle before this
    /// owned scope is installed. Repository and package inputs remain distinct.
    pub fn with_package_scope(mut self, scope: Arc<[CompactString]>) -> Self {
        assert!(
            self.package.is_none(),
            "package feature scope is installed once"
        );
        self.package = Some(scope);
        self
    }

    pub fn repository(&self) -> &[CompactString] {
        &self.repository
    }
    pub fn package(&self) -> Option<&[CompactString]> {
        self.package.as_deref()
    }

    pub fn resolve(&self, rule: &[CompactString], global: &[CompactString]) -> EffectiveFeatures {
        let mut local = parse(&self.repository);
        if let Some(scope) = &self.package {
            merge(&mut local, parse(scope));
        }
        merge(&mut local, parse(rule));
        let global = parse(global);
        for (feature, enabled) in global {
            if !enabled {
                local.insert(feature, false);
            } else {
                local.entry(feature).or_insert(true);
            }
        }
        let mut enabled = Vec::new();
        let mut disabled = Vec::new();
        for (feature, state) in local {
            if state {
                enabled.push(feature);
            } else {
                disabled.push(feature);
            }
        }
        // Java String.compareTo orders UTF-16 code units. Valid Unicode remains
        // Slug-native, but supplementary versus BMP ordering is reproduced.
        let sort = |values: &mut Vec<CompactString>| {
            values.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        };
        sort(&mut enabled);
        sort(&mut disabled);
        EffectiveFeatures {
            enabled: enabled.into(),
            disabled: disabled.into(),
        }
    }
}

impl EffectiveFeatures {
    pub fn enabled(&self) -> &[CompactString] {
        &self.enabled
    }
    pub fn disabled(&self) -> &[CompactString] {
        &self.disabled
    }
}

fn parse(values: &[CompactString]) -> SmallMap<CompactString, bool> {
    let mut result = SmallMap::new();
    for value in values {
        if let Some(name) = value.strip_prefix('-') {
            result.insert(name.into(), false);
        } else if value == "no_layering_check" {
            result.insert("layering_check".into(), false);
        } else {
            result.entry(value.clone()).or_insert(true);
        }
    }
    result
}

fn merge(coarse: &mut SmallMap<CompactString, bool>, fine: SmallMap<CompactString, bool>) {
    for (name, enabled) in fine {
        coarse.insert(name, enabled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn list(values: &[&str]) -> Arc<[CompactString]> {
        values.iter().map(|value| (*value).into()).collect()
    }

    #[test]
    fn feature_scopes_preserve_precedence_aliases_and_edge_names() {
        let defaults = PackageFeatureDefaults::new(list(&["a", "-b", "-r", "repo"]))
            .with_package_scope(list(&["-a", "b", "-p", "package"]));
        let result = defaults.resolve(
            &list(&["a", "-b", "p", "r", "-x", "x"]),
            &list(&["b", "-r", "global"]),
        );
        assert_eq!(
            result.enabled(),
            list(&["a", "global", "p", "package", "repo"]).as_ref()
        );
        assert_eq!(result.disabled(), list(&["b", "r", "x"]).as_ref());
        let result = PackageFeatureDefaults::default().resolve(
            &list(&[
                "",
                "-",
                "--",
                "-no_layering_check",
                "no_layering_check",
                "layering_check",
            ]),
            &[],
        );
        assert_eq!(
            result.disabled(),
            list(&["", "-", "layering_check", "no_layering_check"]).as_ref()
        );
        assert!(result.enabled().is_empty());
    }

    #[test]
    fn feature_scopes_keep_repo_package_boundaries_and_java_string_sorting() {
        let one = PackageFeatureDefaults::default().with_package_scope(list(&["-a", "a"]));
        let two = PackageFeatureDefaults::new(list(&["-a"])).with_package_scope(list(&["a"]));
        assert_ne!(one, two);
        assert_eq!(one.resolve(&[], &[]).disabled(), list(&["a"]).as_ref());
        assert_eq!(two.resolve(&[], &[]).enabled(), list(&["a"]).as_ref());
        let result = two.resolve(&list(&["\u{e000}", "😀", "a"]), &[]);
        assert_eq!(result.enabled(), list(&["a", "😀", "\u{e000}"]).as_ref());
        assert_ne!(
            PackageFeatureDefaults::new(list(&["a"])),
            PackageFeatureDefaults::default().with_package_scope(list(&["a"]))
        );
    }
}
