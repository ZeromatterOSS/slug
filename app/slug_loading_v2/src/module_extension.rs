/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the above-listed
 * licenses.
 */

use std::cell::RefCell;
use std::cmp::Ordering;
use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_bzlmod_v2::HostSelectedExtensionDefinitionLoadRequest;
use slug_bzlmod_v2::RootPackageBzlTarget;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_events_v2::StarlarkSourceLocation;
use slug_identity_v2::CanonicalLabel;
use slug_workspace_v2::NormalizedAbsolutePath;
use starlark::PrintHandler;
use starlark::PrintLocation;
use starlark::any::ProvidesStaticType;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;
use starlark_map::StarlarkHasher;

use crate::attrs::CoercedAttributeValue;
use crate::bzl_module::HostBzlModuleError;
use crate::bzl_module::HostBzlModuleEvalKey;
use crate::bzl_module::HostPreparedModuleExtensionInputs;
use crate::bzl_module::HostPreparedModuleExtensionInputsError;
use crate::bzl_module::HostPreparedModuleExtensionInputsKey;
use crate::bzl_module::HostRootBzlLabel;
use crate::bzl_module::PreparedModuleExtensionInput;
use crate::bzl_module::PreparedModuleExtensionTag;
use crate::package::FrozenModuleExtensionDefinition;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostPureModuleExtensionInvocations {
    pub(crate) prepared: Arc<HostPreparedModuleExtensionInputs>,
    pub(crate) invoked: Arc<[HostPureModuleExtensionInvocationReceipt]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostPureModuleExtensionInvocationReceipt {
    pub(crate) request: HostSelectedExtensionDefinitionLoadRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostPureModuleExtensionInvocationsError {
    Prepared(HostPreparedModuleExtensionInputsError),
    PreparedCompute(CompactString),
    AfterPrepared {
        prepared: Arc<HostPreparedModuleExtensionInputs>,
        request: Option<HostSelectedExtensionDefinitionLoadRequest>,
        error: HostPureModuleExtensionInvocationError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostPureModuleExtensionInvocationError {
    UnsupportedFactors,
    Label(CompactString),
    Bzl(HostBzlModuleError),
    Drift(CompactString),
    Invocation(CompactString),
    Result(CompactString),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostPureModuleExtensionInvocationsKey {
    workspace: NormalizedAbsolutePath,
}

impl HostPureModuleExtensionInvocationsKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostPureModuleExtensionInvocationsKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-pure-module-extension-invocations:{}",
            self.workspace
        )
    }
}

pub(crate) type HostPureModuleExtensionInvocationsOutcome = SourcePreparationOutcome<
    Arc<Result<HostPureModuleExtensionInvocations, HostPureModuleExtensionInvocationsError>>,
>;

fn complete(
    value: Result<HostPureModuleExtensionInvocations, HostPureModuleExtensionInvocationsError>,
) -> HostPureModuleExtensionInvocationsOutcome {
    SourcePreparationOutcome::Complete(Arc::new(value))
}

#[derive(Default)]
struct InvocationPrintCapture {
    events: RefCell<Vec<EvaluationEvent>>,
}

impl InvocationPrintCapture {
    fn into_batch(self) -> EventBatch {
        EventBatch::from_events(self.events.into_inner())
    }
}

impl PrintHandler for InvocationPrintCapture {
    fn println(&self, location: PrintLocation, text: &str) -> starlark::Result<()> {
        let (file, line, column) = location.into_parts();
        self.events
            .borrow_mut()
            .push(EvaluationEvent::StarlarkPrint {
                location: StarlarkSourceLocation::new(file, line, column),
                text: text.into(),
            });
        Ok(())
    }
}

#[async_trait]
impl Key for HostPureModuleExtensionInvocationsKey {
    type Value = HostPureModuleExtensionInvocationsOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let prepared = match ctx
            .compute(&HostPreparedModuleExtensionInputsKey::new(
                self.workspace.clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                Ok(value) => Arc::new(value.clone()),
                Err(error) => {
                    return complete(Err(HostPureModuleExtensionInvocationsError::Prepared(
                        error.clone(),
                    )));
                }
            },
            Err(error) => {
                return complete(Err(
                    HostPureModuleExtensionInvocationsError::PreparedCompute(
                        error.to_string().into(),
                    ),
                ));
            }
        };
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let mut event_batch = EventBatch::empty();
        let result = invoke_all(
            ctx,
            self.workspace.clone(),
            prepared,
            capture_events,
            &mut event_batch,
        )
        .await;
        if capture_events && matches!(result, SourcePreparationOutcome::Complete(_)) {
            ctx.store_evaluation_data(event_batch)
                .expect("pure module-extension invocation stores one Complete event batch");
        }
        result
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

async fn invoke_all(
    ctx: &mut DiceComputations<'_>,
    workspace: NormalizedAbsolutePath,
    prepared: Arc<HostPreparedModuleExtensionInputs>,
    capture_events: bool,
    event_batch: &mut EventBatch,
) -> HostPureModuleExtensionInvocationsOutcome {
    let after = |request: Option<&HostSelectedExtensionDefinitionLoadRequest>, error| {
        HostPureModuleExtensionInvocationsError::AfterPrepared {
            prepared: prepared.clone(),
            request: request.cloned(),
            error,
        }
    };
    struct Preflight {
        module: starlark::environment::FrozenModule,
        implementation: starlark::values::FrozenValue,
        tag_classes: Arc<[CompactString]>,
    }
    let mut preflight = Vec::with_capacity(prepared.inputs.len());
    for (index, input) in prepared.inputs.iter().enumerate() {
        let (request, _, _, _) = input.input.parts().0.parts();
        let loaded = &prepared.definitions.definitions[index];
        let loaded_manifest = &loaded.manifest;
        let loaded_definition = &loaded.definition;
        if !loaded_definition.environment.is_empty()
            || loaded_definition.os_dependent
            || loaded_definition.arch_dependent
            || loaded_definition.facts_version != 0
        {
            return complete(Err(after(
                Some(input.input.parts().0),
                HostPureModuleExtensionInvocationError::UnsupportedFactors,
            )));
        }
        let target = match RootPackageBzlTarget::parse(request.target().as_str()) {
            Ok(target) => target,
            Err(error) => {
                return complete(Err(after(
                    Some(input.input.parts().0),
                    HostPureModuleExtensionInvocationError::Label(error.to_string().into()),
                )));
            }
        };
        let label = HostRootBzlLabel::new(request.package().package().clone(), target);
        let module = match ctx
            .compute(&HostBzlModuleEvalKey::new(workspace.clone(), label))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                Ok(module) => module.clone(),
                Err(error) => {
                    return complete(Err(after(
                        Some(input.input.parts().0),
                        HostPureModuleExtensionInvocationError::Bzl(error.clone()),
                    )));
                }
            },
            Err(error) => {
                return complete(Err(after(
                    Some(input.input.parts().0),
                    HostPureModuleExtensionInvocationError::Invocation(error.to_string().into()),
                )));
            }
        };
        if &module.manifest != loaded_manifest {
            return complete(Err(after(
                Some(input.input.parts().0),
                HostPureModuleExtensionInvocationError::Drift("reacquired manifest differs".into()),
            )));
        }
        let export = match module.module.get(input.input.parts().0.parts().1) {
            Ok(value) => value,
            Err(error) => {
                return complete(Err(after(
                    Some(input.input.parts().0),
                    HostPureModuleExtensionInvocationError::Drift(error.to_string().into()),
                )));
            }
        };
        let definition = match export.downcast::<FrozenModuleExtensionDefinition>() {
            Ok(value) => value,
            Err(_) => {
                return complete(Err(after(
                    Some(input.input.parts().0),
                    HostPureModuleExtensionInvocationError::Drift(
                        "reacquired export is not module_extension".into(),
                    ),
                )));
            }
        };
        if &definition.projection() != loaded_definition {
            return complete(Err(after(
                Some(input.input.parts().0),
                HostPureModuleExtensionInvocationError::Drift(
                    "reacquired definition differs".into(),
                ),
            )));
        }
        preflight.push(Preflight {
            module: module.module.dupe(),
            implementation: definition.implementation,
            tag_classes: loaded_definition
                .tag_classes
                .iter()
                .map(|(name, _)| name.clone())
                .collect::<Vec<_>>()
                .into(),
        });
    }

    let mut invoked = Vec::with_capacity(prepared.inputs.len());
    for (input, preflight) in prepared.inputs.iter().zip(preflight) {
        let _module_lifetime = preflight.module;
        let invocation_module = Module::new();
        let owner = Arc::new(());
        let context = invocation_module
            .heap()
            .alloc_simple(InvocationContext::new(input, preflight.tag_classes, &owner));
        let capture = capture_events.then(InvocationPrintCapture::default);
        let returned = {
            let mut evaluator = Evaluator::new(&invocation_module);
            if let Some(capture) = capture.as_ref() {
                evaluator.set_print_handler(capture);
            }
            let result =
                evaluator.eval_function(preflight.implementation.to_value(), &[context], &[]);
            drop(evaluator);
            result
        };
        if let Some(capture) = capture {
            let capture = capture.into_batch();
            *event_batch = EventBatch::from_events(
                event_batch
                    .events()
                    .iter()
                    .cloned()
                    .chain(capture.events().iter().cloned()),
            );
        }
        let returned = match returned {
            Ok(value) => value,
            Err(error) => {
                return complete(Err(after(
                    Some(input.input.parts().0),
                    HostPureModuleExtensionInvocationError::Invocation(error.to_string().into()),
                )));
            }
        };
        if !returned.is_none() {
            return complete(Err(after(
                Some(input.input.parts().0),
                HostPureModuleExtensionInvocationError::Result(
                    format!(
                        "module extension must return None, got {}",
                        returned.get_type()
                    )
                    .into(),
                ),
            )));
        }
        invoked.push(HostPureModuleExtensionInvocationReceipt {
            request: input.input.parts().0.clone(),
        });
    }
    complete(Ok(HostPureModuleExtensionInvocations {
        prepared,
        invoked: invoked.into(),
    }))
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct InvocationContext {
    module: InvocationModule,
    #[allocative(skip)]
    owner: Arc<()>,
}

impl InvocationContext {
    fn new(
        input: &PreparedModuleExtensionInput,
        tag_classes: Arc<[CompactString]>,
        owner: &Arc<()>,
    ) -> Self {
        let (_, _, name, version, is_root, _) = input.input.parts();
        Self {
            module: InvocationModule {
                name: name.into(),
                version: version.into(),
                is_root,
                tag_classes,
                tags: input.tags.clone(),
                owner: owner.clone(),
            },
            owner: owner.clone(),
        }
    }
}

impl fmt::Display for InvocationContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<module_ctx>")
    }
}
starlark::starlark_simple_value!(InvocationContext);

