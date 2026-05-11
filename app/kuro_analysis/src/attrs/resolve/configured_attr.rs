/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use dupe::Dupe;
use gazebo::prelude::SliceExt;
use kuro_artifact::artifact::artifact_type::Artifact;
use kuro_artifact::artifact::source_artifact::SourceArtifact;
use kuro_build_api::actions::query::CONFIGURED_ATTR_TO_VALUE;
use kuro_build_api::actions::query::PackageLabelOption;
use kuro_build_api::interpreter::rule_defs::artifact::starlark_artifact::StarlarkArtifact;
use kuro_core::configuration::pair::Configuration;
use kuro_core::package::PackageLabel;
use kuro_core::package::package_relative_path::PackageRelativePath;
use kuro_core::package::source_path::SourcePath;
use kuro_interpreter::types::configured_providers_label::StarlarkConfiguredProvidersLabel;
use kuro_interpreter::types::opaque_metadata::OpaqueMetadata;
use kuro_interpreter::types::target_label::StarlarkTargetLabel;
use kuro_node::attrs::attr_type::AttrType;
use kuro_node::attrs::attr_type::AttrTypeInner;
use kuro_node::attrs::attr_type::configuration_dep::ConfigurationDepAttrType;
use kuro_node::attrs::attr_type::configured_dep::ExplicitConfiguredDepAttrType;
use kuro_node::attrs::attr_type::dep::DepAttrType;
use kuro_node::attrs::attr_type::source::SourceAttrType;
use kuro_node::attrs::attr_type::split_transition_dep::SplitTransitionDepAttrType;
use kuro_node::attrs::attr_type::transition_dep::TransitionDepAttrType;
use kuro_node::attrs::configured_attr::ConfiguredAttr;
use kuro_node::visibility::VisibilityPatternList;
use kuro_node::visibility::VisibilitySpecification;
use kuro_node::visibility::WithinViewSpecification;
use kuro_util::arc_str::ArcS;
use starlark::values::Heap;
use starlark::values::Value;
use starlark::values::dict::Dict;
use starlark::values::list::AllocList;
use starlark::values::tuple::AllocTuple;
use starlark_map::small_map::SmallMap;

use crate::attrs::resolve::attr_type::arg::ConfiguredStringWithMacrosExt;
use crate::attrs::resolve::attr_type::configuration_dep::ConfigurationDepAttrTypeExt;
use crate::attrs::resolve::attr_type::dep::DepAttrTypeExt;
use crate::attrs::resolve::attr_type::dep::ExplicitConfiguredDepAttrTypeExt;
use crate::attrs::resolve::attr_type::dep::TransitionDepAttrTypeExt;
use crate::attrs::resolve::attr_type::query::ConfiguredQueryAttrExt;
use crate::attrs::resolve::attr_type::source::SourceAttrTypeExt;
use crate::attrs::resolve::attr_type::split_transition_dep::SplitTransitionDepAttrTypeExt;
use crate::attrs::resolve::ctx::AttrResolutionContext;

#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Tier0)]
enum ConfiguredAttrError {
    #[error("Source path `{0}` cannot be used in attributes referenced in transition")]
    SourceFileToStarlarkValue(ArcS<PackageRelativePath>),
}

pub trait ConfiguredAttrExt {
    fn resolve<'v>(
        &self,
        pkg: PackageLabel,
        ctx: &mut dyn AttrResolutionContext<'v>,
    ) -> kuro_error::Result<Vec<Value<'v>>>;

    fn resolve_single<'v>(
        &self,
        pkg: PackageLabel,
        ctx: &mut dyn AttrResolutionContext<'v>,
    ) -> kuro_error::Result<Value<'v>>;

    fn resolve_for_ctx_attr<'v>(
        &self,
        pkg: PackageLabel,
        attr_type: &AttrType,
        source_file_target_cfg: &Configuration,
        ctx: &mut dyn AttrResolutionContext<'v>,
    ) -> kuro_error::Result<Vec<Value<'v>>>;

    fn resolve_single_for_ctx_attr<'v>(
        &self,
        pkg: PackageLabel,
        attr_type: &AttrType,
        source_file_target_cfg: &Configuration,
        ctx: &mut dyn AttrResolutionContext<'v>,
    ) -> kuro_error::Result<Value<'v>>;

    fn to_value<'v>(
        &self,
        pkg: PackageLabelOption,
        heap: Heap<'v>,
    ) -> kuro_error::Result<Value<'v>>;
}

