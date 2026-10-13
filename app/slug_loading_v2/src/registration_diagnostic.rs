//! A bounded borrowed projection; the full typed error remains the equality domain.
use std::fmt;
use std::fmt::Write;

use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlanError as Plan;
use slug_bzlmod_v2::HostRepositoryLabelPathError as LabelPath;
use slug_bzlmod_v2::RepositoryLabelPathAddress;
use slug_identity_v2::CanonicalLabel;

use crate::ModuleRegistrationExpansionError;
use crate::ModuleRegistrationExpansionErrorKind as Registration;
use crate::bzl_module::ExternalBzlModuleError;
use crate::bzl_module::ExternalBzlRouteError;
use crate::bzl_module::HostBzlModuleError;
use crate::bzl_module::RepositoryPackageLoadDiagnostic;
use crate::bzl_module::RootPackageLoadDiagnostic;
use crate::bzl_module::RootPackageLoadError;
use crate::canonical_repository_load_route::HostCanonicalRepositoryLoadRouteError;
use crate::canonical_repository_load_route::HostCanonicalRepositoryLoadRouteErrorKind as Load;
use crate::canonical_repository_mapping::HostCanonicalRepositoryApparentMappingError;
use crate::canonical_repository_mapping::HostCanonicalRepositoryApparentMappingErrorKind as Mapping;
use crate::canonical_repository_route::HostCanonicalRepositoryRouteError;
use crate::canonical_repository_route::HostCanonicalRepositoryRouteErrorKind as Route;
use crate::generated_repository_definition::HostGeneratedRepositoryDefinitionError;
use crate::generated_repository_definition::HostGeneratedRepositoryDefinitionErrorKind as Generated;
use crate::module_extension::HostSelectedExtensionOwnerPureError as Pure;
use crate::module_extension_innate_repository::HostPureInnateRepositoryOwnerError as Innate;
use crate::module_extension_repository_file_effect::HostSelectedRepositoryFileEffectError as Effect;
use crate::module_extension_repository_instantiation::HostInstantiatedModuleExtensionRepositoryError as Instantiate;
use crate::module_extension_repository_validation::HostModuleExtensionValidationError as ValidationReason;
use crate::module_extension_repository_validation::HostSelectedExtensionOwnerCertificateError;
use crate::module_extension_repository_validation::HostValidateModuleExtensionRequestError as Validate;
use crate::module_extension_repository_validation::PrivateOwnerCertificateError as Certificate;
use crate::root_repository_load_route::HostRootRepositoryLoadRouteError;
use crate::root_repository_load_route::RootLoadDiagnostic;

const LIMIT: usize = 3072;
const OUTPUT_STOP: &str = "[diagnostic incomplete: output limit]";
const DEPTH_STOP: &str = "[diagnostic incomplete: depth limit]";

#[cfg(test)]
#[path = "registration_diagnostic_tests.rs"]
pub(crate) mod tests;

struct Buffer {
    bytes: [u8; LIMIT],
    len: usize,
    stopped: bool,
}

impl Buffer {
    fn new() -> Self {
        Self {
            bytes: [0; LIMIT],
            len: 0,
            stopped: false,
        }
    }

    fn text(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len]).expect("ASCII diagnostic")
    }

    fn label(&mut self, label: &CanonicalLabel) -> fmt::Result {
        write!(
            self,
            "@@{}//{}:{}",
            label.package().repo().as_str(),
            label.package().package().as_str(),
            label.target().as_str()
        )
    }

    fn address(&mut self, address: &RepositoryLabelPathAddress) -> fmt::Result {
        write!(
            self,
            "@@{}//{}:{}",
            address.repo().as_str(),
            address.package().as_str(),
            address.target().as_str()
        )
    }

    fn incomplete(&mut self, owner: &str) -> fmt::Result {
        write!(self, "[diagnostic incomplete: {owner}]")
    }
}

