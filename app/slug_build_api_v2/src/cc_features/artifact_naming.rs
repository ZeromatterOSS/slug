//! Pure POSIX PathFragment naming over the existing retained feature model.
use super::metadata::category_spec;
use super::metadata::normalize_posix_path;
use super::*;

impl CcToolchainFeatures {
    /// Bazel CcToolchainFeatures:1338–1344. The caller selects POSIX semantics
    /// from an observed analysis Host; toolchain exec_os does not select them.
    pub fn artifact_name_for_category_posix(
        &self,
        category: &str,
        output_name: &str,
    ) -> Result<CompactString, CcFeatureError> {
        let normalized_category = category.to_ascii_lowercase();
        if normalized_category.to_ascii_uppercase() != category {
            return Err(invalid(format!("unknown artifact category '{category}'")));
        }
        let (default_prefix, extensions) = category_spec(&normalized_category)?;
        let (prefix, extension) = self
            .model()
            .artifact_name_patterns
            .iter()
            .find(|pattern| pattern.category == normalized_category)
            .map(|pattern| (pattern.prefix.as_str(), pattern.extension.as_str()))
            .unwrap_or((default_prefix, extensions[0]));
        let output = normalize_posix_path(output_name)?;
        // Empty and root fragments have no parent directory, independently of
        // whether the selected prefix/extension could form a nonempty child.
        if output.is_empty() || output == "/" {
            return Err(invalid("artifact output name has no parent directory"));
        }
        let (parent, basename) = match output.rsplit_once('/') {
            Some(("", basename)) => ("/", basename),
            Some(pair) => pair,
            None => ("", output.as_str()),
        };
        let child = format!("{prefix}{basename}{extension}");
        // PathFragment.getChild validates AFTER composition. POSIX backslash
        // and colon are ordinary filename bytes, not separators or drives.
        if child.is_empty() || child == "." || child == ".." || child.contains('/') {
            return Err(invalid(
                "artifact name pattern produced an invalid child name",
            ));
        }
        if child.contains('\0') {
            return Err(invalid("NUL in C++ artifact names is unsupported"));
        }
        Ok(match parent {
            "" => child.into(),
            "/" => format!("/{child}").into(),
            _ => format!("{parent}/{child}").into(),
        })
    }
}

#[cfg(test)]
mod tests;