impl ConfiguredAttrExt for ConfiguredAttr {
    /// "Resolves" the configured value to the resolved value provided to the rule implementation.
    ///
    /// `resolve` may return multiple values. It is up to the caller to fail if
    /// an inappropriate number of elements is returned. e.g. `attrs.list()` might
    /// accept and merge multiple returned values from `attrs.source()`, but
    /// `attrs.optional()` might only accept a single value, and fail otherwise.
    fn resolve<'v>(
        &self,
        pkg: PackageLabel,
        ctx: &mut dyn AttrResolutionContext<'v>,
    ) -> kuro_error::Result<Vec<Value<'v>>> {
        match self {
            // SourceLabel is special since it is the only type that can be expand to many
            ConfiguredAttr::SourceLabel(src) => SourceAttrType::resolve_label(ctx, src),
            // OneOf could contain a SourceLabel
            ConfiguredAttr::OneOf(box l, _) => l.resolve(pkg, ctx),
            _ => Ok(vec![self.resolve_single(pkg, ctx)?]),
        }
    }

    /// Resolving a single value is common, so `resolve_single` will validate
    /// this function's output, and return a single value or an error.
    fn resolve_single<'v>(
        &self,
        pkg: PackageLabel,
        ctx: &mut dyn AttrResolutionContext<'v>,
    ) -> kuro_error::Result<Value<'v>> {
        resolve_single_impl(self, pkg, None, ctx)
    }

    fn resolve_for_ctx_attr<'v>(
        &self,
        pkg: PackageLabel,
        attr_type: &AttrType,
        source_file_target_cfg: &Configuration,
        ctx: &mut dyn AttrResolutionContext<'v>,
    ) -> kuro_error::Result<Vec<Value<'v>>> {
        let source_file_as_target = false;
        resolve_for_ctx_attr_impl(
            self,
            pkg,
            attr_type,
            source_file_target_cfg,
            source_file_as_target,
            ctx,
        )
    }

    fn resolve_single_for_ctx_attr<'v>(
        &self,
        pkg: PackageLabel,
        attr_type: &AttrType,
        source_file_target_cfg: &Configuration,
        ctx: &mut dyn AttrResolutionContext<'v>,
    ) -> kuro_error::Result<Value<'v>> {
        let mut resolved =
            self.resolve_for_ctx_attr(pkg, attr_type, source_file_target_cfg, ctx)?;
        if resolved.len() == 1 {
            Ok(resolved.pop().unwrap())
        } else {
            Ok(ctx.heap().alloc(resolved))
        }
    }

    /// Resolving a single value is common, so `resolve_single` will validate
    /// this function's output, and return a single value or an error.
    fn to_value<'v>(
        &self,
        pkg: PackageLabelOption,
        heap: Heap<'v>,
    ) -> kuro_error::Result<Value<'v>> {
        configured_attr_to_value(self, pkg, heap)
    }
}

