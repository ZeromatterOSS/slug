//! Physical immutable-output declarations on the existing materialization input.
//! These records never participate in active canonical request lookup.
use super::*;

impl RepositoryMaterializationResultEpoch {
    /// Borrow complete active declarations after a finite caller-owned bound.
    #[doc(hidden)]
    pub fn borrowed_entries(
        &self,
        workspace: &NormalizedAbsolutePath,
        maximum: usize,
    ) -> Result<
        impl ExactSizeIterator<Item = &RepositoryMaterializationEpochEntry>,
        RepositoryMaterializationInstanceLookupError,
    > {
        if workspace != &self.workspace {
            return Err(RepositoryMaterializationInstanceLookupError::WrongWorkspace);
        }
        if self.entries.len() > maximum {
            return Err(RepositoryMaterializationInstanceLookupError::TooManyEntries);
        }
        Ok(self.entries.values())
    }

    /// Complete retained incarnation guards, not successful request results.
    #[doc(hidden)]
    pub fn borrowed_retained_generated(
        &self,
        workspace: &NormalizedAbsolutePath,
        maximum: usize,
    ) -> Result<
        &[Arc<RepositoryMaterializationEpochEntry>],
        RepositoryMaterializationInstanceLookupError,
    > {
        if workspace != &self.workspace {
            return Err(RepositoryMaterializationInstanceLookupError::WrongWorkspace);
        }
        let entries = self.retained_generated.as_deref().unwrap_or(&[]);
        if entries.len() > maximum {
            return Err(RepositoryMaterializationInstanceLookupError::TooManyEntries);
        }
        Ok(entries)
    }

    /// Opt-in native input construction. The host must prepay complete foreign
    /// comparison and old/new vectors/entry/Arc/payload peaks before calling.
    /// Existing `new` callers keep an empty inventory. Retired entries confer
    /// physical ownership only; `get(request)` remains the active map lookup.
    #[doc(hidden)]
    pub fn retaining_generated_outputs(
        mut self,
        previous: Option<&Self>,
        maximum: usize,
    ) -> Result<Self, RepositoryMaterializationResultEpochError> {
        use RepositoryMaterializationResultEpochError as E;
        if self.entries.len() > maximum
            || previous.is_some_and(|p| p.workspace != self.workspace || p.entries.len() > maximum)
        {
            return Err(E::RetainedOutputLimit);
        }
        let mut retained: Vec<Arc<RepositoryMaterializationEpochEntry>> = Vec::new();
        let mine = self.retained_generated.as_deref().unwrap_or(&[]);
        if mine.len() > maximum {
            return Err(E::RetainedOutputLimit);
        }
        for entry in mine {
            insert(
                &mut retained,
                entry.as_ref(),
                Some(entry),
                &self.workspace,
                maximum,
            )?;
        }
        if let Some(previous) = previous {
            let old = previous.retained_generated.as_deref().unwrap_or(&[]);
            if old.len() > maximum {
                return Err(E::RetainedOutputLimit);
            }
            for entry in old {
                insert(
                    &mut retained,
                    entry.as_ref(),
                    Some(entry),
                    &self.workspace,
                    maximum,
                )?;
            }
            // Migration from a successful old-style active input is lossless.
            for entry in previous.entries.values() {
                if generated(entry) {
                    insert(&mut retained, entry, None, &self.workspace, maximum)?;
                }
            }
        }
        for entry in self.entries.values() {
            if generated(entry) {
                insert(&mut retained, entry, None, &self.workspace, maximum)?;
            }
        }
        retained.sort_by(|a, b| {
            let (ar, ai) = root(a).expect("validated output");
            let (br, bi) = root(b).expect("validated output");
            ai.cmp(&bi).then_with(|| ar.cmp(br))
        });
        self.retained_generated = (!retained.is_empty()).then(|| retained.into());
        Ok(self)
    }
}
fn generated(entry: &RepositoryMaterializationEpochEntry) -> bool {
    matches!(
        (&entry.request.kind, &entry.result),
        (
            RepositoryMaterializationKind::GeneratedFileEffects(_),
            RepositoryMaterializationResult::Success(
                RepositoryMaterializationSuccess::Immutable { .. }
            )
        )
    )
}
fn root(entry: &RepositoryMaterializationEpochEntry) -> Option<(&Path, PathObservationInstanceId)> {
    match &entry.result {
        RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable {
            generation_root,
            observation_instance,
            ..
        }) => Some((generation_root, *observation_instance)),
        _ => None,
    }
}
fn insert(
    out: &mut Vec<Arc<RepositoryMaterializationEpochEntry>>,
    entry: &RepositoryMaterializationEpochEntry,
    guard: Option<&Arc<RepositoryMaterializationEpochEntry>>,
    workspace: &NormalizedAbsolutePath,
    maximum: usize,
) -> Result<(), RepositoryMaterializationResultEpochError> {
    use RepositoryMaterializationResultEpochError as E;
    if !generated(entry) || &entry.request.id.workspace != workspace {
        return Err(E::InvalidRetainedOutput);
    }
    let (path, instance) = root(entry).ok_or(E::InvalidRetainedOutput)?;
    if instance.value() == 0
        || !path.is_absolute()
        || path == workspace.as_path()
        || workspace.as_path().starts_with(path)
        || path.components().any(|c| {
            !matches!(
                c,
                Component::Prefix(_) | Component::RootDir | Component::Normal(_)
            )
        })
    {
        return Err(E::InvalidRetainedOutput);
    }
    let RepositoryMaterializationKind::GeneratedFileEffects(plan) = &entry.request.kind else {
        unreachable!()
    };
    if !plan
        .ordered_effects()
        .all(|e| matches!(e, crate::GeneratedRepositoryOrderedEffectRef::File(_)))
    {
        return Err(E::InvalidRetainedOutput);
    }
    for prior in out.iter() {
        let (old, old_instance) = root(prior).ok_or(E::InvalidRetainedOutput)?;
        if old_instance == instance || old == path {
            if prior.as_ref() == entry {
                return Ok(());
            }
            return Err(E::ConflictingRetainedOutput);
        }
        if old.starts_with(path) || path.starts_with(old) {
            return Err(E::ConflictingRetainedOutput);
        }
    }
    if out.len() == maximum {
        return Err(E::RetainedOutputLimit);
    }
    out.push(match guard {
        Some(guard) => guard.clone(),
        None => Arc::new(entry.clone()),
    });
    Ok(())
}