#[starlark_value(type = "module_ctx")]
impl<'v> StarlarkValue<'v> for InvocationContext {
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        (name == "modules")
            .then(|| heap.alloc_simple(InvocationModuleList(Arc::from([self.module.clone()]))))
    }
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(invocation_context_methods)
    }
}

#[starlark_module]
fn invocation_context_methods(builder: &mut MethodsBuilder) {
    fn is_dev_dependency(this: Value, tag: Value) -> anyhow::Result<bool> {
        let this = InvocationContext::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid module_ctx receiver"))?;
        let tag = InvocationTag::from_value(tag)
            .ok_or_else(|| anyhow::anyhow!("expected a tag from ctx.modules"))?;
        if !Arc::ptr_eq(&this.owner, &tag.owner) {
            anyhow::bail!("tag belongs to another module_ctx");
        }
        Ok(tag.dev_dependency)
    }
    fn tag_sort_key(this: Value, tag: Value) -> anyhow::Result<InvocationTagSortKey> {
        let this = InvocationContext::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid module_ctx receiver"))?;
        let tag = InvocationTag::from_value(tag)
            .ok_or_else(|| anyhow::anyhow!("expected a tag from ctx.modules"))?;
        if !Arc::ptr_eq(&this.owner, &tag.owner) {
            anyhow::bail!("tag belongs to another module_ctx");
        }
        Ok(InvocationTagSortKey(tag.module_index, tag.tag_index))
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct InvocationModule {
    name: CompactString,
    version: CompactString,
    is_root: bool,
    tag_classes: Arc<[CompactString]>,
    tags: Arc<[PreparedModuleExtensionTag]>,
    #[allocative(skip)]
    owner: Arc<()>,
}
impl fmt::Display for InvocationModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<bazel_module>")
    }
}
starlark::starlark_simple_value!(InvocationModule);
#[starlark_value(type = "bazel_module")]
impl<'v> StarlarkValue<'v> for InvocationModule {
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match name {
            "name" => Some(heap.alloc_str(&self.name).to_value()),
            "version" => Some(heap.alloc_str(&self.version).to_value()),
            "is_root" => Some(Value::new_bool(self.is_root)),
            "tags" => Some(heap.alloc_simple(InvocationTags {
                classes: self.tag_classes.clone(),
                tags: self.tags.clone(),
                owner: self.owner.clone(),
            })),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct InvocationTags {
    classes: Arc<[CompactString]>,
    tags: Arc<[PreparedModuleExtensionTag]>,
    #[allocative(skip)]
    owner: Arc<()>,
}
impl fmt::Display for InvocationTags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<bazel_module_tags>")
    }
}
starlark::starlark_simple_value!(InvocationTags);
#[starlark_value(type = "bazel_module_tags")]
impl<'v> StarlarkValue<'v> for InvocationTags {
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        self.classes.iter().any(|class| class == name).then(|| {
            heap.alloc_simple(InvocationTagList(
                self.tags
                    .iter()
                    .filter(|tag| tag.tag_class == name)
                    .cloned()
                    .map(InvocationTag::from)
                    .map(|mut tag| {
                        tag.owner = self.owner.clone();
                        tag
                    })
                    .collect::<Vec<_>>()
                    .into(),
            ))
        })
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct InvocationTag {
    class: CompactString,
    attributes: Arc<[(CompactString, CoercedAttributeValue)]>,
    dev_dependency: bool,
    location: slug_bzlmod_v2::LogicalSpan,
    module_index: usize,
    tag_index: usize,
    #[allocative(skip)]
    owner: Arc<()>,
}
impl From<PreparedModuleExtensionTag> for InvocationTag {
    fn from(tag: PreparedModuleExtensionTag) -> Self {
        Self {
            class: tag.tag_class,
            attributes: tag.attributes,
            dev_dependency: tag.dev_dependency,
            location: tag.location,
            module_index: tag.module_index,
            tag_index: tag.tag_index,
            owner: Arc::new(()),
        }
    }
}
impl fmt::Display for InvocationTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "'{}' tag at {}:{}:{}",
            self.class, self.location.file.0, self.location.start_line, self.location.start_column
        )
    }
}
starlark::starlark_simple_value!(InvocationTag);
#[starlark_value(type = "bazel_module_tag")]
impl<'v> StarlarkValue<'v> for InvocationTag {
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        self.attributes.iter().find_map(|(attribute, value)| {
            (attribute == name).then(|| allocate_attribute(value, heap))
        })
    }
}

