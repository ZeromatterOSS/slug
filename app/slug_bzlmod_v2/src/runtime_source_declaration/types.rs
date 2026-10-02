use std::fmt;
use std::path::Path;

use allocative::Allocative;
use dupe::Dupe;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ImageDescriptorBudget;
use slug_workspace_v2::ImageDescriptorBudgetError;

#[derive(Debug, Clone, Copy, Dupe, Eq, PartialEq, Hash, Allocative)]
pub enum RuntimeSourcePlatform {
    LinuxX86_64,
}
#[derive(Debug, Clone, Copy, Dupe, Eq, PartialEq, Hash, Allocative)]
pub enum RuntimeSourceFactTier {
    TreeOnly,
}
#[derive(Debug, Clone, Copy, Dupe, Eq, PartialEq, Hash, Allocative)]
pub struct RuntimeSourceLimits {
    pub max_declarations: usize,
    pub max_path_bytes: usize,
    pub max_components: usize,
    pub max_declaration_bytes: usize,
    pub max_entries: usize,
    pub max_depth: usize,
    pub max_total_file_bytes: u64,
    pub max_file_bytes: u64,
    pub max_image_descriptor_bytes: usize,
}
impl Default for RuntimeSourceLimits {
    fn default() -> Self {
        Self {
            max_declarations: 64,
            max_path_bytes: 4096,
            max_components: 64,
            max_declaration_bytes: 4 << 20,
            max_entries: 32_768,
            max_depth: 32,
            max_total_file_bytes: 2 << 30,
            max_file_bytes: 512 << 20,
            max_image_descriptor_bytes: 4 << 20,
        }
    }
}
impl RuntimeSourceLimits {
    pub(super) fn validate(self) -> Result<(), RuntimeSourceDeclarationError> {
        let c = Self::default();
        if self.max_declarations == 0
            || self.max_declarations > c.max_declarations
            || self.max_path_bytes == 0
            || self.max_path_bytes > c.max_path_bytes
            || self.max_components > c.max_components
            || self.max_declaration_bytes > c.max_declaration_bytes
            || self.max_entries == 0
            || self.max_entries > c.max_entries
            || self.max_depth > c.max_depth
            || self.max_total_file_bytes > c.max_total_file_bytes
            || self.max_file_bytes > c.max_file_bytes
            || self.max_image_descriptor_bytes > c.max_image_descriptor_bytes
        {
            return Err(RuntimeSourceDeclarationError::InvalidDeclaration);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, Dupe, Eq, PartialEq, Allocative)]
pub enum RuntimeSourceLimit {
    Declarations,
    PathBytes,
    Components,
    DeclarationBytes,
}
#[derive(Debug, Clone, Copy, Dupe, Eq, PartialEq, Allocative)]
pub enum RuntimeSourceDeclarationError {
    InvalidDeclaration,
    LimitExceeded(RuntimeSourceLimit),
    UnsupportedPlatform,
    UnsupportedSource,
    MissingDeclaration,
    Overlap,
    Resource(ImageDescriptorBudgetError),
    UnauthorizedCurrentPolicy,
    Operational,
}
impl RuntimeSourceDeclarationError {
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::Resource(_) | Self::UnauthorizedCurrentPolicy | Self::Operational
        )
    }
}
impl From<ImageDescriptorBudgetError> for RuntimeSourceDeclarationError {
    fn from(value: ImageDescriptorBudgetError) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for RuntimeSourceDeclarationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "runtime source declaration: {self:?}")
    }
}
impl std::error::Error for RuntimeSourceDeclarationError {}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeSourceDeclarationInput<'a> {
    pub source: RuntimeSourceSelectorInput<'a>,
    pub guest_prefix: &'a str,
}
#[derive(Debug, Clone, Copy)]
pub enum RuntimeSourceSelectorInput<'a> {
    ExplicitHostDirectory {
        anchor: &'a str,
    },
    WorkspaceRelativeDirectory {
        relative: &'a str,
    },
    SelectedToolOriginLabelParent {
        role: crate::SelectedToolRole,
        children: &'a str,
    },
}

#[derive(Clone, Dupe, Eq, PartialEq, Hash, Allocative)]
pub struct RuntimeSourceComponent(pub(super) BudgetedImageBlock<String>);
impl RuntimeSourceComponent {
    pub fn as_str(&self) -> &str {
        self.0.get()
    }
    pub(super) fn same_budget(&self, budget: &ImageDescriptorBudget) -> bool {
        self.0.same_budget(budget)
    }
}
impl fmt::Debug for RuntimeSourceComponent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RuntimeSourceComponent")
    }
}
#[derive(Clone, Dupe, Eq, PartialEq, Hash, Allocative)]
pub struct RuntimeSourceGuestPrefix(pub(super) BudgetedImageBlock<String>);
impl RuntimeSourceGuestPrefix {
    pub fn as_str(&self) -> &str {
        self.0.get()
    }
    pub fn as_path(&self) -> &Path {
        Path::new(self.as_str())
    }
    pub(super) fn same_budget(&self, budget: &ImageDescriptorBudget) -> bool {
        self.0.same_budget(budget)
    }
}
impl fmt::Debug for RuntimeSourceGuestPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RuntimeSourceGuestPrefix")
    }
}
