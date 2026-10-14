//! Consumer-local expansion of actual retained callback inputs.
use crate::AnalysisValue;
use crate::AnalysisValueKind;
use crate::args_mapper::ArgsDirectoryContext;
use crate::args_mapper::ArgsExpansionError;
use crate::args_mapper::NativeArgsBudget;
use crate::args_mapper::RetainedArgsInputs;
use crate::args_mapper::RetainedArgsMapper;
use crate::callable_graph::CALLABLE_GRAPH_BYTE_LIMIT;
use crate::callable_graph::CALLABLE_GRAPH_NODE_LIMIT;

pub(super) fn expand_mapped(
    inputs: &RetainedArgsInputs,
    mapper: &RetainedArgsMapper,
    registration_location: Option<&str>,
    context: &dyn ArgsDirectoryContext,
    expand_directories: bool,
) -> Result<Vec<String>, ArgsExpansionError> {
    let expand = || {
        if context.is_cancelled() {
            return Err(ArgsExpansionError::Cancelled);
        }
        let values = inputs.values()?;
        let values = if expand_directories {
            let mut expanded = Vec::new();
            let mut budget = NativeArgsBudget::default();
            for value in values {
                if context.is_cancelled() {
                    return Err(ArgsExpansionError::Cancelled);
                }
                if let AnalysisValueKind::Artifact(artifact) = value.kind() {
                    for child in context.list(artifact)? {
                        if context.is_cancelled() {
                            return Err(ArgsExpansionError::Cancelled);
                        }
                        if expanded.len() >= CALLABLE_GRAPH_NODE_LIMIT {
                            return Err(ArgsExpansionError::Limit("directory input count".into()));
                        }
                        let child = AnalysisValue::artifact(child);
                        budget.audit(&child)?;
                        expanded.push(child);
                    }
                } else {
                    if expanded.len() >= CALLABLE_GRAPH_NODE_LIMIT {
                        return Err(ArgsExpansionError::Limit("directory input count".into()));
                    }
                    budget.audit(&value)?;
                    expanded.push(value);
                }
            }
            expanded
        } else {
            values
        };
        let mapped = mapper.map_batch(&values, context)?;
        if mapped.len() > CALLABLE_GRAPH_NODE_LIMIT {
            return Err(ArgsExpansionError::Limit("callback output count".into()));
        }
        let mut bytes = 0usize;
        for value in &mapped {
            if context.is_cancelled() {
                return Err(ArgsExpansionError::Cancelled);
            }
            bytes = bytes
                .checked_add(value.len())
                .ok_or_else(|| ArgsExpansionError::Limit("callback output bytes".into()))?;
            if bytes > CALLABLE_GRAPH_BYTE_LIMIT {
                return Err(ArgsExpansionError::Limit("callback output bytes".into()));
            }
        }
        Ok(mapped.into_iter().map(|value| value.to_string()).collect())
    };
    expand().map_err(|error| match (registration_location, error) {
        (_, ArgsExpansionError::Cancelled) => ArgsExpansionError::Cancelled,
        (Some(location), error) => {
            ArgsExpansionError::Callback(format!("{location}: {error}").into())
        }
        (None, error) => error,
    })
}