fn allocate_attribute<'v>(value: &CoercedAttributeValue, heap: Heap<'v>) -> Value<'v> {
    match value {
        CoercedAttributeValue::String(value) => heap.alloc_str(value).to_value(),
        CoercedAttributeValue::Boolean(value) => Value::new_bool(*value),
        CoercedAttributeValue::Integer(value) => heap.alloc(*value),
        CoercedAttributeValue::Label(value) => heap.alloc_simple(InvocationLabel(value.clone())),
        CoercedAttributeValue::None => Value::new_none(),
        _ => unreachable!("preparation rejects non-scalar module-extension attributes"),
    }
}

fn list_index(index: i32, len: usize) -> starlark::Result<usize> {
    let index = if index < 0 {
        len as i64 + index as i64
    } else {
        index as i64
    };
    (index >= 0 && index < len as i64)
        .then_some(index as usize)
        .ok_or_else(|| starlark::Error::new_other(anyhow::anyhow!("list index out of range")))
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct InvocationModuleList(Arc<[InvocationModule]>);
impl fmt::Display for InvocationModuleList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[<bazel_module>]")
    }
}
starlark::starlark_simple_value!(InvocationModuleList);
#[starlark_value(type = "list")]
impl<'v> StarlarkValue<'v> for InvocationModuleList {
    fn at(&self, index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let index = index
            .unpack_i32()
            .ok_or_else(|| starlark::Error::new_other(anyhow::anyhow!("list index must be int")))?;
        self.0
            .get(list_index(index, self.0.len())?)
            .cloned()
            .map(|v| heap.alloc_simple(v))
            .ok_or_else(|| starlark::Error::new_other(anyhow::anyhow!("list index out of range")))
    }
    fn length(&self) -> starlark::Result<i32> {
        Ok(self.0.len() as i32)
    }
    fn iterate_collect(&self, heap: Heap<'v>) -> starlark::Result<Vec<Value<'v>>> {
        Ok(self
            .0
            .iter()
            .cloned()
            .map(|v| heap.alloc_simple(v))
            .collect())
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct InvocationTagList(Arc<[InvocationTag]>);
impl fmt::Display for InvocationTagList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[<bazel_module_tag>]")
    }
}
starlark::starlark_simple_value!(InvocationTagList);
#[starlark_value(type = "list")]
impl<'v> StarlarkValue<'v> for InvocationTagList {
    fn at(&self, index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let index = index
            .unpack_i32()
            .ok_or_else(|| starlark::Error::new_other(anyhow::anyhow!("list index must be int")))?;
        self.0
            .get(list_index(index, self.0.len())?)
            .cloned()
            .map(|v| heap.alloc_simple(v))
            .ok_or_else(|| starlark::Error::new_other(anyhow::anyhow!("list index out of range")))
    }
    fn length(&self) -> starlark::Result<i32> {
        Ok(self.0.len() as i32)
    }
    fn iterate_collect(&self, heap: Heap<'v>) -> starlark::Result<Vec<Value<'v>>> {
        Ok(self
            .0
            .iter()
            .cloned()
            .map(|v| heap.alloc_simple(v))
            .collect())
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct InvocationTagSortKey(usize, usize);
impl fmt::Display for InvocationTagSortKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<sort_key>")
    }
}
starlark::starlark_simple_value!(InvocationTagSortKey);
#[starlark_value(type = "sort_key")]
impl<'v> StarlarkValue<'v> for InvocationTagSortKey {
    fn compare(&self, other: Value<'v>) -> starlark::Result<Ordering> {
        let other = Self::from_value(other).ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!("sort keys can only compare with sort keys"))
        })?;
        Ok((self.0, self.1).cmp(&(other.0, other.1)))
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct InvocationLabel(CanonicalLabel);
impl fmt::Display for InvocationLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
starlark::starlark_simple_value!(InvocationLabel);
#[starlark_value(type = "Label")]
impl<'v> StarlarkValue<'v> for InvocationLabel {
    fn collect_str(&self, collector: &mut String) {
        collector.push_str(&self.0.to_string());
    }
    fn collect_repr(&self, collector: &mut String) {
        collector.push_str("Label(\"");
        collector.push_str(&self.0.to_string());
        collector.push_str("\")");
    }
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.0.hash(hasher);
        Ok(())
    }
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other).is_some_and(|other| self.0 == other.0))
    }
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match name {
            "name" => Some(heap.alloc_str(self.0.target().as_str()).to_value()),
            "package" => Some(
                heap.alloc_str(self.0.package().package().as_str())
                    .to_value(),
            ),
            "repo_name" | "workspace_name" => {
                Some(heap.alloc_str(self.0.package().repo().as_str()).to_value())
            }
            _ => None,
        }
    }
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(invocation_label_methods)
    }
}

