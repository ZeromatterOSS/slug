/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;

use kuro_error::ErrorTag;
use kuro_error::source_location::SourceLocation;
use kuro_event_observer::display::TargetDisplayOptions;
use kuro_event_observer::display::display_action_error;

use crate::actions::execute::error::ExecuteError;

#[derive(Debug)]
pub struct ActionError {
    execute_error: ExecuteError,
    name: kuro_data::ActionName,
    key: kuro_data::ActionKey,
    last_command: Option<kuro_data::CommandExecution>,
    error_diagnostics: Option<kuro_data::ActionErrorDiagnostics>,
}

impl From<ActionError> for kuro_error::Error {
    fn from(this: ActionError) -> kuro_error::Error {
        let is_command_failure = this.last_command.as_ref().is_some_and(|c| {
            matches!(
                c.status,
                Some(kuro_data::command_execution::Status::Failure { .. })
            )
        });

        let mut tags = vec![];
        let mut string_tags = vec![];
        let mut source_location = SourceLocation::new(std::file!()).with_type_name("ActionError");
        match &this.execute_error {
            ExecuteError::CommandExecutionError { error, .. } => {
                if let Some(err) = error {
                    tags.extend(err.tags());
                    string_tags.extend(err.string_tags());
                    source_location = err.source_location().clone();
                }

                if is_command_failure {
                    if let Some(diagnostic) = &this.error_diagnostics {
                        if let Some(kuro_data::action_error_diagnostics::Data::SubErrors(
                            sub_errors,
                        )) = diagnostic.data.as_ref()
                        {
                            // Only adding the first error category as multiple would likely cause too many variants and
                            // cause the data to be less useful. We can revisit this once we have more categories if needed.
                            if !sub_errors.sub_errors.is_empty() {
                                string_tags.push(sub_errors.sub_errors[0].category.clone());
                            }
                        }
                    }

                    tags.push(ErrorTag::ActionCommandFailure)
                }
            }
            // Returning extra outputs is a bug in the executor
            ExecuteError::MismatchedOutputs { .. } => tags.push(ErrorTag::ActionMismatchedOutputs),
            // However outputs may be legitimately missing if the action didn't produce them
            ExecuteError::MissingOutputs { .. } => tags.push(ErrorTag::ActionMissingOutputs),
            // Or if the action produced the wrong type
            ExecuteError::WrongOutputType { .. } => tags.push(ErrorTag::ActionWrongOutputType),
            ExecuteError::Error { .. } => (),
        };

        let msg = display_action_error(&this.as_proto_event(), TargetDisplayOptions::for_log())
            .expect("Action key is always present in `ActionError`")
            .simple_format_for_build_report();

        let base_error = match this.execute_error {
            ExecuteError::Error { error } => error.tag([ErrorTag::AnyActionExecution]).context(msg),
            // FIXME(JakobDegen): What about `CommandExecutionError`?
            _ => kuro_error::Error::new(
                msg,
                ErrorTag::AnyActionExecution,
                source_location,
                Some(this.as_proto_event()),
            ),
        };

        let mut e = base_error.tag(tags);
        for t in string_tags {
            e = e.string_tag(&t);
        }
        e
    }
}

impl ActionError {
    pub(crate) fn new(
        execute_error: ExecuteError,
        name: kuro_data::ActionName,
        key: kuro_data::ActionKey,
        last_command: Option<kuro_data::CommandExecution>,
        error_diagnostics: Option<kuro_data::ActionErrorDiagnostics>,
    ) -> Self {
        Self {
            execute_error,
            name,
            key,
            last_command,
            error_diagnostics,
        }
    }

    pub(crate) fn as_proto_field(&self) -> kuro_data::action_execution_end::Error {
        match &self.execute_error {
            ExecuteError::MissingOutputs { declared } => kuro_data::CommandOutputsMissing {
                message: format!("Action failed to produce outputs: {}", error_items(declared)),
            }
            .into(),
            ExecuteError::MismatchedOutputs { declared, real } => kuro_data::CommandOutputsMissing {
                message: format!(
                    "Action didn't produce the right set of outputs.\nExpected {}`\nreal {}",
                    error_items(declared),
                    error_items(real)
                ),
            }
            .into(),
            ExecuteError::WrongOutputType {path, declared, real} => kuro_data::CommandOutputsMissing {
                message: format!(
                    "Action didn't produce output of the right type.\nExpected {path} to be {declared:?}\nreal {real:?}",
                ),
            }
            .into(),
            ExecuteError::Error { error } => format!("{error:#}").into(),
            ExecuteError::CommandExecutionError { .. } => kuro_data::CommandExecutionError {}.into(),
        }
    }

    pub(crate) fn as_proto_event(&self) -> kuro_data::ActionError {
        let field = match self.as_proto_field() {
            kuro_data::action_execution_end::Error::Unknown(e) => e.into(),
            kuro_data::action_execution_end::Error::MissingOutputs(e) => e.into(),
            kuro_data::action_execution_end::Error::CommandExecutionError(e) => e.into(),
        };
        kuro_data::ActionError {
            error: Some(field),
            name: Some(self.name.clone()),
            key: Some(self.key.clone()),
            last_command: self.last_command.clone(),
            error_diagnostics: self.error_diagnostics.clone(),
        }
    }
}

fn error_items<T: fmt::Display>(xs: &[T]) -> String {
    use fmt::Write;

    if xs.is_empty() {
        return "none".to_owned();
    }
    let mut res = String::new();
    for (i, x) in xs.iter().enumerate() {
        if i != 0 {
            res.push_str(", ");
        }
        write!(res, "`{x}`").unwrap();
    }
    res
}

#[cfg(test)]
mod tests {
    use kuro_error::ErrorTag;
    use kuro_error::kuro_error;

    use super::*;

    #[test]
    fn test_error_conversion() {
        let error = kuro_error!(ErrorTag::Http, "error");

        let execute_error = ExecuteError::Error {
            error: error.into(),
        };

        let action_error = ActionError::new(
            execute_error,
            kuro_data::ActionName {
                category: "category".to_owned(),
                identifier: "identifier".to_owned(),
                progress_message: String::new(),
            },
            kuro_data::ActionKey {
                id: vec![],
                key: "key".to_owned(),
                owner: Some(kuro_data::action_key::Owner::TargetLabel(
                    kuro_data::ConfiguredTargetLabel {
                        label: Some(kuro_data::TargetLabel {
                            package: "package".to_owned(),
                            name: "name".to_owned(),
                        }),
                        configuration: Some(kuro_data::Configuration {
                            full_name: "conf".into(),
                        }),
                        execution_configuration: None,
                    },
                )),
            },
            None,
            None,
        );

        let kuro_error = kuro_error::Error::from(action_error);

        assert_eq!(
            kuro_error.tags(),
            vec![ErrorTag::AnyActionExecution, ErrorTag::Http]
        );
    }
}
