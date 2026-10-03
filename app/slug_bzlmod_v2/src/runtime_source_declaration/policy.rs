use std::fmt;
use std::mem::size_of;
use std::path::Path;
use std::path::PathBuf;

use allocative::Allocative;
use dupe::Dupe;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ImageChargedPath;
use slug_workspace_v2::ImageDescriptorBudget;
use slug_workspace_v2::ImageDescriptorBudgetError;
use slug_workspace_v2::InputImageAliasPolicy;
use slug_workspace_v2::InputImageLimits;
use slug_workspace_v2::InputImagePolicy;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::types::*;

#[derive(Debug, Eq, PartialEq, Hash, Allocative)]
enum Selector {
    Host(ImageChargedPath),
    Workspace(BudgetedImageBlock<Vec<RuntimeSourceComponent>>),
    Label(
        crate::SelectedToolRole,
        BudgetedImageBlock<Vec<RuntimeSourceComponent>>,
    ),
}
#[derive(Debug, Clone, Dupe, Eq, PartialEq, Hash, Allocative)]
enum GuestMapping {
    OriginalSource,
    ExplicitPrefix(RuntimeSourceGuestPrefix),
}
#[derive(Debug, Clone, Dupe, Eq, PartialEq, Hash, Allocative)]
pub struct RuntimeSourceDeclaration {
    source: BudgetedImageBlock<Selector>,
    guest: GuestMapping,
    image_policy: InputImagePolicy,
}
#[derive(Debug, Clone, Copy)]
pub enum RuntimeSourceSelectorRef<'a> {
    ExplicitHostDirectory {
        anchor: &'a ImageChargedPath,
    },
    WorkspaceRelativeDirectory {
        components: &'a [RuntimeSourceComponent],
    },
    SelectedToolOriginLabelParent {
        role: crate::SelectedToolRole,
        components: &'a [RuntimeSourceComponent],
    },
}
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub enum RuntimeSourceGuestMappingRef<'a> {
    OriginalSource,
    ExplicitPrefix(&'a RuntimeSourceGuestPrefix),
}
impl RuntimeSourceDeclaration {
    pub fn selector(&self) -> RuntimeSourceSelectorRef<'_> {
        match self.source.get() {
            Selector::Host(anchor) => RuntimeSourceSelectorRef::ExplicitHostDirectory { anchor },
            Selector::Workspace(rows) => RuntimeSourceSelectorRef::WorkspaceRelativeDirectory {
                components: rows.get(),
            },
            Selector::Label(role, rows) => {
                RuntimeSourceSelectorRef::SelectedToolOriginLabelParent {
                    role: *role,
                    components: rows.get(),
                }
            }
        }
    }
    #[doc(hidden)]
    pub fn guest_mapping(&self) -> RuntimeSourceGuestMappingRef<'_> {
        match &self.guest {
            GuestMapping::OriginalSource => RuntimeSourceGuestMappingRef::OriginalSource,
            GuestMapping::ExplicitPrefix(prefix) => {
                RuntimeSourceGuestMappingRef::ExplicitPrefix(prefix)
            }
        }
    }
    pub fn image_policy(&self) -> InputImagePolicy {
        self.image_policy
    }
    fn same_budget(&self, budget: &ImageDescriptorBudget) -> bool {
        self.source.same_budget(budget)
            && match &self.guest {
                GuestMapping::OriginalSource => true,
                GuestMapping::ExplicitPrefix(p) => p.same_budget(budget),
            }
            && match self.source.get() {
                Selector::Host(p) => p.same_budget(budget),
                Selector::Workspace(rows) | Selector::Label(_, rows) => {
                    rows.same_budget(budget) && rows.get().iter().all(|r| r.same_budget(budget))
                }
            }
    }
}
#[derive(Eq, PartialEq, Hash, Allocative)]
struct PolicyInner {
    workspace: ImageChargedPath,
    rows: [Option<RuntimeSourceDeclaration>; 64],
    len: usize,
    limits: RuntimeSourceLimits,
}
#[derive(Clone, Dupe, Eq, PartialEq, Hash, Allocative)]
pub struct RuntimeSourceDeclarationPolicy(BudgetedImageBlock<PolicyInner>);
impl fmt::Debug for RuntimeSourceDeclarationPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeSourceDeclarationPolicy")
            .field("declarations", &self.0.get().len)
            .finish_non_exhaustive()
    }
}
impl RuntimeSourceDeclarationPolicy {
    pub fn new(
        workspace: &ImageChargedPath,
        declarations: &[RuntimeSourceDeclarationInput<'_>],
        limits: RuntimeSourceLimits,
        budget: &ImageDescriptorBudget,
    ) -> Result<Self, RuntimeSourceDeclarationError> {
        if !workspace.same_budget(budget) {
            return Err(ImageDescriptorBudgetError::ForeignBudget.into());
        }
        limits.validate()?;
        if declarations.len() > limits.max_declarations {
            return Err(RuntimeSourceDeclarationError::LimitExceeded(
                RuntimeSourceLimit::Declarations,
            ));
        }
        // Validate the WHOLE borrowed input before allocating any owned row.
        let mut bytes = 0usize;
        for (i, row) in declarations.iter().enumerate() {
            let guest_bytes = match row.guest_mapping {
                RuntimeSourceGuestMappingInput::OriginalSource => 0,
                RuntimeSourceGuestMappingInput::ExplicitPrefix(prefix) => {
                    guest_prefix(prefix, &limits)?;
                    prefix.len()
                }
            };
            let (text, relative) = source_text(row.source);
            let component_count = if relative {
                relative_path(text, &limits)?
            } else {
                absolute(text, &limits)?;
                if overlaps(Path::new(text), workspace.as_path()) {
                    return Err(RuntimeSourceDeclarationError::Overlap);
                }
                0
            };
            let row_bytes = add(
                add(guest_bytes, text.len())?,
                add(64, mul(component_count, 16)?)?,
            )?;
            bytes = add(bytes, row_bytes)?;
            if bytes > limits.max_declaration_bytes {
                return Err(RuntimeSourceDeclarationError::LimitExceeded(
                    RuntimeSourceLimit::DeclarationBytes,
                ));
            }
            for earlier in &declarations[..i] {
                let known_guest_overlap = match (row.guest_mapping, earlier.guest_mapping) {
                    (
                        RuntimeSourceGuestMappingInput::ExplicitPrefix(a),
                        RuntimeSourceGuestMappingInput::ExplicitPrefix(b),
                    ) => overlaps(Path::new(a), Path::new(b)),
                    _ => false, // Normal Requests owns source-derived original mappings.
                };
                if known_guest_overlap || same_source_overlap(row.source, earlier.source) {
                    return Err(RuntimeSourceDeclarationError::Overlap);
                }
            }
        }
        let image_policy = InputImagePolicy::new(
            InputImageAliasPolicy::Deny,
            InputImageLimits {
                max_entries: limits.max_entries,
                max_depth: limits.max_depth,
                max_total_file_bytes: limits.max_total_file_bytes,
                max_file_bytes: limits.max_file_bytes,
                max_descriptor_bytes: limits.max_image_descriptor_bytes,
                max_path_bytes: limits.max_path_bytes,
                ..InputImageLimits::default()
            },
        )
        .map_err(|_| RuntimeSourceDeclarationError::InvalidDeclaration)?;
        let mut rows = std::array::from_fn(|_| None);
        for (i, row) in declarations.iter().enumerate() {
            let selector = match row.source {
                RuntimeSourceSelectorInput::ExplicitHostDirectory { anchor } => {
                    Selector::Host(host_path(anchor, budget)?)
                }
                RuntimeSourceSelectorInput::WorkspaceRelativeDirectory { relative } => {
                    Selector::Workspace(components(relative, budget)?)
                }
                RuntimeSourceSelectorInput::SelectedToolOriginLabelParent { role, children } => {
                    Selector::Label(role, components(children, budget)?)
                }
            };
            rows[i] = Some(RuntimeSourceDeclaration {
                source: budget.try_reserve(0)?.freeze(selector)?,
                guest: match row.guest_mapping {
                    RuntimeSourceGuestMappingInput::OriginalSource => GuestMapping::OriginalSource,
                    RuntimeSourceGuestMappingInput::ExplicitPrefix(prefix) => {
                        GuestMapping::ExplicitPrefix(RuntimeSourceGuestPrefix::new(
                            prefix, limits, budget,
                        )?)
                    }
                },
                image_policy,
            });
        }
        Ok(Self(budget.try_reserve(0)?.freeze(PolicyInner {
            workspace: workspace.dupe(),
            rows,
            len: declarations.len(),
            limits,
        })?))
    }
    pub fn workspace(&self) -> &ImageChargedPath {
        &self.0.get().workspace
    }
    pub fn declarations(&self) -> impl ExactSizeIterator<Item = &RuntimeSourceDeclaration> {
        self.0.get().rows[..self.0.get().len]
            .iter()
            .map(|r| r.as_ref().unwrap())
    }
    pub fn limits(&self) -> RuntimeSourceLimits {
        self.0.get().limits
    }
    pub fn platform(&self) -> RuntimeSourcePlatform {
        RuntimeSourcePlatform::LinuxX86_64
    }
    pub fn fact_tier(&self) -> RuntimeSourceFactTier {
        RuntimeSourceFactTier::TreeOnly
    }
    pub fn same_budget(&self, budget: &ImageDescriptorBudget) -> bool {
        self.0.same_budget(budget)
            && self.workspace().same_budget(budget)
            && self.declarations().all(|r| r.same_budget(budget))
    }
}
impl RuntimeSourceGuestPrefix {
    /// Resolve a finite guest spelling after normal source selection. This
    /// configuration value grants no native source or descriptor authority.
    #[doc(hidden)]
    pub fn new(
        text: &str,
        limits: RuntimeSourceLimits,
        budget: &ImageDescriptorBudget,
    ) -> Result<Self, RuntimeSourceDeclarationError> {
        limits.validate()?;
        guest_prefix(text, &limits)?;
        Ok(Self(text_block(text, budget)?))
    }
}
fn guest_prefix(
    text: &str,
    limits: &RuntimeSourceLimits,
) -> Result<(), RuntimeSourceDeclarationError> {
    absolute(text, limits)?;
    if overlaps(Path::new(text), Path::new("/__slug_module")) {
        return Err(RuntimeSourceDeclarationError::Overlap);
    }
    Ok(())
}
fn source_text<'a>(input: RuntimeSourceSelectorInput<'a>) -> (&'a str, bool) {
    match input {
        RuntimeSourceSelectorInput::ExplicitHostDirectory { anchor } => (anchor, false),
        RuntimeSourceSelectorInput::WorkspaceRelativeDirectory { relative } => (relative, true),
        RuntimeSourceSelectorInput::SelectedToolOriginLabelParent { children, .. } => {
            (children, true)
        }
    }
}
fn overlaps(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}
fn same_source_overlap(
    a: RuntimeSourceSelectorInput<'_>,
    b: RuntimeSourceSelectorInput<'_>,
) -> bool {
    match (a, b) {
        (
            RuntimeSourceSelectorInput::ExplicitHostDirectory { anchor: a },
            RuntimeSourceSelectorInput::ExplicitHostDirectory { anchor: b },
        ) => overlaps(Path::new(a), Path::new(b)),
        (
            RuntimeSourceSelectorInput::WorkspaceRelativeDirectory { relative: a },
            RuntimeSourceSelectorInput::WorkspaceRelativeDirectory { relative: b },
        ) => overlaps(Path::new(a), Path::new(b)),
        (
            RuntimeSourceSelectorInput::SelectedToolOriginLabelParent {
                role: a,
                children: ac,
            },
            RuntimeSourceSelectorInput::SelectedToolOriginLabelParent {
                role: b,
                children: bc,
            },
        ) if a == b => overlaps(Path::new(ac), Path::new(bc)),
        _ => false, // Different resolved owners receive an additional normal-plan check.
    }
}
fn absolute(text: &str, limits: &RuntimeSourceLimits) -> Result<(), RuntimeSourceDeclarationError> {
    if !text.starts_with('/') || text == "/" {
        return Err(RuntimeSourceDeclarationError::InvalidDeclaration);
    }
    relative_path(&text[1..], limits)?;
    path_len(text, limits)
}
fn relative_path(
    text: &str,
    limits: &RuntimeSourceLimits,
) -> Result<usize, RuntimeSourceDeclarationError> {
    path_len(text, limits)?;
    if text.is_empty() {
        return Ok(0);
    }
    let mut n = 0;
    for part in text.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains(['\\', '\0', ':']) {
            return Err(RuntimeSourceDeclarationError::InvalidDeclaration);
        }
        n += 1;
        if n > limits.max_components {
            return Err(RuntimeSourceDeclarationError::LimitExceeded(
                RuntimeSourceLimit::Components,
            ));
        }
    }
    Ok(n)
}
fn path_len(text: &str, limits: &RuntimeSourceLimits) -> Result<(), RuntimeSourceDeclarationError> {
    if text.len() > limits.max_path_bytes {
        Err(RuntimeSourceDeclarationError::LimitExceeded(
            RuntimeSourceLimit::PathBytes,
        ))
    } else {
        Ok(())
    }
}
fn components(
    text: &str,
    budget: &ImageDescriptorBudget,
) -> Result<BudgetedImageBlock<Vec<RuntimeSourceComponent>>, RuntimeSourceDeclarationError> {
    let count = if text.is_empty() {
        0
    } else {
        text.split('/').count()
    };
    let credit = budget.try_reserve(mul(count, size_of::<RuntimeSourceComponent>())?)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| ImageDescriptorBudgetError::ResourceLimit)?;
    if !text.is_empty() {
        for part in text.split('/') {
            rows.push(RuntimeSourceComponent(text_block(part, budget)?));
        }
    }
    Ok(credit.freeze(rows)?)
}
fn text_block(
    text: &str,
    budget: &ImageDescriptorBudget,
) -> Result<BudgetedImageBlock<String>, RuntimeSourceDeclarationError> {
    let credit = budget.try_reserve(text.len())?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(text.len())
        .map_err(|_| ImageDescriptorBudgetError::ResourceLimit)?;
    owned.push_str(text);
    Ok(credit.freeze(owned)?)
}
fn host_path(
    text: &str,
    budget: &ImageDescriptorBudget,
) -> Result<ImageChargedPath, RuntimeSourceDeclarationError> {
    // Input PathBuf, lexical normalizer replacement peaks and fixed temporary
    // headers are reserved BEFORE either path allocation. Strict validation has
    // already guaranteed that normalization cannot change the spelling.
    let _peak = budget.try_reserve(add(mul(text.len(), 6)?, 512)?)?;
    let mut path = PathBuf::new();
    path.try_reserve_exact(text.len())
        .map_err(|_| ImageDescriptorBudgetError::ResourceLimit)?;
    path.push(text);
    let normalized = NormalizedAbsolutePath::new(path)
        .map_err(|_| RuntimeSourceDeclarationError::InvalidDeclaration)?;
    Ok(ImageChargedPath::pin_existing(normalized, budget)?)
}
fn add(a: usize, b: usize) -> Result<usize, RuntimeSourceDeclarationError> {
    a.checked_add(b)
        .ok_or_else(|| ImageDescriptorBudgetError::ChargeOverflow.into())
}
fn mul(a: usize, b: usize) -> Result<usize, RuntimeSourceDeclarationError> {
    a.checked_mul(b)
        .ok_or_else(|| ImageDescriptorBudgetError::ChargeOverflow.into())
}