#[starlark_module]
fn invocation_label_methods(builder: &mut MethodsBuilder) {
    fn same_package_label(this: Value, target_name: &str) -> anyhow::Result<InvocationLabel> {
        let this = InvocationLabel::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid Label receiver"))?;
        let label = CanonicalLabel::parse(&format!("{}:{}", this.0.package(), target_name))
            .map_err(anyhow::Error::msg)?;
        Ok(InvocationLabel(label))
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    #[derive(Debug, Clone, Allocative)]
    pub(crate) struct InvokePreparedKey {
        pub(crate) workspace: NormalizedAbsolutePath,
        pub(crate) prepared: Arc<HostPreparedModuleExtensionInputs>,
        pub(crate) id: u64,
    }

    impl PartialEq for InvokePreparedKey {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }
    impl Eq for InvokePreparedKey {}
    impl std::hash::Hash for InvokePreparedKey {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            self.id.hash(state);
        }
    }
    impl fmt::Display for InvokePreparedKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "test-invoke-prepared:{}", self.id)
        }
    }

    #[derive(Debug, Clone, Allocative)]
    pub(crate) struct InvokePreparedValue {
        pub(crate) outcome: HostPureModuleExtensionInvocationsOutcome,
        pub(crate) prints: Arc<[CompactString]>,
    }

    #[async_trait]
    impl Key for InvokePreparedKey {
        type Value = Arc<InvokePreparedValue>;

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _: &CancellationContext,
        ) -> Self::Value {
            let mut events = EventBatch::empty();
            let outcome = invoke_all(
                ctx,
                self.workspace.clone(),
                self.prepared.clone(),
                true,
                &mut events,
            )
            .await;
            let prints = events
                .events()
                .iter()
                .filter_map(|event| match event {
                    EvaluationEvent::StarlarkPrint { text, .. } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .into();
            Arc::new(InvokePreparedValue { outcome, prints })
        }

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            x.prints == y.prints
                && HostPureModuleExtensionInvocationsKey::equality(&x.outcome, &y.outcome)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
    pub(crate) struct InvocationConsumerKey {
        pub(crate) workspace: NormalizedAbsolutePath,
        pub(crate) id: u64,
    }
    impl fmt::Display for InvocationConsumerKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "test-invocation-consumer:{}", self.id)
        }
    }
    #[async_trait]
    impl Key for InvocationConsumerKey {
        type Value = HostPureModuleExtensionInvocationsOutcome;

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _: &CancellationContext,
        ) -> Self::Value {
            ctx.compute(&HostPureModuleExtensionInvocationsKey::new(
                self.workspace.clone(),
            ))
            .await
            .unwrap()
        }

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            x.complete_eq(y)
        }
    }
}

