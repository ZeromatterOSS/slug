/*
 * Copyright 2018 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! String interpolation-related code.
//! Based on <https://docs.python.org/3/library/stdtypes.html#printf-style-string-formatting>

use std::borrow::Cow;
use std::fmt;
use std::fmt::Write;

use dupe::Dupe;
use num_traits::Signed;
use thiserror::Error;

use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::values::BoundedText;
use crate::values::Heap;
use crate::values::StringValue;
use crate::values::UnpackValue;
use crate::values::Value;
use crate::values::ValueError;
use crate::values::ValueLike;
use crate::values::ValueTextMode;
use crate::values::float::StarlarkFloat;
use crate::values::float::float;
use crate::values::string::dot_format::format_one;
use crate::values::string::dot_format::format_one_bounded;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::types::int::int_or_big::StarlarkIntRef;
use crate::values::types::num::value::NumRef;
use crate::values::types::tuple::value::Tuple;
use crate::values::unpack::incorrect_parameter_type;

// `i32::abs(i32::MIN)` panics as `i32::MIN` has no corresponding
// positive value that fits inside `i32`. For this edge case,
// let's just hardcode the results.
const I32_MIN_OCTAL: &str = "-20000000000";
const I32_MIN_HEX: &str = "-80000000";

/// Operator `%` format or evaluation errors
#[derive(Clone, Dupe, Debug, Error)]
enum StringInterpolationError {
    #[error("Too many arguments for format string")]
    TooManyParameters,
    #[error("Not enough arguments for format string")]
    NotEnoughParameters,
    #[error("Incomplete format")]
    IncompleteFormat,
    #[error("Unsupported format character: {0:?}")]
    UnsupportedFormatCharacter(char),
    #[error("Expecting format character (internal error)")]
    ExpectingFormatCharacter,
}

enum PercentSFormat {
    /// `%s`.
    Str,
    /// `%r`.
    Repr,
    /// `%d`.
    Dec,
    /// `%o`.
    Oct,
    /// `%x`.
    Hex,
    /// `%X`.
    HexUpper,
    /// `%e`.
    Exp,
    /// `%E`.
    ExpUpper,
    /// `%f` or `%F`.
    Float,
    /// `%g`.
    FloatCompact,
    /// `%G`.
    FloatCompactUpper,
}

struct PercentFormatParser<'a> {
    rem: &'a str,
}

struct Item<'a> {
    literal: &'a str,
    format: Option<PercentSFormat>,
}

impl<'a> Iterator for PercentFormatParser<'a> {
    type Item = anyhow::Result<Item<'a>>;

    #[allow(clippy::collapsible_else_if)] // Makes code more readable.
    fn next(&mut self) -> Option<Self::Item> {
        let index_of_percent = self.rem.bytes().position(|c| c == b'%');
        if let Some(index_of_percent) = index_of_percent {
            let prev_rem = self.rem;
            let (literal, rem) = self.rem.split_at(index_of_percent);
            match rem.as_bytes().get(1) {
                None => return Some(Err(StringInterpolationError::IncompleteFormat.into())),
                Some(f) => {
                    let res = match f {
                        b'%' => {
                            // Include the percent in the literal.
                            let literal = &prev_rem[..index_of_percent + 1];
                            Item {
                                literal,
                                format: None,
                            }
                        }
                        b's' => Item {
                            literal,
                            format: Some(PercentSFormat::Str),
                        },
                        b'r' => Item {
                            literal,
                            format: Some(PercentSFormat::Repr),
                        },
                        b'd' => Item {
                            literal,
                            format: Some(PercentSFormat::Dec),
                        },
                        b'o' => Item {
                            literal,
                            format: Some(PercentSFormat::Oct),
                        },
                        b'x' => Item {
                            literal,
                            format: Some(PercentSFormat::Hex),
                        },
                        b'X' => Item {
                            literal,
                            format: Some(PercentSFormat::HexUpper),
                        },
                        b'e' => Item {
                            literal,
                            format: Some(PercentSFormat::Exp),
                        },
                        b'E' => Item {
                            literal,
                            format: Some(PercentSFormat::ExpUpper),
                        },
                        b'f' | b'F' => Item {
                            literal,
                            format: Some(PercentSFormat::Float),
                        },
                        b'g' => Item {
                            literal,
                            format: Some(PercentSFormat::FloatCompact),
                        },
                        b'G' => Item {
                            literal,
                            format: Some(PercentSFormat::FloatCompactUpper),
                        },
                        _ => {
                            // Note we need to find the second character, not the second byte.
                            let Some(c) = rem.chars().nth(1) else {
                                return Some(Err(
                                    StringInterpolationError::ExpectingFormatCharacter.into(),
                                ));
                            };
                            return Some(Err(
                                StringInterpolationError::UnsupportedFormatCharacter(c).into(),
                            ));
                        }
                    };
                    // We reach here only if format character is ASCII,
                    // so we can safely skip 2 bytes.
                    self.rem = &rem[2..];
                    Some(Ok(res))
                }
            }
        } else {
            if self.rem.is_empty() {
                None
            } else {
                let literal = self.rem;
                self.rem = "";
                Some(Ok(Item {
                    literal,
                    format: None,
                }))
            }
        }
    }
}

pub(crate) fn percent(format: &str, value: Value) -> crate::Result<String> {
    // NOTE(nga): use could reuse `Evaluator::string_pool` here, but
    //   * we don't have access to `Evaluator` in `StarlarkValue::percent`
    //   * after single %s made intrinsic, this code is not that hot now

    // random guess as a baseline capacity
    let mut res = String::with_capacity(format.len() + 20);
    write_percent(format, value, &mut res, None, |res, value, mode| {
        match mode {
            ValueTextMode::Str => value.collect_str(res),
            ValueTextMode::Repr => value.collect_repr(res),
            ValueTextMode::Display => unreachable!(),
        }
        Ok(())
    })?;
    Ok(res)
}

pub(crate) fn percent_bounded<'v>(
    format: &str,
    value: Value<'v>,
    heap: Heap<'v>,
) -> crate::Result<StringValue<'v>> {
    let Some(budget) = heap.execution_budget() else {
        return Ok(heap.alloc_str(&percent(format, value)?));
    };
    budget.charge(1 + ExecutionBudget::text_work(format.len()), 0)?;
    let mut res = BoundedText::new(budget, usize::MAX);
    write_percent(format, value, &mut res, Some(budget), |res, value, mode| {
        Ok(res.write_value(value, mode)?)
    })?;
    Ok(heap.alloc_budgeted_str(res.as_str())?)
}

fn write_percent<'v, W: Write>(
    format: &str,
    value: Value<'v>,
    res: &mut W,
    budget: Option<&ExecutionBudget>,
    mut write_value: impl FnMut(&mut W, Value<'v>, ValueTextMode) -> crate::Result<()>,
) -> crate::Result<()> {
    let tuple = Tuple::from_value(value);
    let one = &[value];
    let values = match &tuple {
        Some(xs) => xs.content(),
        None => one,
    };
    let mut values = values.iter().copied();
    let mut next_value = || -> crate::Result<Value> {
        if let Some(budget) = budget {
            budget.charge(1, 0)?;
        }
        values
            .next()
            .ok_or_else(|| crate::Error::new_other(StringInterpolationError::NotEnoughParameters))
    };

    // because of the way format is defined, we can deal with it as bytes
    for item in (PercentFormatParser { rem: format }) {
        let item = item?;
        check_format(res.write_str(item.literal), budget)?;
        let formatted = match item.format {
            None => continue,
            Some(PercentSFormat::Str) => {
                write_value(res, next_value()?, ValueTextMode::Str)?;
                continue;
            }
            Some(PercentSFormat::Repr) => {
                write_value(res, next_value()?, ValueTextMode::Repr)?;
                continue;
            }
            Some(PercentSFormat::Dec) => {
                let value = next_value()?;
                match value.unpack_num() {
                    Some(NumRef::Int(StarlarkIntRef::Small(v))) => {
                        write!(res, "{}", v.to_i32())
                    }
                    Some(NumRef::Int(StarlarkIntRef::Big(v))) => {
                        check_big_int(budget)?;
                        write!(res, "{}", v.get())
                    }
                    Some(NumRef::Float(v)) => {
                        match NumRef::Float(StarlarkFloat(v.0.trunc())).as_int() {
                            Some(v) => write!(res, "{v}"),
                            None => return ValueError::unsupported_type(value, "format(%d)"),
                        }
                    }
                    None => return ValueError::unsupported_type(value, "format(%d)"),
                }
            }
            Some(PercentSFormat::Oct) => {
                let value = next_value()?;
                match value.unpack_num() {
                    Some(NumRef::Int(StarlarkIntRef::Small(v))) => {
                        let v = v.to_i32();
                        match v.checked_abs() {
                            Some(vp) => {
                                write!(res, "{}{:o}", if v < 0 { "-" } else { "" }, vp)
                            }
                            None => write!(res, "{I32_MIN_OCTAL}"),
                        }
                    }
                    Some(NumRef::Int(StarlarkIntRef::Big(v))) => {
                        check_big_int(budget)?;
                        let v = v.get();
                        write!(
                            res,
                            "{}{:o}",
                            if v.is_negative() { "-" } else { "" },
                            v.abs()
                        )
                    }
                    Some(NumRef::Float(_)) | None => {
                        return ValueError::unsupported_type(value, "format(%o)");
                    }
                }
            }
            Some(PercentSFormat::Hex) => {
                let value = next_value()?;
                match value.unpack_num() {
                    Some(NumRef::Int(StarlarkIntRef::Small(v))) => {
                        let v = v.to_i32();
                        match v.checked_abs() {
                            Some(vp) => {
                                write!(res, "{}{:x}", if v < 0 { "-" } else { "" }, vp)
                            }
                            None => write!(res, "{I32_MIN_HEX}"),
                        }
                    }
                    Some(NumRef::Int(StarlarkIntRef::Big(v))) => {
                        check_big_int(budget)?;
                        let v = v.get();
                        write!(
                            res,
                            "{}{:x}",
                            if v.is_negative() { "-" } else { "" },
                            v.abs()
                        )
                    }
                    Some(NumRef::Float(_)) | None => {
                        return ValueError::unsupported_type(value, "format(%x)");
                    }
                }
            }
            Some(PercentSFormat::HexUpper) => {
                let value = next_value()?;
                match value.unpack_num() {
                    Some(NumRef::Int(StarlarkIntRef::Small(v))) => {
                        let v = v.to_i32();
                        match v.checked_abs() {
                            Some(vp) => {
                                write!(res, "{}{:X}", if v < 0 { "-" } else { "" }, vp)
                            }
                            None => write!(res, "{I32_MIN_HEX}"),
                        }
                    }
                    Some(NumRef::Int(StarlarkIntRef::Big(v))) => {
                        check_big_int(budget)?;
                        let v = v.get();
                        write!(
                            res,
                            "{}{:X}",
                            if v.is_negative() { "-" } else { "" },
                            v.abs()
                        )
                    }
                    Some(NumRef::Float(_)) | None => {
                        return ValueError::unsupported_type(value, "format(%X)");
                    }
                }
            }
            Some(conversion) => {
                let v = float_arg(next_value()?, budget)?;
                match conversion {
                    PercentSFormat::Exp => float::write_scientific(res, v, 'e', false),
                    PercentSFormat::ExpUpper => float::write_scientific(res, v, 'E', false),
                    PercentSFormat::Float => float::write_decimal(res, v),
                    PercentSFormat::FloatCompact => float::write_compact(res, v, 'e'),
                    PercentSFormat::FloatCompactUpper => float::write_compact(res, v, 'E'),
                    _ => unreachable!(),
                }
            }
        };
        check_format(formatted, budget)?;
    }
    if values.next().is_some() {
        Err(crate::Error::new_other(
            StringInterpolationError::TooManyParameters,
        ))
    } else {
        Ok(())
    }
}

fn check_format(result: fmt::Result, budget: Option<&ExecutionBudget>) -> crate::Result<()> {
    match budget {
        Some(budget) => result.map_err(|_| budget.refuse(ExecutionLimit::TextFormatting).into()),
        None => {
            result.unwrap();
            Ok(())
        }
    }
}

fn check_big_int(budget: Option<&ExecutionBudget>) -> crate::Result<()> {
    if let Some(budget) = budget {
        // The dependency converts big integers using host radix scratch.
        return Err(budget.refuse(ExecutionLimit::UnsupportedText("int")).into());
    }
    Ok(())
}

fn float_arg(value: Value, budget: Option<&ExecutionBudget>) -> crate::Result<f64> {
    let Some(budget) = budget else {
        return Ok(NumRef::unpack_param(value)?.as_float());
    };
    match value.unpack_num() {
        Some(NumRef::Int(StarlarkIntRef::Big(_))) => {
            // Bound the operand before calling the dependency's conversion.
            Err(budget.refuse(ExecutionLimit::UnsupportedText("int")).into())
        }
        Some(number) => Ok(number.as_float()),
        None => Err(incorrect_parameter_type(
            NumRef::starlark_type_repr(),
            value.bounded_type_description(budget)?,
        )),
    }
}

fn finish_percent_segment(
    format: &str,
    start: usize,
    end: usize,
    buffer: Option<String>,
) -> Cow<'_, str> {
    match buffer {
        Some(mut buffer) => {
            buffer.push_str(&format[start..end]);
            Cow::Owned(buffer)
        }
        None => Cow::Borrowed(&format[start..end]),
    }
}

/// Try parse `"aaa%sbbb"` and return `("aaa", "bbb")`, borrowing unescaped text.
pub(crate) fn parse_percent_s_one(format: &str) -> Option<(Cow<'_, str>, Cow<'_, str>)> {
    let mut before = None;
    let mut fragment_start = 0;
    let mut chars = format.char_indices();
    let capture = loop {
        match chars.next()? {
            (index, '%') => match chars.next()?.1 {
                '%' => {
                    before
                        .get_or_insert_with(String::new)
                        .push_str(&format[fragment_start..index + 1]);
                    fragment_start = index + 2;
                }
                's' => break index,
                _ => return None,
            },
            _ => {}
        }
    };
    let before = finish_percent_segment(format, fragment_start, capture, before);
    let mut after = None;
    let mut fragment_start = capture + 2;
    loop {
        match chars.next() {
            Some((index, '%')) => match chars.next()?.1 {
                '%' => {
                    after
                        .get_or_insert_with(String::new)
                        .push_str(&format[fragment_start..index + 1]);
                    fragment_start = index + 2;
                }
                _ => return None,
            },
            Some(_) => {}
            None => break,
        }
    }
    let after = finish_percent_segment(format, fragment_start, format.len(), after);
    Some((before, after))
}

/// Evaluate `"<before>%s<after>" % arg`.
pub(crate) fn percent_s_one<'v>(
    before: &str,
    arg: Value<'v>,
    after: &str,
    heap: Heap<'v>,
) -> crate::Result<StringValue<'v>> {
    if let Some(budget) = heap.execution_budget() {
        budget.charge(1, 0)?;
        return format_one_bounded(before, percent_s_arg(arg)?, after, heap);
    }
    Ok(match StringValue::new(arg) {
        Some(arg) if before.is_empty() && after.is_empty() => arg,
        Some(arg) => heap.alloc_str_concat3(before, &arg, after),
        None => format_one(before, percent_s_arg(arg)?, after, heap),
    })
}

fn percent_s_arg(arg: Value<'_>) -> crate::Result<Value<'_>> {
    match Tuple::from_value(arg) {
        Some(tuple) => match tuple.content() {
            [] => Err(crate::Error::new_other(
                StringInterpolationError::NotEnoughParameters,
            )),
            [value] => Ok(*value),
            [_, _, ..] => Err(crate::Error::new_other(
                StringInterpolationError::TooManyParameters,
            )),
        },
        None => Ok(arg),
    }
}

#[cfg(test)]
#[path = "percent_budget_tests.rs"]
mod percent_budget_tests;

#[cfg(test)]
#[path = "percent_s_memory_tests.rs"]
mod percent_s_memory_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert;

    #[test]
    fn test_incomplete_format() {
        assert::fail("'%' % ()", "Incomplete format");
    }

    #[test]
    fn test_unsupported_format_character() {
        assert::fail("'xx%qxx' % (1,)", "Unsupported format character: 'q'");
    }

    #[test]
    fn test_parse_percent_s_one() {
        assert_eq!(
            Some(("abc".into(), "def".into())),
            parse_percent_s_one("abc%sdef")
        );
        assert_eq!(
            Some(("a%b".into(), "c%d%".into())),
            parse_percent_s_one("a%%b%sc%%d%%")
        );
        assert_eq!(None, parse_percent_s_one("a%"));
        assert_eq!(None, parse_percent_s_one("a%s%"));
        assert_eq!(None, parse_percent_s_one("a%s%s"));
        assert_eq!(None, parse_percent_s_one("%d"));
    }

    #[test]
    fn test_type_support_d() {
        assert::eq("'%d' % (-123,)", "'-123'");
        assert::eq("'%d' % (-12345678901234567890,)", "'-12345678901234567890'");
        assert::eq("'%d' % (-123.0,)", "'-123'");

        assert::fail(
            "'%d' % (True,)",
            "Operation `format(%d)` not supported on type `bool`",
        );
        assert::fail(
            "'%d' % ('abc',)",
            "Operation `format(%d)` not supported on type `string`",
        );
        assert::fail(
            "'%d' % ([],)",
            "Operation `format(%d)` not supported on type `list`",
        );
    }

    #[test]
    fn test_type_support_o() {
        assert::eq("'%o' % (-123,)", "'-173'");
        assert::eq(
            "'%o' % (-12345678901234567890,)",
            "'-1255245230635307605322'",
        );

        assert::fail(
            "'%o' % (-123.0,)",
            "Operation `format(%o)` not supported on type `float`",
        );
        assert::fail(
            "'%o' % (True,)",
            "Operation `format(%o)` not supported on type `bool`",
        );
        assert::fail(
            "'%o' % ('abc',)",
            "Operation `format(%o)` not supported on type `string`",
        );
        assert::fail(
            "'%o' % ([],)",
            "Operation `format(%o)` not supported on type `list`",
        );
    }

    #[test]
    fn test_type_support_x() {
        assert::eq("'%x' % (-123,)", "'-7b'");
        assert::eq("'%x' % (-12345678901234567890,)", "'-ab54a98ceb1f0ad2'");

        assert::fail(
            "'%x' % (-123.0,)",
            "Operation `format(%x)` not supported on type `float`",
        );
        assert::fail(
            "'%x' % (True,)",
            "Operation `format(%x)` not supported on type `bool`",
        );
        assert::fail(
            "'%x' % ('abc',)",
            "Operation `format(%x)` not supported on type `string`",
        );
        assert::fail(
            "'%x' % ([],)",
            "Operation `format(%x)` not supported on type `list`",
        );
    }

    #[test]
    fn test_type_support_e() {
        assert::eq("'%e' % (-123,)", "'-1.230000e+02'");
        assert::eq("'%e' % (-12345678901234567890,)", "'-1.234568e+19'");
        assert::eq("'%e' % (-123.0,)", "'-1.230000e+02'");

        assert::fail(
            "'%e' % (True,)",
            "Type of parameters mismatch, expected `float | int`, actual `bool (repr: True)`",
        );
        assert::fail(
            "'%e' % ('abc',)",
            "Type of parameters mismatch, expected `float | int`, actual `string (repr:",
        );
        assert::fail(
            "'%e' % ([],)",
            "Type of parameters mismatch, expected `float | int`, actual `list (repr",
        );
    }

    #[test]
    fn test_int_min() {
        // 2147483647 is `i32::MIN + 1`, it has corresponding positive value in i32.
        // 2147483648 is `i32::MIN`, it has no corresponding positive value in i32.
        assert::eq("'%o' % (-2147483647,)", "'-17777777777'");
        assert::eq("'%o' % (-2147483648,)", "'-20000000000'");

        assert::eq("'%x' % (-2147483647,)", "'-7fffffff'");
        assert::eq("'%x' % (-2147483648,)", "'-80000000'");
    }
}