fn resolve_single_impl<'v>(
    this: &ConfiguredAttr,
    pkg: PackageLabel,
    source_file_target_cfg: Option<&Configuration>,
    ctx: &mut dyn AttrResolutionContext<'v>,
) -> kuro_error::Result<Value<'v>> {
    match this {
        ConfiguredAttr::Bool(v) => Ok(Value::new_bool(v.0)),
        ConfiguredAttr::Int(v) => Ok(ctx.heap().alloc(*v)),
        ConfiguredAttr::String(v) | ConfiguredAttr::EnumVariant(v) => {
            Ok(ctx.heap().alloc(v.as_str()))
        }
        ConfiguredAttr::List(list) => {
            let mut values = Vec::with_capacity(list.len());
            for v in list.iter() {
                values.append(&mut v.resolve(pkg, ctx)?);
            }
            Ok(ctx.heap().alloc(values))
        }
        ConfiguredAttr::Tuple(list) => {
            let mut values = Vec::with_capacity(list.len());
            for v in list.iter() {
                values.push(v.resolve_single(pkg, ctx)?);
            }
            Ok(ctx.heap().alloc(AllocTuple(values)))
        }
        ConfiguredAttr::Dict(dict) => {
            let mut res = SmallMap::with_capacity(dict.len());
            for (k, v) in dict.iter() {
                res.insert_hashed(
                    k.resolve_single(pkg, ctx)?.get_hashed()?,
                    v.resolve_single(pkg, ctx)?,
                );
            }
            Ok(ctx.heap().alloc(Dict::new(res)))
        }
        ConfiguredAttr::None => Ok(Value::new_none()),
        ConfiguredAttr::OneOf(box l, _) => resolve_single_impl(l, pkg, source_file_target_cfg, ctx),
        a @ (ConfiguredAttr::Visibility(_) | ConfiguredAttr::WithinView(_)) => {
            // TODO(nga): rule implementations should not need visibility attribute.
            //   But adding it here to preserve existing behavior.
            configured_attr_to_value(a, PackageLabelOption::PackageLabel(pkg), ctx.heap())
        }
        ConfiguredAttr::ExplicitConfiguredDep(d) => {
            ExplicitConfiguredDepAttrType::resolve_single(ctx, d.as_ref())
        }
        ConfiguredAttr::TransitionDep(d) => TransitionDepAttrType::resolve_single(ctx, d.as_ref()),
        ConfiguredAttr::SplitTransitionDep(d) => {
            SplitTransitionDepAttrType::resolve_single(ctx, d.as_ref())
        }
        ConfiguredAttr::ConfigurationDep(d) => ConfigurationDepAttrType::resolve_single(ctx, d),
        ConfiguredAttr::PluginDep(d, _) => Ok(ctx.heap().alloc(StarlarkTargetLabel::new(d.dupe()))),
        ConfiguredAttr::Dep(d) => DepAttrType::resolve_single(ctx, d),
        ConfiguredAttr::SourceLabel(s) => SourceAttrType::resolve_single_label(ctx, s),
        ConfiguredAttr::Label(label) => {
            let label = StarlarkConfiguredProvidersLabel::new(label.dupe());
            Ok(ctx.heap().alloc(label))
        }
        ConfiguredAttr::Arg(arg) => arg.resolve(ctx, pkg),
        ConfiguredAttr::Query(query) => query.resolve(ctx),
        ConfiguredAttr::SourceFile(s) => {
            let path = SourcePath::new(pkg, s.path().dupe());
            match source_file_target_cfg {
                Some(cfg_pair) => SourceAttrType::resolve_single_file_target(ctx, path, cfg_pair),
                None => Ok(SourceAttrType::resolve_single_file(ctx, path)),
            }
        }
        ConfiguredAttr::Metadata(..) => Ok(ctx.heap().alloc(OpaqueMetadata)),
        ConfiguredAttr::TargetModifiers(..) => Ok(ctx.heap().alloc(OpaqueMetadata)),
    }
}

fn resolve_for_ctx_attr_impl<'v>(
    this: &ConfiguredAttr,
    pkg: PackageLabel,
    attr_type: &AttrType,
    source_file_target_cfg: &Configuration,
    source_file_as_target: bool,
    ctx: &mut dyn AttrResolutionContext<'v>,
) -> kuro_error::Result<Vec<Value<'v>>> {
    match (&attr_type.0.inner, this) {
        (AttrTypeInner::List(list_ty), ConfiguredAttr::List(list)) => {
            let mut values = Vec::with_capacity(list.len());
            for v in list.iter() {
                values.append(&mut resolve_for_ctx_attr_impl(
                    v,
                    pkg.dupe(),
                    &list_ty.inner,
                    source_file_target_cfg,
                    source_file_as_target,
                    ctx,
                )?);
            }
            Ok(vec![ctx.heap().alloc(values)])
        }
        (AttrTypeInner::Tuple(tuple_ty), ConfiguredAttr::Tuple(list))
            if tuple_ty.xs.len() == list.len() =>
        {
            let mut values = Vec::with_capacity(list.len());
            for (v, inner_ty) in list.iter().zip(tuple_ty.xs.iter()) {
                values.push(resolve_single_for_ctx_attr_impl(
                    v,
                    pkg.dupe(),
                    inner_ty,
                    source_file_target_cfg,
                    source_file_as_target,
                    ctx,
                )?);
            }
            Ok(vec![ctx.heap().alloc(AllocTuple(values))])
        }
        (AttrTypeInner::Dict(dict_ty), ConfiguredAttr::Dict(dict)) => {
            let mut res = SmallMap::with_capacity(dict.len());
            for (k, v) in dict.iter() {
                res.insert_hashed(
                    resolve_single_for_ctx_attr_impl(
                        k,
                        pkg.dupe(),
                        &dict_ty.key,
                        source_file_target_cfg,
                        source_file_as_target,
                        ctx,
                    )?
                    .get_hashed()?,
                    resolve_single_for_ctx_attr_impl(
                        v,
                        pkg.dupe(),
                        &dict_ty.value,
                        source_file_target_cfg,
                        source_file_as_target,
                        ctx,
                    )?,
                );
            }
            Ok(vec![ctx.heap().alloc(Dict::new(res))])
        }
        (AttrTypeInner::Option(_), ConfiguredAttr::None) => Ok(vec![Value::new_none()]),
        (AttrTypeInner::Option(option_ty), _) => resolve_for_ctx_attr_impl(
            this,
            pkg,
            &option_ty.inner,
            source_file_target_cfg,
            source_file_as_target,
            ctx,
        ),
        (AttrTypeInner::OneOf(oneof_ty), ConfiguredAttr::OneOf(inner, i)) => {
            let inner_ty = oneof_ty
                .xs
                .get(*i as usize)
                .ok_or_else(|| kuro_error::internal_error!("oneof index ({}) out of bounds", i))?;
            let source_file_as_target =
                source_file_as_target || selected_source_variant_is_dependency_attr(oneof_ty, *i)?;
            resolve_for_ctx_attr_impl(
                inner,
                pkg,
                inner_ty,
                source_file_target_cfg,
                source_file_as_target,
                ctx,
            )
        }
        (AttrTypeInner::Source(_), ConfiguredAttr::SourceLabel(src)) => {
            SourceAttrType::resolve_label(ctx, src)
        }
        _ => Ok(vec![resolve_single_impl(
            this,
            pkg,
            if source_file_as_target {
                Some(source_file_target_cfg)
            } else {
                None
            },
            ctx,
        )?]),
    }
}