#[cfg(test)]
mod tests {
    use starlark::environment::Globals;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    use super::*;

    fn empty_module(owner: &Arc<()>) -> InvocationModule {
        InvocationModule {
            name: "root".into(),
            version: "".into(),
            is_root: true,
            tag_classes: Arc::from([CompactString::from("tag")]),
            tags: Arc::from([]),
            owner: owner.clone(),
        }
    }

    fn tag(owner: &Arc<()>, index: usize, dev_dependency: bool) -> InvocationTag {
        InvocationTag {
            class: "tag".into(),
            attributes: Arc::from([(
                CompactString::from("value"),
                CoercedAttributeValue::String(format!("v{index}").into()),
            )]),
            dev_dependency,
            location: slug_bzlmod_v2::LogicalSpan {
                file: slug_bzlmod_v2::LogicalModuleFileId::new("MODULE.bazel"),
                start_line: index as u32 + 1,
                start_column: 1,
                end_line: index as u32 + 1,
                end_column: 2,
            },
            module_index: 0,
            tag_index: index,
            owner: owner.clone(),
        }
    }

    fn prepared_tag(index: usize, dev_dependency: bool) -> PreparedModuleExtensionTag {
        PreparedModuleExtensionTag {
            tag_class: "tag".into(),
            attributes: Arc::from([(
                CompactString::from("value"),
                CoercedAttributeValue::String(format!("v{index}").into()),
            )]),
            dev_dependency,
            location: slug_bzlmod_v2::LogicalSpan {
                file: slug_bzlmod_v2::LogicalModuleFileId::new("MODULE.bazel"),
                start_line: index as u32 + 1,
                start_column: 1,
                end_line: index as u32 + 1,
                end_column: 2,
            },
            module_index: 0,
            tag_index: index,
        }
    }

