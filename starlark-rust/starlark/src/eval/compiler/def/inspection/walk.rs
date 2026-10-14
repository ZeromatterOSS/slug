//! Exhaustive dependency walk of the retained pre-freeze IR.
use super::DefCodeInspection;
use super::DefDependency;
use super::InspectionBudget;
use super::ModuleAccess;
use super::OriginalModule;
use super::inspect_native_function;
use crate::eval::compiler::compr::ComprCompiled;
use crate::eval::compiler::def::ParameterCompiled;
use crate::eval::compiler::expr::Builtin1;
use crate::eval::compiler::expr::Builtin2;
use crate::eval::compiler::expr::ExprCompiled;
use crate::eval::compiler::span::IrSpanned;
use crate::eval::compiler::stmt::AssignCompiledValue;
use crate::eval::compiler::stmt::AssignModifyLhs;
use crate::eval::compiler::stmt::StmtCompiled;
use crate::eval::compiler::stmt::StmtsCompiled;
use crate::eval::runtime::frame_span::FrameSpan;
use crate::values::FrozenValue;
use crate::values::Value;
use crate::values::ValueLike;
use crate::values::typing::type_compiled::compiled::TypeCompiled;

pub(super) fn visit<'v>(
    code: DefCodeInspection<'v>,
    budget: &mut InspectionBudget,
    visitor: &mut dyn FnMut(DefDependency<'v>) -> crate::Result<()>,
) -> crate::Result<()> {
    Walker {
        module: code.module,
        budget,
        visitor,
    }
    .body(&code.info.body_stmts, 0)
}

