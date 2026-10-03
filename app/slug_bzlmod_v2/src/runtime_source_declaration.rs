//! Explicit bounded raw runtime-directory declarations, not source or execution authority.
mod policy;
#[cfg(test)]
mod tests;
mod transaction;
mod types;

pub use policy::RuntimeSourceDeclaration;
pub use policy::RuntimeSourceDeclarationPolicy;
#[doc(hidden)]
pub use policy::RuntimeSourceGuestMappingRef;
pub use policy::RuntimeSourceSelectorRef;
pub use transaction::RuntimeSourceDeclarationPolicyKey;
pub use transaction::RuntimeSourceDeclarationTransaction;
pub use types::RuntimeSourceComponent;
pub use types::RuntimeSourceDeclarationError;
pub use types::RuntimeSourceDeclarationInput;
pub use types::RuntimeSourceFactTier;
#[doc(hidden)]
pub use types::RuntimeSourceGuestMappingInput;
pub use types::RuntimeSourceGuestPrefix;
pub use types::RuntimeSourceLimit;
pub use types::RuntimeSourceLimits;
pub use types::RuntimeSourcePlatform;
pub use types::RuntimeSourceSelectorInput;