    fn call(
        source: &str,
        values: impl FnOnce(&Module) -> Vec<Value<'_>>,
    ) -> Result<String, String> {
        let module = Module::new();
        let ast = AstModule::parse("test.bzl", source.to_owned(), &Dialect::Standard).unwrap();
        let mut evaluator = Evaluator::new(&module);
        evaluator
            .eval_module(ast, &Globals::standard())
            .map_err(|error| error.to_string())?;
        let function = module.get("f").unwrap();
        let values = values(&module);
        evaluator
            .eval_function(function, &values, &[])
            .map(|value| value.to_repr())
            .map_err(|error| error.to_string())
    }

    #[test]
    fn immutable_lists_and_foreign_tags_fail_closed() {
        let owner = Arc::new(());
        let context = InvocationContext {
            module: empty_module(&owner),
            owner: owner.clone(),
        };
        let modules = call("def f(ctx):\n  ctx.modules.append(1)\n", |module| {
            vec![module.heap().alloc_simple(context.clone())]
        });
        assert!(modules.unwrap_err().contains("append"));

        let tag_lists = call(
            "def f(ctx):\n  ctx.modules[0].tags.tag.append(1)\n",
            |module| vec![module.heap().alloc_simple(context.clone())],
        );
        assert!(tag_lists.unwrap_err().contains("append"));

        let foreign = InvocationTag {
            class: "tag".into(),
            attributes: Arc::from([]),
            dev_dependency: true,
            location: slug_bzlmod_v2::LogicalSpan {
                file: slug_bzlmod_v2::LogicalModuleFileId::new("MODULE.bazel"),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 2,
            },
            module_index: 0,
            tag_index: 0,
            owner: Arc::new(()),
        };
        let error = call(
            "def f(ctx, tag):\n  return ctx.is_dev_dependency(tag)\n",
            |module| {
                vec![
                    module.heap().alloc_simple(context),
                    module.heap().alloc_simple(foreign),
                ]
            },
        )
        .unwrap_err();
        assert!(error.contains("another module_ctx"));

        let owner = Arc::new(());
        let tags: Arc<[PreparedModuleExtensionTag]> = Arc::from([]);
        let context = InvocationContext {
            module: InvocationModule {
                tags,
                ..empty_module(&owner)
            },
            owner: owner.clone(),
        };
        let foreign = tag(&Arc::new(()), 0, false);
        let error = call(
            "def f(ctx, tag):\n  return ctx.tag_sort_key(tag)\n",
            |module| {
                vec![
                    module.heap().alloc_simple(context),
                    module.heap().alloc_simple(foreign),
                ]
            },
        )
        .unwrap_err();
        assert!(error.contains("another module_ctx"));
    }