impl Write for Buffer {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if self.stopped {
            return Err(fmt::Error);
        }
        // Reserve the complete marker; inspect only the consumed prefix plus one char.
        for ch in text.chars() {
            let escaped = ch.escape_default();
            let literal = ch.is_ascii() && !ch.is_ascii_control();
            let width = if literal { 1 } else { escaped.len() };
            if self.len + width > LIMIT - OUTPUT_STOP.len() {
                self.bytes[self.len..self.len + OUTPUT_STOP.len()]
                    .copy_from_slice(OUTPUT_STOP.as_bytes());
                self.len += OUTPUT_STOP.len();
                self.stopped = true;
                return Err(fmt::Error);
            }
            if literal {
                self.bytes[self.len] = ch as u8;
                self.len += 1;
            } else {
                for byte in escaped {
                    self.bytes[self.len] = byte as u8;
                    self.len += 1;
                }
            }
        }
        Ok(())
    }
}

struct Diagnostic<'a>(&'a ModuleRegistrationExpansionError);

struct RootPackageDiagnostic<'a>(&'a RootPackageLoadError);

impl RootPackageLoadError {
    /// Bounded borrowed Slug-native presentation, separate from typed equality.
    pub fn diagnostic(&self) -> impl fmt::Display + '_ {
        RootPackageDiagnostic(self)
    }
}

impl fmt::Display for RootPackageDiagnostic<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = Buffer::new();
        let result = render_root_package(&mut out, self.0);
        debug_assert!(result.is_ok() || out.stopped);
        f.write_str(out.text())
    }
}

fn render_root_package(out: &mut Buffer, error: &RootPackageLoadError) -> fmt::Result {
    write!(out, "{}: ", error.diagnostic_stage())?;
    let node = match error.diagnostic_view() {
        RootPackageLoadDiagnostic::Leaf(error) => return error.write(out),
        RootPackageLoadDiagnostic::ExternalRoute {
            load,
            apparent_repo,
            error,
        } => {
            write!(out, "{load} @{}: ", apparent_repo.as_str())?;
            Node::RootLoad(error)
        }
        RootPackageLoadDiagnostic::Bzl {
            origin,
            load,
            error,
        } => {
            write!(out, "{origin} load {load}: ")?;
            Node::RootBzl(error)
        }
        RootPackageLoadDiagnostic::ExternalBzl {
            origin,
            load,
            label,
            error,
        } => {
            write!(out, "{origin} load {load} ")?;
            out.label(label)?;
            out.write_str(": ")?;
            Node::ExternalBzl(error)
        }
    };
    walk_with_mode(out, node, true)
}

impl ModuleRegistrationExpansionError {
    /// Bounded Slug-native presentation; never use it as semantic identity.
    pub fn diagnostic(&self) -> impl fmt::Display + '_ {
        Diagnostic(self)
    }
}

impl fmt::Display for Diagnostic<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = Buffer::new();
        let result = render(&mut out, self.0);
        // Only our bounded sink can fail during traversal; caller failures are separate.
        debug_assert!(result.is_ok() || out.stopped);
        f.write_str(out.text())
    }
}

