//! Immediate closed root metadata only: never a native payload walk.
use super::*;
use crate::AnalysisValueKind as K;
use crate::ProviderValue as P;
use crate::cc_features::CcToolchainFeatureModel;

pub(super) fn atom(
    value: &AnalysisValue,
) -> Option<(
    PublicationNativeRootFamily,
    Option<&Arc<CcToolchainFeatureModel>>,
)> {
    // Completed pure certificates reject origins, including the feature cache.
    // Check the barrier before kind(), whose Published arm otherwise delegates.
    if value.original_origin().is_some() {
        return None;
    }
    use PublicationNativeRootFamily as F;
    Some(match value.kind() {
        K::None | K::Boolean(_) | K::Number(_) | K::String(_) | K::DottedVersion(_) => {
            (F::Scalar, None)
        }
        K::Label(_) | K::Artifact(_) | K::ArtifactRoot(_) => (F::LabelArtifact, None),
        K::CppFragment(_) | K::CcHeaderInfo(_) => (F::CppMetadata, None),
        K::CcToolchainFeatures(v) => (F::CcFeatureModel, Some(v.model())),
        K::CcFeatureConfiguration(v) => (F::CcFeatureConfiguration, Some(&v.model().model)),
        K::Runfiles(_) | K::Depset(_) => (F::RunfilesDepset, None),
        K::TemplateVariableInfo(_) | K::RunEnvironmentInfo(_) => (F::VariableEnvironment, None),
        K::PackageSpecificationInfo(_) => (F::PackageSpecification, None),
        K::Function(_)
        | K::ProviderConstructor(_)
        | K::RawProviderConstructor(_)
        | K::CcToolchainVariables(_)
        | K::ConfiguredTarget(_)
        | K::List(_)
        | K::Tuple(_)
        | K::Dictionary(_)
        | K::Struct(_)
        | K::Provider(_) => return None,
    })
}
pub(super) fn provider(value: &P) -> Option<PublicationNativeRootFamily> {
    use PublicationNativeRootFamily as F;
    Some(match value {
        P::TemplateVariableInfo(_) | P::RunEnvironmentInfo(_) => F::VariableEnvironment,
        P::PackageSpecificationInfo(_) => F::PackageSpecification,
        P::DefaultInfo(_) | P::OutputGroupInfo(_) | P::FilesToRunProvider(_) => F::FilesProvider,
        P::PlatformInfo(_) | P::FeatureFlagInfo(_) => F::OtherProvider,
        P::Occurrence(_) => return None,
    })
}
pub(super) fn row(row: &PublishedNode) -> PublicationRowKind {
    use PublicationRowKind as R;
    match row {
        PublishedNode::Function(_) => R::Function,
        PublishedNode::Intrinsic(_) => R::Intrinsic,
        PublishedNode::Code(_) => R::Code,
        PublishedNode::ProviderConstructor(_) => R::ProviderConstructor,
        PublishedNode::RawProviderConstructor { .. } => R::RawProviderConstructor,
        PublishedNode::Cell { .. } => R::Cell,
        PublishedNode::Set(_) => R::Set,
        PublishedNode::List(_) => R::List,
        PublishedNode::Tuple(_) => R::Tuple,
        PublishedNode::Dictionary(_) => R::Dictionary,
        PublishedNode::Struct { .. } => R::Struct,
        PublishedNode::Provider(_) => R::Provider,
        PublishedNode::ConfiguredTarget { .. } => R::ConfiguredTarget,
        PublishedNode::Depset(_) => R::Depset,
        PublishedNode::CcVariables(_) => R::CcVariables,
        PublishedNode::Native(_) => R::Native,
    }
}