    #[test]
    fn immutable_lists_support_exact_negative_indexing_and_tag_order() {
        let owner = Arc::new(());
        let tags: Arc<[InvocationTag]> = Arc::from([tag(&owner, 0, false), tag(&owner, 1, true)]);
        let modules = InvocationModuleList(Arc::from([empty_module(&owner)]));
        let value = call(
            "def f(modules, tags):\n  return [modules[-1].name, tags[-1].value, tags[-2].value]\n",
            |module| {
                vec![
                    module.heap().alloc_simple(modules.clone()),
                    module.heap().alloc_simple(InvocationTagList(tags.clone())),
                ]
            },
        )
        .unwrap();
        assert_eq!(value, "[\"root\", \"v1\", \"v0\"]");
        for source in [
            "def f(modules, tags):\n  return modules[-2]\n",
            "def f(modules, tags):\n  return tags[-3]\n",
        ] {
            let error = call(source, |module| {
                vec![
                    module.heap().alloc_simple(modules.clone()),
                    module.heap().alloc_simple(InvocationTagList(tags.clone())),
                ]
            })
            .unwrap_err();
            assert!(error.contains("list index out of range"), "{error}");
        }
        let context = InvocationContext {
            module: InvocationModule {
                tags: Arc::from([prepared_tag(0, false), prepared_tag(1, true)]),
                ..empty_module(&owner)
            },
            owner: owner.clone(),
        };
        let value = call(
            "def f(ctx):\n  tags=ctx.modules[0].tags.tag\n  return [len(tags), ctx.is_dev_dependency(tags[-1]), ctx.tag_sort_key(tags[0]) < ctx.tag_sort_key(tags[1])]\n",
            |module| vec![module.heap().alloc_simple(context)],
        )
        .unwrap();
        assert_eq!(value, "[2, True, True]");
    }