fn resolve_single_for_ctx_attr_impl<'v>(
    this: &ConfiguredAttr,
    pkg: PackageLabel,
    attr_type: &AttrType,
    source_file_target_cfg: &Configuration,
    source_file_as_target: bool,
    ctx: &mut dyn AttrResolutionContext<'v>,
) -> kuro_error::Result<Value<'v>> {
    let mut resolved = resolve_for_ctx_attr_impl(
        this,
        pkg,
        attr_type,
        source_file_target_cfg,
        source_file_as_target,
        ctx,
    )?;
    if resolved.len() == 1 {
        Ok(resolved.pop().unwrap())
    } else {
        Ok(ctx.heap().alloc(resolved))
    }
}

fn selected_source_variant_is_dependency_attr(
    oneof_ty: &kuro_node::attrs::attr_type::one_of::OneOfAttrType,
    selected: u32,
) -> kuro_error::Result<bool> {
    let selected_ty = oneof_ty
        .xs
        .get(selected as usize)
        .ok_or_else(|| kuro_error::internal_error!("oneof index ({}) out of bounds", selected))?;
    if !matches!(selected_ty.0.inner, AttrTypeInner::Source(_)) {
        return Ok(false);
    }
    Ok(oneof_ty.xs.iter().any(|ty| {
        matches!(
            ty.0.inner,
            AttrTypeInner::Dep(_) | AttrTypeInner::TransitionDep(_)
        )
    }))
}