/// Slug-native consumer output bound. Charge forecast sizes before allocating
/// formatted words, joins, prefixes or serialized virtual-file content.
#[derive(Default)]
pub(super) struct ExpansionBudget {
    bytes: usize,
    words: usize,
}
impl ExpansionBudget {
    pub(super) fn charge(&mut self, words: usize, bytes: usize) -> Result<(), ArgsExpansionError> {
        let bytes = checked_add(self.bytes, bytes)?;
        let words = checked_add(self.words, words)?;
        check_size(words, bytes)?;
        self.bytes = bytes;
        self.words = words;
        Ok(())
    }
    pub(super) fn bytes(&self) -> usize {
        self.bytes
    }
}
pub(super) fn checked_add(a: usize, b: usize) -> Result<usize, ArgsExpansionError> {
    a.checked_add(b)
        .ok_or_else(|| ArgsExpansionError::Limit("native expanded output size overflow".into()))
}
pub(super) fn checked_mul(a: usize, b: usize) -> Result<usize, ArgsExpansionError> {
    a.checked_mul(b)
        .ok_or_else(|| ArgsExpansionError::Limit("native expanded output size overflow".into()))
}
pub(super) fn check_size(words: usize, bytes: usize) -> Result<(), ArgsExpansionError> {
    if bytes > CALLABLE_GRAPH_BYTE_LIMIT {
        return Err(ArgsExpansionError::Limit(
            "native expanded output bytes".into(),
        ));
    }
    if words > CALLABLE_GRAPH_NODE_LIMIT {
        return Err(ArgsExpansionError::Limit(
            "native expanded output words".into(),
        ));
    }
    Ok(())
}
#[derive(Clone, Copy)]
pub(super) struct FormatSize {
    literal: usize,
    substitutions: usize,
}
impl FormatSize {
    pub(super) fn new(format: &str) -> Result<Self, ArgsExpansionError> {
        check_size(0, format.len())?;
        let mut literal = 0usize;
        let mut substitutions = 0usize;
        let mut chars = format.chars();
        while let Some(ch) = chars.next() {
            if ch != '%' {
                literal = checked_add(literal, ch.len_utf8())?;
            } else {
                match chars.next() {
                    Some('%') => literal = checked_add(literal, 1)?,
                    Some('s') => substitutions = checked_add(substitutions, 1)?,
                    _ => {
                        return Err(ArgsExpansionError::Unsupported(
                            "invalid retained Args format".into(),
                        ));
                    }
                }
            }
        }
        Ok(Self {
            literal,
            substitutions,
        })
    }
    pub(super) fn size(self, value: usize) -> Result<usize, ArgsExpansionError> {
        checked_add(self.literal, checked_mul(self.substitutions, value)?)
    }
}
pub(super) fn strings_size<'a>(
    values: impl IntoIterator<Item = &'a str>,
) -> Result<(usize, usize), ArgsExpansionError> {
    let mut count = 0usize;
    let mut bytes = 0usize;
    for value in values {
        count = checked_add(count, 1)?;
        bytes = checked_add(bytes, value.len())?;
        check_size(count, bytes)?;
    }
    Ok((count, bytes))
}
pub(super) fn shell_needs_quotes(value: &str) -> bool {
    let safe =
        |character: char| character.is_ascii_alphanumeric() || "@%-_+:,./".contains(character);
    value.is_empty()
        || !value
            .chars()
            .enumerate()
            .all(|(index, ch)| safe(ch) || (ch == '~' && index != 0))
}
pub(super) fn gcc_needs_escape(ch: char) -> bool {
    matches!(
        ch,
        '\'' | '"' | '\\' | ' ' | '\t' | '\r' | '\n' | '\u{c}' | '\u{b}'
    )
}
pub(super) fn param_lines_size(
    lines: &[String],
    format: super::spec::RetainedParamFileFormat,
) -> Result<(usize, usize), ArgsExpansionError> {
    let mut budget = ExpansionBudget::default();
    for line in lines {
        let bytes = match format {
            super::spec::RetainedParamFileFormat::Shell if shell_needs_quotes(line) => checked_add(
                checked_add(
                    line.len(),
                    checked_mul(line.bytes().filter(|b| *b == b'\'').count(), 3)?,
                )?,
                2,
            )?,
            super::spec::RetainedParamFileFormat::Gcc if line.is_empty() => 2,
            super::spec::RetainedParamFileFormat::Gcc => checked_add(
                line.len(),
                line.chars().filter(|ch| gcc_needs_escape(*ch)).count(),
            )?,
            _ => line.len(),
        };
        budget.charge(1, checked_add(bytes, 1)?)?;
    }
    Ok((lines.len(), budget.bytes()))
}

/// Size of one call's words before and after the optional FlagPerLine grouping.
#[derive(Default)]
pub(super) struct GroupSize {
    count: usize,
    bytes: usize,
    first: usize,
}
impl GroupSize {
    pub(super) fn add(&mut self, bytes: usize) -> Result<(), ArgsExpansionError> {
        if self.count == 0 {
            self.first = bytes;
        }
        self.count = checked_add(self.count, 1)?;
        self.bytes = checked_add(self.bytes, bytes)?;
        // Intermediate words remain byte-bounded; the consumer word bound uses
        // the actual grouped output, which can have a single word.
        check_size(0, self.bytes)
    }
    pub(super) fn charge(
        &self,
        budget: &mut ExpansionBudget,
        flag_per_line: bool,
    ) -> Result<(), ArgsExpansionError> {
        if flag_per_line && self.count >= 2 {
            let bytes = checked_add(self.bytes, self.count - 2)?;
            budget.charge(1, checked_add(bytes, usize::from(self.first != 0))?)
        } else {
            budget.charge(self.count, self.bytes)
        }
    }
}