    #[test]
    fn forbidden_abi_and_cross_context_captured_tags_fail_closed() {
        let owner = Arc::new(());
        let context = InvocationContext {
            module: InvocationModule {
                tags: Arc::from([prepared_tag(0, false)]),
                ..empty_module(&owner)
            },
            owner,
        };
        for name in [
            "facts",
            "is_isolated",
            "root_module_has_non_dev_dependency",
            "extension_metadata",
            "wait",
            "download",
            "download_and_extract",
            "extract",
            "file",
            "getenv",
            "path",
            "read",
            "watch",
            "report_progress",
            "os",
            "execute",
            "load_wasm",
            "execute_wasm",
            "which",
        ] {
            let source = format!("def f(ctx):\n  return ctx.{name}\n");
            assert!(
                call(&source, |module| vec![
                    module.heap().alloc_simple(context.clone())
                ])
                .unwrap_err()
                .contains("has no attribute")
            );
        }
        for source in [
            "def f(ctx):\n  return ctx.modules[0].missing\n",
            "def f(ctx):\n  return ctx.modules[0].tags.missing\n",
            "def f(ctx):\n  return ctx.modules[0].tags.tag[0].missing\n",
        ] {
            assert!(
                call(source, |module| vec![
                    module.heap().alloc_simple(context.clone())
                ])
                .is_err()
            );
        }
        let other_owner = Arc::new(());
        let other = InvocationContext {
            module: InvocationModule {
                tags: Arc::from([prepared_tag(0, true)]),
                ..empty_module(&other_owner)
            },
            owner: other_owner,
        };
        for method in ["is_dev_dependency", "tag_sort_key"] {
            let source = format!(
                "def f(left, right):\n  tag=right.modules[0].tags.tag[0]\n  return left.{method}(tag)\n"
            );
            let error = call(&source, |module| {
                vec![
                    module.heap().alloc_simple(context.clone()),
                    module.heap().alloc_simple(other.clone()),
                ]
            })
            .unwrap_err();
            assert!(error.contains("another module_ctx"), "{error}");
        }
        let label = InvocationLabel(CanonicalLabel::parse("@@//pkg:item").unwrap());
        for name in ["workspace_root", "relative"] {
            let source = format!("def f(label):\n  return label.{name}\n");
            assert!(
                call(&source, |module| vec![
                    module.heap().alloc_simple(label.clone())
                ])
                .is_err()
            );
        }
    }

    #[test]
    fn scalar_none_label_and_exact_read_only_abi_are_visible() {
        let module = Module::new();
        assert!(allocate_attribute(&CoercedAttributeValue::None, module.heap()).is_none());
        let owner = Arc::new(());
        let label = CanonicalLabel::parse("@@dep+//pkg:item").unwrap();
        let tag = InvocationTag {
            class: "tag".into(),
            attributes: Arc::from([(
                CompactString::from("target"),
                CoercedAttributeValue::Label(label),
            )]),
            dev_dependency: true,
            location: slug_bzlmod_v2::LogicalSpan {
                file: slug_bzlmod_v2::LogicalModuleFileId::new("MODULE.bazel"),
                start_line: 3,
                start_column: 2,
                end_line: 3,
                end_column: 4,
            },
            module_index: 0,
            tag_index: 0,
            owner: owner.clone(),
        };
        let context = InvocationContext {
            module: InvocationModule {
                name: "root".into(),
                version: "".into(),
                is_root: true,
                tag_classes: Arc::from([CompactString::from("tag")]),
                tags: Arc::from([]),
                owner: owner.clone(),
            },
            owner,
        };
        let value = call("def f(ctx, tag):\n  label=tag.target\n  return [len(ctx.modules), ctx.modules[0].name, ctx.modules[0].version, ctx.modules[0].is_root, ctx.is_dev_dependency(tag), label.name, label.package, label.repo_name, label.workspace_name, str(label), repr(label), '%s' % label, '%r' % label, '{}'.format(label), '{!s}'.format(label), '{!r}'.format(label), label.same_package_label('other').name, {label: 1}[label], label == label]\n", |module| vec![module.heap().alloc_simple(context), module.heap().alloc_simple(tag)]).unwrap();
        assert_eq!(
            value,
            "[1, \"root\", \"\", True, True, \"item\", \"pkg\", \"dep+\", \"dep+\", \"@@dep+//pkg:item\", \"Label(\\\"@@dep+//pkg:item\\\")\", \"@@dep+//pkg:item\", \"Label(\\\"@@dep+//pkg:item\\\")\", \"@@dep+//pkg:item\", \"@@dep+//pkg:item\", \"Label(\\\"@@dep+//pkg:item\\\")\", \"other\", 1, True]"
        );
    }
}
