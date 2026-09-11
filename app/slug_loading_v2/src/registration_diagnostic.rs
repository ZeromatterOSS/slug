//! A bounded borrowed projection; the full typed error remains the equality domain.
use std::fmt;
use std::fmt::Write;

use slug_identity_v2::CanonicalLabel;

use crate::ModuleRegistrationExpansionError;
use crate::ModuleRegistrationExpansionErrorKind as Registration;
use crate::bzl_module::ExternalBzlModuleError;
use crate::bzl_module::HostBzlModuleError;
use crate::canonical_repository_load_route::HostCanonicalRepositoryLoadRouteError;
use crate::canonical_repository_load_route::HostCanonicalRepositoryLoadRouteErrorKind as Load;
use crate::canonical_repository_route::HostCanonicalRepositoryRouteError;
use crate::canonical_repository_route::HostCanonicalRepositoryRouteErrorKind as Route;
use crate::generated_repository_definition::HostGeneratedRepositoryDefinitionError;
use crate::generated_repository_definition::HostGeneratedRepositoryDefinitionErrorKind as Generated;
use crate::module_extension::HostSelectedExtensionOwnerPureError as Pure;
use crate::module_extension_innate_repository::HostPureInnateRepositoryOwnerError as Innate;
use crate::module_extension_repository_instantiation::HostInstantiatedModuleExtensionRepositoryError as Instantiate;
use crate::module_extension_repository_validation::HostModuleExtensionValidationError as ValidationReason;
use crate::module_extension_repository_validation::HostSelectedExtensionOwnerCertificateError;
use crate::module_extension_repository_validation::HostValidateModuleExtensionRequestError as Validate;
use crate::module_extension_repository_validation::PrivateOwnerCertificateError as Certificate;

const LIMIT: usize = 3072;
const OUTPUT_STOP: &str = "[diagnostic incomplete: output limit]";
const DEPTH_STOP: &str = "[diagnostic incomplete: depth limit]";

#[cfg(test)]
#[path = "registration_diagnostic_tests.rs"]
mod tests;

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
    Load(&'a HostCanonicalRepositoryLoadRouteError),
    Route(&'a HostCanonicalRepositoryRouteError),
    Generated(&'a HostGeneratedRepositoryDefinitionError),
    Certificate(&'a HostSelectedExtensionOwnerCertificateError),
    Pure(&'a Pure),
    Innate(&'a Innate),
    RootBzl(&'a HostBzlModuleError),
    ExternalBzl(&'a ExternalBzlModuleError),
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
        Registration::CanonicalPackage(_) => return out.incomplete("CanonicalPackage"),
    };
    walk(out, node)
}

#[rustfmt::skip]
fn walk(out: &mut Buffer, mut node: Node<'_>) -> fmt::Result {
    for _ in 0..32 {
        node = match node {
            Node::Load(error) => {
                write!(out, "CanonicalRoute {}: ", error.canonical_repo.as_str())?;
                match &error.kind {
                    Load::Route(error) => Node::Route(error),
                    Load::RouteCompute(message) => return write!(out, "RouteCompute: {message}"),
                    Load::EffectCompute(message) => return write!(out, "EffectCompute: {message}"),
                    Load::Effect(_) => return out.incomplete("Effect"),
                    Load::Projection(_) => return out.incomplete("Projection"),
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
                out.write_str("Pure: ")?;
                match error {
                    Pure::Inputs(error) => { out.write_str("Inputs: ")?; return error.write_registration_diagnostic(out); }
                    Pure::Compute(message) => return write!(out, "Compute: {message}"),
                    Pure::AfterInputs { message, .. } => return write!(out, "AfterInputs: {message}"),
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
                    E::Route { message, .. } => return write!(out, "Route: {message}"),
                    E::Parse { message, .. } => return write!(out, "Parse: {message}"),
                    E::Evaluation { message, .. } => return write!(out, "Evaluation: {message}"),
                    E::Freeze { message, .. } => return write!(out, "Freeze: {message}"),
                    E::Absent { label } => { out.write_str("Absent ")?; return out.label(label); }
                    E::Encoding { label } => { out.write_str("Encoding ")?; return out.label(label); }
                    E::Cycle(_) => return out.write_str("Cycle"),
                    E::Source { .. } => return out.incomplete("Source"),
                    E::SourceObservation { label, error } => {
                        out.write_str("SourceObservation ")?;
                        out.label(label)?;
                        out.write_str(": ")?;
                        return error.write_registration_diagnostic(out);
                    }
                    E::LoadLabel { .. } => return out.incomplete("LoadLabel"),
                }
            }
        };
    }
    out.write_str(DEPTH_STOP)
}
