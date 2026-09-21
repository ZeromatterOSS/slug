//! Owned C++ HeaderInfo DAG. Public value identity is occurrence-based;
//! publication compares complete payloads and the shared graph's alias topology.
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use starlark_map::small_set::SmallSet;

use crate::AnalysisArtifact;
use crate::analysis_value::PublicationEqState;

#[cfg(test)]
mod tests;

#[derive(Debug, Default, Allocative)]
pub struct CcHeaderInfoInput {
    pub header_module: Option<AnalysisArtifact>,
    pub pic_header_module: Option<AnalysisArtifact>,
    pub modular_public_headers: Box<[AnalysisArtifact]>,
    pub modular_private_headers: Box<[AnalysisArtifact]>,
    pub textual_headers: Box<[AnalysisArtifact]>,
    pub separate_module_headers: Box<[AnalysisArtifact]>,
    pub separate_module: Option<AnalysisArtifact>,
    pub separate_pic_module: Option<AnalysisArtifact>,
    pub deps: Box<[CcHeaderInfo]>,
}

#[derive(Debug, Clone, Dupe, Allocative)]
pub struct CcHeaderInfo {
    occurrence: CcHeaderInfoOccurrence,
    model: Arc<CcHeaderInfoInput>,
}

impl CcHeaderInfo {
    /// Mirrors Bazel HeaderInfo.create. Host policy is supplied by the observed
    /// caller, never inferred from target configuration or ambient process state.
    pub fn create(
        mut input: CcHeaderInfoInput,
        windows_host: bool,
        merged_deps: &[Self],
    ) -> Result<Self, String> {
        for (name, module) in [
            ("header_module", &input.header_module),
            ("pic_header_module", &input.pic_header_module),
            ("separate_module", &input.separate_module),
            ("separate_pic_module", &input.separate_pic_module),
        ] {
            if matches!(module, Some(AnalysisArtifact::Source(_))) {
                return Err(format!("{name} requires a derived File or None"));
            }
        }
        if (input.separate_module.is_some() && input.header_module.is_none())
            || (input.separate_pic_module.is_some() && input.pic_header_module.is_none())
        {
            return Err("Separate module cannot be used without main module".to_owned());
        }
        let mut textual: SmallSet<_> = input.textual_headers.into_iter().collect();
        let mut public = SmallSet::new();
        let mut private = SmallSet::new();
        for (headers, modular) in [
            (input.modular_public_headers, &mut public),
            (input.modular_private_headers, &mut private),
        ] {
            for header in headers {
                let path = header.path();
                let suffix = path.get(path.len().saturating_sub(4)..).unwrap_or("");
                let is_textual = if windows_host {
                    suffix.eq_ignore_ascii_case(".inc")
                } else {
                    suffix == ".inc"
                };
                if is_textual {
                    textual.insert(header);
                } else {
                    modular.insert(header);
                }
            }
        }
        for dep in merged_deps {
            for (headers, set) in [
                (&dep.model.modular_public_headers, &mut public),
                (&dep.model.modular_private_headers, &mut private),
                (&dep.model.textual_headers, &mut textual),
            ] {
                for header in headers {
                    set.insert(header.clone());
                }
            }
        }
        input.modular_public_headers = public.into_iter().collect();
        input.modular_private_headers = private.into_iter().collect();
        input.textual_headers = textual.into_iter().collect();
        Ok(Self {
            occurrence: CcHeaderInfoOccurrence::new(),
            model: Arc::new(input),
        })
    }

    pub fn with_deps(
        &self,
        deps: Box<[Self]>,
        merged_deps: &[Self],
        windows_host: bool,
    ) -> Result<Self, String> {
        let model = self.model();
        Self::create(
            CcHeaderInfoInput {
                header_module: model.header_module.clone(),
                pic_header_module: model.pic_header_module.clone(),
                modular_public_headers: model.modular_public_headers.clone(),
                modular_private_headers: model.modular_private_headers.clone(),
                textual_headers: model.textual_headers.clone(),
                separate_module_headers: model.separate_module_headers.clone(),
                separate_module: model.separate_module.clone(),
                separate_pic_module: model.separate_pic_module.clone(),
                deps,
            },
            windows_host,
            merged_deps,
        )
    }

    pub fn model(&self) -> &CcHeaderInfoInput {
        &self.model
    }

    pub fn occurrence(&self) -> &CcHeaderInfoOccurrence {
        &self.occurrence
    }

    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        let mut pending = vec![(self, other)];
        while let Some((left, right)) = pending.pop() {
            match state
                .enter_header_info_pair(left.occurrence.pointer(), right.occurrence.pointer())
            {
                Err(()) => return false,
                Ok(false) => continue,
                Ok(true) => {}
            }
            let left = left.model();
            let right = right.model();
            if left.header_module != right.header_module
                || left.pic_header_module != right.pic_header_module
                || left.separate_module != right.separate_module
                || left.separate_pic_module != right.separate_pic_module
                || left.modular_public_headers != right.modular_public_headers
                || left.modular_private_headers != right.modular_private_headers
                || left.textual_headers != right.textual_headers
                || left.separate_module_headers != right.separate_module_headers
                || left.deps.len() != right.deps.len()
            {
                return false;
            }
            pending.extend(left.deps.iter().zip(&right.deps));
        }
        true
    }
}

impl PartialEq for CcHeaderInfo {
    fn eq(&self, other: &Self) -> bool {
        self.occurrence == other.occurrence
    }
}
impl Eq for CcHeaderInfo {}
impl Hash for CcHeaderInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.occurrence.hash(state);
    }
}

#[derive(Debug, Clone, Dupe, Allocative)]
pub struct CcHeaderInfoOccurrence(Arc<()>);
impl CcHeaderInfoOccurrence {
    pub fn new() -> Self {
        Self(Arc::new(()))
    }
    pub(crate) fn pointer(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
}
impl Default for CcHeaderInfoOccurrence {
    fn default() -> Self {
        Self::new()
    }
}
impl PartialEq for CcHeaderInfoOccurrence {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for CcHeaderInfoOccurrence {}
impl Hash for CcHeaderInfoOccurrence {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pointer().hash(state);
    }
}