fn configured_attr_to_value<'v>(
    this: &ConfiguredAttr,
    pkg: PackageLabelOption,
    heap: Heap<'v>,
) -> kuro_error::Result<Value<'v>> {
    Ok(match this {
        ConfiguredAttr::Bool(v) => heap.alloc(v.0),
        ConfiguredAttr::Int(v) => heap.alloc(*v),
        ConfiguredAttr::String(s) | ConfiguredAttr::EnumVariant(s) => heap.alloc(s.as_str()),
        ConfiguredAttr::List(list) => {
            heap.alloc(list.try_map(|v| configured_attr_to_value(&v, pkg, heap))?)
        }
        ConfiguredAttr::Tuple(v) => heap.alloc(AllocTuple(
            v.try_map(|v| configured_attr_to_value(&v, pkg, heap))?,
        )),
        ConfiguredAttr::Dict(map) => {
            let mut res = SmallMap::with_capacity(map.len());

            for (k, v) in map.iter() {
                res.insert_hashed(
                    configured_attr_to_value(&k, pkg, heap)?.get_hashed()?,
                    configured_attr_to_value(&v, pkg, heap)?,
                );
            }

            heap.alloc(Dict::new(res))
        }
        ConfiguredAttr::None => Value::new_none(),
        ConfiguredAttr::OneOf(box l, _) => configured_attr_to_value(&l, pkg, heap)?,
        ConfiguredAttr::Visibility(VisibilitySpecification(specs))
        | ConfiguredAttr::WithinView(WithinViewSpecification(specs)) => match specs {
            VisibilityPatternList::Public => heap.alloc(AllocList(["PUBLIC"])),
            VisibilityPatternList::List(specs) => {
                heap.alloc(AllocList(specs.iter().map(|s| s.to_string())))
            }
        },
        ConfiguredAttr::ExplicitConfiguredDep(d) => match pkg {
            PackageLabelOption::TransitionAttr => heap.alloc(StarlarkTargetLabel::new(
                d.as_ref().label.target().unconfigured().dupe(),
            )),
            PackageLabelOption::PackageLabel(_) => heap.alloc(
                StarlarkConfiguredProvidersLabel::new(d.as_ref().label.dupe()),
            ),
        },
        ConfiguredAttr::TransitionDep(t) => match pkg {
            PackageLabelOption::TransitionAttr => heap.alloc(StarlarkTargetLabel::new(
                t.dep.target().unconfigured().dupe(),
            )),
            PackageLabelOption::PackageLabel(_) => {
                heap.alloc(StarlarkConfiguredProvidersLabel::new(t.dep.dupe()))
            }
        },
        ConfiguredAttr::SplitTransitionDep(t) => {
            let mut map = SmallMap::with_capacity(t.deps.len());

            for (trans, p) in t.deps.iter() {
                let label = match pkg {
                    PackageLabelOption::TransitionAttr => {
                        heap.alloc(StarlarkTargetLabel::new(p.target().unconfigured().dupe()))
                    }
                    PackageLabelOption::PackageLabel(_) => {
                        heap.alloc(StarlarkConfiguredProvidersLabel::new(p.dupe()))
                    }
                };
                map.insert_hashed(heap.alloc(trans).get_hashed()?, label);
            }

            heap.alloc(Dict::new(map))
        }
        ConfiguredAttr::ConfigurationDep(c) => {
            // TODO(T198210718)
            heap.alloc(StarlarkTargetLabel::new(c.target().dupe()))
        }
        ConfiguredAttr::PluginDep(d, _) => heap.alloc(StarlarkTargetLabel::new(d.dupe())),
        ConfiguredAttr::Dep(d) => match pkg {
            PackageLabelOption::TransitionAttr => heap.alloc(StarlarkTargetLabel::new(
                d.label.target().unconfigured().dupe(),
            )),
            PackageLabelOption::PackageLabel(_) => {
                heap.alloc(StarlarkConfiguredProvidersLabel::new(d.label.dupe()))
            }
        },
        ConfiguredAttr::SourceLabel(s) => match pkg {
            PackageLabelOption::TransitionAttr => {
                heap.alloc(StarlarkTargetLabel::new(s.target().unconfigured().dupe()))
            }
            PackageLabelOption::PackageLabel(_) => {
                heap.alloc(StarlarkConfiguredProvidersLabel::new(s.dupe()))
            }
        },
        ConfiguredAttr::Label(l) => match pkg {
            PackageLabelOption::TransitionAttr => {
                heap.alloc(StarlarkTargetLabel::new(l.target().unconfigured().dupe()))
            }
            PackageLabelOption::PackageLabel(_) => {
                heap.alloc(StarlarkConfiguredProvidersLabel::new(l.dupe()))
            }
        },
        ConfiguredAttr::Arg(arg) => heap.alloc(arg.to_string()),
        ConfiguredAttr::Query(query) => heap.alloc(&query.query.query),
        ConfiguredAttr::SourceFile(f) => match pkg {
            PackageLabelOption::PackageLabel(pkg) => {
                heap.alloc(StarlarkArtifact::new(Artifact::from(SourceArtifact::new(
                    SourcePath::new(pkg.to_owned(), f.path().dupe()),
                ))))
            }
            // We don't store package label in transition key for better caching of transition between packages.
            // (This is not inherent requirement,
            // but it was easier to implement this ways,
            // and probably transitions do not need access to sources anyway).
            // So package label is not available. If the need arises, we can store package label along with source attributes.
            // TODO(romanp): add earlier check during rule function construction to prevent using source attributes in transitions.
            PackageLabelOption::TransitionAttr => {
                return Err(ConfiguredAttrError::SourceFileToStarlarkValue(f.path().dupe()).into());
            }
        },
        ConfiguredAttr::Metadata(data) => heap.alloc(data.to_value()),
        ConfiguredAttr::TargetModifiers(data) => heap.alloc(data.to_value()),
    })
}

pub(crate) fn init_configured_attr_to_value() {
    CONFIGURED_ATTR_TO_VALUE.init(configured_attr_to_value);
}