struct Walker<'v, 'a> {
    module: OriginalModule<'v>,
    budget: &'a mut InspectionBudget,
    visitor: &'a mut dyn FnMut(DefDependency<'v>) -> crate::Result<()>,
}
impl<'v> Walker<'v, '_> {
    fn emit(&mut self, dependency: DefDependency<'v>, depth: usize) -> crate::Result<()> {
        self.budget.step(depth)?;
        (self.visitor)(dependency)
    }
    fn constant(&mut self, value: Value<'v>, depth: usize) -> crate::Result<()> {
        self.emit(
            match inspect_native_function(value) {
                Some(native) => DefDependency::NativeFunction(native),
                None => DefDependency::Constant(value),
            },
            depth,
        )
    }
    fn span(&mut self, span: FrameSpan, depth: usize) -> crate::Result<()> {
        self.emit(DefDependency::Source(span.span.file_span_ref()), depth)?;
        let mut frames = span.inlined_frames.frames;
        let mut frame_depth = depth;
        while let Some(frame) = frames {
            frame_depth += 1;
            self.emit(
                DefDependency::Source(frame.span.span.file_span_ref()),
                frame_depth,
            )?;
            // Keep the actual helper dependency even if compiler inlining removed its call.
            self.constant(frame.fun.to_value(), frame_depth)?;
            frames = frame.span.inlined_frames.frames;
        }
        Ok(())
    }
    fn module(
        &mut self,
        slot: crate::environment::slots::ModuleSlotId,
        access: ModuleAccess,
        depth: usize,
    ) -> crate::Result<()> {
        let (name, value) = self.module.slot(slot)?;
        self.emit(
            DefDependency::ModuleSymbol {
                name,
                value,
                access,
            },
            depth,
        )
    }
    fn ty(&mut self, ty: TypeCompiled<FrozenValue>, depth: usize) -> crate::Result<()> {
        self.budget.step(depth)?;
        self.emit(DefDependency::Type(ty.to_inner().to_value()), depth)
    }
    fn body(&mut self, body: &'v StmtsCompiled, depth: usize) -> crate::Result<()> {
        self.budget.step(depth)?;
        for statement in body.stmts() {
            self.statement(statement, depth + 1)?;
        }
        Ok(())
    }
    fn statement(
        &mut self,
        statement: &'v IrSpanned<StmtCompiled>,
        depth: usize,
    ) -> crate::Result<()> {
        self.budget.step(depth)?;
        self.span(statement.span, depth)?;
        match &statement.node {
            StmtCompiled::PossibleGc | StmtCompiled::Break | StmtCompiled::Continue => Ok(()),
            StmtCompiled::Return(value) | StmtCompiled::Expr(value) => {
                self.expression(value, depth + 1)
            }
            StmtCompiled::Assign(lhs, annotation, rhs) => {
                self.assign(lhs, depth + 1)?;
                if let Some(annotation) = annotation {
                    self.ty(annotation.node, depth + 1)?;
                }
                self.expression(rhs, depth + 1)
            }
            StmtCompiled::AssignModify(lhs, _, rhs) => {
                self.budget.step(depth + 1)?;
                match lhs {
                    AssignModifyLhs::Dot(value, _) => self.expression(value, depth + 2)?,
                    AssignModifyLhs::Array(value, index) => {
                        self.expression(value, depth + 2)?;
                        self.expression(index, depth + 2)?;
                    }
                    AssignModifyLhs::Module(slot) => {
                        self.module(slot.node, ModuleAccess::ReadWrite, depth + 2)?
                    }
                    AssignModifyLhs::Local(_) | AssignModifyLhs::LocalCaptured(_) => {}
                }
                self.expression(rhs, depth + 1)
            }
            StmtCompiled::If(parts) => {
                self.expression(&parts.0, depth + 1)?;
                self.body(&parts.1, depth + 1)?;
                self.body(&parts.2, depth + 1)
            }
            StmtCompiled::For(parts) => {
                self.assign(&parts.0, depth + 1)?;
                self.expression(&parts.1, depth + 1)?;
                self.body(&parts.2, depth + 1)
            }
        }
    }
    fn assign(
        &mut self,
        target: &'v IrSpanned<AssignCompiledValue>,
        depth: usize,
    ) -> crate::Result<()> {
        self.budget.step(depth)?;
        self.span(target.span, depth)?;
        match &target.node {
            AssignCompiledValue::Dot(receiver, _) => self.expression(receiver, depth + 1),
            AssignCompiledValue::Index(receiver, index) => {
                self.expression(receiver, depth + 1)?;
                self.expression(index, depth + 1)
            }
            AssignCompiledValue::Tuple(targets) => {
                for target in targets {
                    self.assign(target, depth + 1)?;
                }
                Ok(())
            }
            AssignCompiledValue::Module(slot, _) => {
                self.module(*slot, ModuleAccess::Write, depth + 1)
            }
            AssignCompiledValue::Local(_) | AssignCompiledValue::LocalCaptured(_) => Ok(()),
        }
    }
    fn expression(
        &mut self,
        expression: &'v IrSpanned<ExprCompiled>,
        depth: usize,
    ) -> crate::Result<()> {
        self.budget.step(depth)?;
        self.span(expression.span, depth)?;
        match &expression.node {
            ExprCompiled::Value(value) => self.constant(value.to_value(), depth + 1),
            ExprCompiled::Local(_) | ExprCompiled::LocalCaptured(_) => Ok(()),
            ExprCompiled::Module(slot) => self.module(*slot, ModuleAccess::Read, depth + 1),
            ExprCompiled::Tuple(values) | ExprCompiled::List(values) => {
                for value in values {
                    self.expression(value, depth + 1)?;
                }
                Ok(())
            }
            ExprCompiled::Dict(entries) => {
                for (key, value) in entries {
                    self.expression(key, depth + 1)?;
                    self.expression(value, depth + 1)?;
                }
                Ok(())
            }
            ExprCompiled::Compr(comprehension) => {
                self.budget.step(depth + 1)?;
                match comprehension {
                    ComprCompiled::List(value, _) => self.expression(value, depth + 2)?,
                    ComprCompiled::Dict(entries, _) => {
                        self.expression(&entries.0, depth + 2)?;
                        self.expression(&entries.1, depth + 2)?;
                    }
                }
                for clause in comprehension.clauses().inspection_clauses() {
                    self.budget.step(depth + 2)?;
                    self.assign(&clause.var, depth + 3)?;
                    self.expression(&clause.over, depth + 3)?;
                    for condition in &clause.ifs {
                        self.expression(condition, depth + 3)?;
                    }
                }
                Ok(())
            }
            ExprCompiled::If(parts) | ExprCompiled::Index2(parts) => {
                self.expression(&parts.0, depth + 1)?;
                self.expression(&parts.1, depth + 1)?;
                self.expression(&parts.2, depth + 1)
            }
            ExprCompiled::Slice(parts) => {
                self.expression(&parts.0, depth + 1)?;
                for value in [&parts.1, &parts.2, &parts.3].into_iter().flatten() {
                    self.expression(value, depth + 1)?;
                }
                Ok(())
            }
            ExprCompiled::Builtin1(builtin, operand) => {
                self.budget.step(depth + 1)?;
                match builtin {
                    Builtin1::TypeIs(value) => self.constant(value.to_value(), depth + 2)?,
                    Builtin1::PercentSOne(before, after) | Builtin1::FormatOne(before, after) => {
                        self.constant(before.to_value(), depth + 2)?;
                        self.constant(after.to_value(), depth + 2)?;
                    }
                    Builtin1::Dot(_)
                    | Builtin1::Minus
                    | Builtin1::Plus
                    | Builtin1::BitNot
                    | Builtin1::Not => {}
                }
                self.expression(operand, depth + 1)
            }
            ExprCompiled::Builtin2(builtin, parts) => {
                self.budget.step(depth + 1)?;
                match builtin {
                    Builtin2::Equals
                    | Builtin2::In
                    | Builtin2::Sub
                    | Builtin2::Add
                    | Builtin2::Multiply
                    | Builtin2::Percent
                    | Builtin2::Divide
                    | Builtin2::FloorDivide
                    | Builtin2::BitAnd
                    | Builtin2::BitOr
                    | Builtin2::BitXor
                    | Builtin2::LeftShift
                    | Builtin2::RightShift
                    | Builtin2::Compare(_)
                    | Builtin2::ArrayIndex => {}
                }
                self.expression(&parts.0, depth + 1)?;
                self.expression(&parts.1, depth + 1)
            }
            ExprCompiled::LogicalBinOp(_, parts) | ExprCompiled::Seq(parts) => {
                self.expression(&parts.0, depth + 1)?;
                self.expression(&parts.1, depth + 1)
            }
            ExprCompiled::Call(call) => {
                self.budget.step(depth + 1)?;
                self.span(call.node.call_site, depth + 1)?;
                self.expression(&call.node.fun, depth + 2)?;
                self.budget.step(depth + 2)?;
                for argument in &call.node.args.pos_named {
                    self.expression(argument, depth + 3)?;
                }
                for (_, name) in &call.node.args.names {
                    self.constant(name.to_value(), depth + 3)?;
                }
                if let Some(argument) = &call.node.args.args {
                    self.expression(argument, depth + 3)?;
                }
                if let Some(argument) = &call.node.args.kwargs {
                    self.expression(argument, depth + 3)?;
                }
                Ok(())
            }
            ExprCompiled::Def(definition) => {
                self.budget.step(depth + 1)?;
                for parameter in &definition.params.params {
                    self.budget.step(depth + 2)?;
                    match &parameter.node {
                        ParameterCompiled::Normal(_, annotation, default) => {
                            if let Some(annotation) = annotation {
                                self.ty(*annotation, depth + 3)?;
                            }
                            if let Some(default) = default {
                                self.expression(default, depth + 3)?;
                            }
                        }
                        ParameterCompiled::Args(_, annotation)
                        | ParameterCompiled::KwArgs(_, annotation) => {
                            if let Some(annotation) = annotation {
                                self.ty(*annotation, depth + 3)?;
                            }
                        }
                    }
                }
                if let Some(annotation) = definition.return_type {
                    self.ty(annotation, depth + 2)?;
                }
                let nested = DefCodeInspection {
                    info: definition.info.as_ref(),
                    module: self.module,
                };
                self.emit(DefDependency::NestedCode(nested), depth + 2)?;
                self.emit(DefDependency::Source(nested.signature_span()), depth + 2)?;
                self.body(&nested.info.body_stmts, depth + 2)
            }
        }
    }
}