enum Node<'a> {
    RootLoad(&'a HostRootRepositoryLoadRouteError),
    ApparentMapping(&'a HostCanonicalRepositoryApparentMappingError),
    Load(&'a HostCanonicalRepositoryLoadRouteError),
    Effect(&'a Effect),
    Route(&'a HostCanonicalRepositoryRouteError),
    Generated(&'a HostGeneratedRepositoryDefinitionError),
    Certificate(&'a HostSelectedExtensionOwnerCertificateError),
    Pure(&'a Pure),
    Innate(&'a Innate),
    RootBzl(&'a HostBzlModuleError),
    ExternalBzl(&'a ExternalBzlModuleError),
}

#[rustfmt::skip]
fn label_path(out: &mut Buffer, error: &LabelPath) -> fmt::Result {
    match error {
        LabelPath::RootRepositoryMismatch(repo) => write!(out, "RootRepositoryMismatch: {}", repo.as_str()),
        LabelPath::ExternalRepositoryMismatch(route, requested) => write!(out, "ExternalRepositoryMismatch: {} / {}", route.as_str(), requested.as_str()),
        LabelPath::PackageAbsent => out.write_str("PackageAbsent"),
        LabelPath::PackageDeleted => out.write_str("PackageDeleted"),
        LabelPath::PackageIgnored => out.write_str("PackageIgnored"),
        LabelPath::InvalidPackageName(message) => write!(out, "InvalidPackageName: {message}"),
        LabelPath::RootPackageLookup(message) => write!(out, "RootPackageLookup: {message}"),
        LabelPath::ExternalPackageLookup(message) => write!(out, "ExternalPackageLookup: {message}"),
        LabelPath::BuiltinCatalog => out.write_str("BuiltinCatalog"),
        LabelPath::Materialization(message) => write!(out, "Materialization: {message}"),
        LabelPath::MaterializationCompute(message) => write!(out, "MaterializationCompute: {message}"),
        LabelPath::InvalidMaterializedPath => out.write_str("InvalidMaterializedPath"),
        LabelPath::NonUnicodePhysicalPath => out.write_str("NonUnicodePhysicalPath"),
    }
}

fn instantiation(out: &mut Buffer, error: &Instantiate) -> fmt::Result {
    match error {
        Instantiate::Join(message) => write!(out, "Join: {message}"),
        Instantiate::Namespace(message) => write!(out, "Namespace: {message}"),
        Instantiate::Attribute(message) => write!(out, "Attribute: {message}"),
    }
}

fn validation(out: &mut Buffer, error: &Validate) -> fmt::Result {
    match error {
        Validate::Join(message) => write!(out, "Join: {message}"),
        Validate::Validation { error, .. } => out.write_str(match error {
            ValidationReason::MissingImport => "MissingImport",
            ValidationReason::MissingOverride => "MissingOverride",
            ValidationReason::InjectCollision => "InjectCollision",
        }),
    }
}

// Compact exhaustive arms expose exactly which fields are visited. No recursive
// formatter, collection traversal, or success predecessor enters this projection.
#[rustfmt::skip]
fn render(out: &mut Buffer, error: &ModuleRegistrationExpansionError) -> fmt::Result {
    write!(out, "{} registration", error.family())?;
    if let Some(row) = error.row() { write!(out, " row {row}")?; }
    out.write_str(": ")?;
    let node = match error.kind() {
        Registration::CanonicalRoute(error) => Node::Load(error),
        Registration::Parse(message) => return write!(out, "Parse: {message}"),
        Registration::MissingTarget(label) => { out.write_str("MissingTarget ")?; return out.label(label); }
        Registration::RootMappingUnavailable => return out.write_str("RootMappingUnavailable"),
        Registration::RowOverflow => return out.write_str("RowOverflow"),
        Registration::Selected(_) => return out.incomplete("Selected"),
        Registration::Configuration(_) => return out.incomplete("Configuration"),
        Registration::RootMapping(_) => return out.incomplete("RootMapping"),
        Registration::RootSubtree(_) => return out.incomplete("RootSubtree"),
        Registration::CanonicalSubtree(_) => return out.incomplete("CanonicalSubtree"),
        Registration::RootPackage(_) => return out.incomplete("RootPackage"),
        Registration::CanonicalPackage(error) => {
            out.write_str("CanonicalPackage: ")?;
            match error.registration_diagnostic() {
                RepositoryPackageLoadDiagnostic::Leaf(error) => return error.write(out),
                RepositoryPackageLoadDiagnostic::AttemptLoading(error) => {
                    return write!(out, "Attempt.Loading: {error}");
                }
                RepositoryPackageLoadDiagnostic::Bzl { raw_load, canonical_label, error } => {
                    write!(out, "Bzl {raw_load} ")?;
                    out.label(canonical_label)?;
                    out.write_str(": ")?;
                    Node::ExternalBzl(error)
                }
            }
        }
    };
    walk(out, node)
}

fn walk(out: &mut Buffer, node: Node<'_>) -> fmt::Result {
    walk_with_mode(out, node, false)
}

#[rustfmt::skip]
fn walk_with_mode(out: &mut Buffer, mut node: Node<'_>, strict: bool) -> fmt::Result {
    for _ in 0..32 {
        node = match node {
            Node::RootLoad(error) => {
                out.write_str("RootLoad: ")?;
                match error.diagnostic_view() {
                    RootLoadDiagnostic::Root(error) => return error.write_route_diagnostic(out),
                    RootLoadDiagnostic::Mapping(error) => Node::ApparentMapping(error),
                    RootLoadDiagnostic::LoadRoute(error) => Node::Load(error),
                    RootLoadDiagnostic::RootCompute(message) => return write!(out, "RootCompute: {message}"),
                    RootLoadDiagnostic::MappingCompute(message) => return write!(out, "MappingCompute: {message}"),
                    RootLoadDiagnostic::LoadRouteCompute(message) => return write!(out, "LoadRouteCompute: {message}"),
                    RootLoadDiagnostic::Projection(message) => return write!(out, "Projection: {message}"),
                }
            }
            Node::ApparentMapping(error) => {
                write!(out, "ApparentMapping {} @{}: ", error.context_repo.as_str(), error.apparent_repo.as_str())?;
                match &error.kind {
                    Mapping::RootApparent => return out.write_str("RootApparent"),
                    Mapping::Missing { .. } => return out.write_str("Missing"),
                    Mapping::ContextMismatch { .. } => return out.write_str("ContextMismatch"),
                    Mapping::RootMapping(_) => return out.incomplete("RootMapping"),
                    Mapping::RootMappingCompute(message) => return write!(out, "RootMappingCompute: {message}"),
                    Mapping::RouteCompute(message) => return write!(out, "RouteCompute: {message}"),
                    Mapping::Route(error) => Node::Route(error),
                }
            }
            Node::Load(error) => {
                write!(out, "CanonicalRoute {}: ", error.canonical_repo.as_str())?;
                match &error.kind {
                    Load::Route(error) => Node::Route(error),
                    Load::RouteCompute(message) => return write!(out, "RouteCompute: {message}"),
                    Load::EffectCompute(message) => return write!(out, "EffectCompute: {message}"),
                    Load::Effect(error) => Node::Effect(error),
                    Load::Projection(_) => return out.incomplete("Projection"),
                }
            }
            Node::Effect(error) => {
                out.write_str("Effect: ")?;
                match error {
                    Effect::Certificate(error) => Node::Certificate(error),
                    Effect::Compute(message) => return write!(out, "Compute: {message}"),
                    Effect::MissingOrdinal { ordinal, .. } => return write!(out, "MissingOrdinal: {ordinal}"),
                    Effect::UnsupportedDefiningLabel { ordinal, label, .. } => { write!(out, "UnsupportedDefiningLabel {ordinal}: ")?; return out.label(label); }
                    Effect::HostBzl { ordinal, error, .. } => return write!(out, "HostBzl {ordinal}: {}", error.message()),
                    Effect::Projection { ordinal, message, .. } => return write!(out, "Projection {ordinal}: {message}"),
                    Effect::HostInput { ordinal, message, .. } => return write!(out, "HostInput {ordinal}: {message}"),
                    Effect::Path { ordinal, error, .. } => return match error { Plan::InvalidPath(path) => write!(out, "Path {ordinal}: InvalidPath: {path}"), Plan::RepeatedPath(path) => write!(out, "Path {ordinal}: RepeatedPath: {path}"), Plan::ConflictingPath(path) => write!(out, "Path {ordinal}: ConflictingPath: {path}"), Plan::SymlinkArchiveMix => write!(out, "SymlinkPlan {ordinal}: SymlinkArchiveMix"), Plan::TooManyFilesystemOperations => write!(out, "FilesystemPlan {ordinal}: TooManyFilesystemOperations"), Plan::SymlinkDeleteMix => write!(out, "FilesystemPlan {ordinal}: SymlinkDeleteMix"), Plan::EmptyArchiveUrls => write!(out, "ArchivePlan {ordinal}: EmptyArchiveUrls"), Plan::TooManyArchiveUrls => write!(out, "ArchivePlan {ordinal}: TooManyArchiveUrls"), Plan::InvalidArchiveUrl => write!(out, "ArchivePlan {ordinal}: InvalidArchiveUrl"), Plan::InvalidArchiveStripPrefix => write!(out, "ArchivePlan {ordinal}: InvalidArchiveStripPrefix"), Plan::MissingArchiveChecksum => write!(out, "ArchivePlan {ordinal}: MissingArchiveChecksum"), Plan::ConflictingArchiveChecksum => write!(out, "ArchivePlan {ordinal}: ConflictingArchiveChecksum"), Plan::InvalidArchiveChecksum => write!(out, "ArchivePlan {ordinal}: InvalidArchiveChecksum"), Plan::UnsupportedArchiveChecksum => write!(out, "ArchivePlan {ordinal}: UnsupportedArchiveChecksum") },
                    Effect::LabelPathRoute { address, error, .. } => { out.write_str("LabelPathRoute ")?; out.address(address)?; out.write_str(": ")?; Node::Load(error) }
                    Effect::LabelPath { address, error, .. } => { out.write_str("LabelPath ")?; out.address(address)?; out.write_str(": ")?; return label_path(out, error); }
                    Effect::Invocation { ordinal, message, .. } => return write!(out, "Invocation {ordinal}: {message}"),
                    Effect::Result { ordinal, type_name, .. } => return write!(out, "Result {ordinal}: {type_name}"),
                }
            }
            Node::Route(error) => {
                write!(out, "Route {}: ", error.canonical_repo.as_str())?;
                match &error.kind {
                    Route::Generated { error, .. } => Node::Generated(error),
                    Route::GeneratedCompute { message, .. } => return write!(out, "GeneratedCompute: {message}"),
                    Route::Missing { .. } => return out.write_str("Missing: selected and generated lookups missed"),
                    Route::BuiltinCompute(message) => return write!(out, "BuiltinCompute: {message}"),
                    Route::SelectedCompute(message) => return write!(out, "SelectedCompute: {message}"),
                    Route::Builtin(_) => return out.incomplete("Builtin"),
                    Route::Selected(_) => return out.incomplete("Selected"),
                }
            }
            Node::Generated(error) => {
                write!(out, "Generated {}: ", error.requested.as_str())?;
                match &error.kind {
                    Generated::Demand(error) => { out.write_str("Demand: ")?; return error.write_registration_diagnostic(out); }
                    Generated::DemandCompute(message) => return write!(out, "DemandCompute: {message}"),
                    Generated::Loading(error) => Node::Certificate(error),
                    Generated::LoadingCompute(message) => return write!(out, "LoadingCompute: {message}"),
                    Generated::Missing {} => return out.write_str("Missing"),
                    Generated::Duplicate { first, conflicting } => return write!(out, "Duplicate: {first} / {conflicting}"),
                }
            }
            Node::Certificate(error) => {
                out.write_str("Loading: ")?;
                match &error.0 {
                    Certificate::Pure(error) => Node::Pure(error),
                    Certificate::InnatePure(error) => Node::Innate(error),
                    Certificate::Compute(message) => return write!(out, "Compute: {message}"),
                    Certificate::Instantiation { error, .. } => { out.write_str("Instantiation: ")?; return instantiation(out, &error.error); }
                    Certificate::InnateInstantiation { error, .. } => { out.write_str("InnateInstantiation: ")?; return instantiation(out, &error.error); }
                    Certificate::Validation { error, .. } => { out.write_str("Validation: ")?; return validation(out, error); }
                    Certificate::InnateValidation { error, .. } => { out.write_str("InnateValidation: ")?; return validation(out, error); }
                }
            }
            Node::Pure(error) => {
                if let Pure::Factored { error, .. } = error {
                    node = Node::Pure(error);
                    continue;
                }
                out.write_str("Pure: ")?;
                match error {
                    Pure::Inputs(error) => { out.write_str("Inputs: ")?; return error.write_registration_diagnostic(out); }
                    Pure::Compute(message) => return write!(out, "Compute: {message}"),
                    Pure::AfterInputs { message, .. } => return write!(out, "AfterInputs: {message}"),
                    Pure::SourceOperational { error, .. } => return write!(out, "Source: {error}"),
                    Pure::Factored { .. } => unreachable!("factored selected error consumes a walk step"),
                }
            }
            Node::Innate(error) => {
                out.write_str("InnatePure: ")?;
                match error {
                    Innate::Inputs(error) => { out.write_str("Inputs: ")?; return error.write_registration_diagnostic(out); }
                    Innate::Compute(message) => return write!(out, "Compute: {message}"),
                    Innate::Label(message) => return write!(out, "Label: {message}"),
                    Innate::Export(message) => return write!(out, "Export: {message}"),
                    Innate::Call(message) => return write!(out, "Call: {message}"),
                    Innate::Drift => return out.write_str("Drift"),
                    Innate::LoadRoute(error) => Node::Load(error),
                    Innate::RootBzl(error) => Node::RootBzl(error),
                    Innate::ExternalBzl(error) => Node::ExternalBzl(error),
                }
            }
            Node::RootBzl(error) => {
                out.write_str("RootBzl: ")?;
                match error {
                    HostBzlModuleError::Child { load, error, .. } => { write!(out, "Child {load}: ")?; Node::RootBzl(error) }
                    HostBzlModuleError::Parse { message, .. } => return write!(out, "Parse: {message}"),
                    HostBzlModuleError::Freeze { message, .. } => return write!(out, "Freeze: {message}"),
                    HostBzlModuleError::Cycle(_) => return out.write_str("Cycle"),
                    HostBzlModuleError::Source(_) => return out.incomplete("Source"),
                    HostBzlModuleError::Input(_) => return out.incomplete("Input"),
                    HostBzlModuleError::LoadLabel { .. } => return out.incomplete("LoadLabel"),
                    HostBzlModuleError::Evaluation(_) => return out.incomplete("Evaluation"),
                }
            }
            Node::ExternalBzl(error) => {
                use ExternalBzlModuleError as E;
                out.write_str("ExternalBzl: ")?;
                match error {
                    E::Child { raw_load, error, .. } => { write!(out, "Child {raw_load}: ")?; Node::ExternalBzl(error) }
                    E::SourceCompute { message, .. } => return write!(out, "SourceCompute: {message}"),
                    E::Route { error, .. } => {
                        out.write_str("Route: ")?;
                        match error {
                            ExternalBzlRouteError::Compute(_) => return out.write_str("Compute"),
                            ExternalBzlRouteError::Observation(_) => return out.write_str("Observation"),
                            ExternalBzlRouteError::Load(error) => Node::Load(error),
                        }
                    }
                    E::Parse { message, .. } => return write!(out, "Parse: {message}"),
                    E::Evaluation { message, .. } => return write!(out, "Evaluation: {message}"),
                    E::Freeze { message, .. } => return write!(out, "Freeze: {message}"),
                    E::Absent { label } => { out.write_str("Absent ")?; return out.label(label); }
                    E::Encoding { label } => { out.write_str("Encoding ")?; return out.label(label); }
                    E::Cycle(_) => return out.write_str("Cycle"),
                    E::DependencyCycle(_) if strict => return out.write_str("DependencyCycle"),
                    E::DependencyCycle(cycle) => return write!(out, "DependencyCycle: {cycle}"),
                    E::Source { .. } => return out.incomplete("Source"),
                    E::SourceObservation { label, error } => {
                        out.write_str("SourceObservation ")?;
                        out.label(label)?;
                        out.write_str(": ")?;
                        return error.write_registration_diagnostic(out);
                    }
                    E::LoadLabel { .. } => return out.incomplete("LoadLabel"),
                    E::IncompatibleNeeds { message, .. } => {
                        return write!(out, "IncompatibleNeeds: {message}");
                    }
                }
            }
        };
    }
    out.write_str(DEPTH_STOP)
}
