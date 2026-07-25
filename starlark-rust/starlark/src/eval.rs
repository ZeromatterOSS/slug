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

//! Evaluate some code, typically done by creating an [`Evaluator`], then calling
//! [`eval_module`](Evaluator::eval_module).

pub(crate) mod bc;
pub(crate) mod compiler;
mod params;
pub(crate) mod runtime;
pub(crate) mod soft_error;

use std::collections::HashMap;
use std::mem;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use dupe::Dupe;
pub use runtime::arguments::Arguments;
pub use runtime::before_stmt::BeforeStmtFuncDyn;
pub use runtime::evaluator::CallStackCheckpoint;
pub use runtime::evaluator::Evaluator;
pub use runtime::file_loader::FileLoader;
pub use runtime::file_loader::ReturnFileLoader;
pub use runtime::params::parser::ParametersParser;
pub use runtime::params::spec::ParametersSpec;
pub use runtime::params::spec::ParametersSpecParam;
pub use runtime::profile::data::ProfileData;
pub use runtime::profile::mode::ProfileMode;
pub use soft_error::SoftErrorHandler;
pub use starlark_syntax::call_stack::CallStack;
use starlark_syntax::slice_vec_ext::SliceExt;
use starlark_syntax::syntax::module::AstModule;
use starlark_syntax::syntax::module::AstModuleFields;

use crate::collections::symbol::symbol::Symbol;
use crate::docs::DocString;
use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::compiler::Compiler;
use crate::eval::compiler::def::DefInfo;
use crate::eval::compiler::scope::ModuleScopes;
use crate::eval::compiler::scope::ScopeId;
use crate::eval::compiler::scope::scope_resolver_globals::ScopeResolverGlobals;
pub use crate::eval::params::param_specs;
use crate::eval::runtime::arguments::ArgNames;
use crate::eval::runtime::arguments::ArgumentsFull;
use crate::eval::runtime::evaluator;
use crate::syntax::DialectTypes;
use crate::values::Value;

/// Opaque reusable bytecode bound to one module's frozen heap and slots.
pub struct PreparedModule<'v> {
    module: &'v Module,
    bytecode: crate::eval::compiler::module::PreparedModuleBytecode,
    def_info: crate::values::FrozenRef<'static, DefInfo>,
}

#[derive(Debug, thiserror::Error)]
enum PreparedModuleError {
    #[error("prepared module registry was already installed")]
    RegistryAlreadyInstalled,
    #[error("prepared module registry is not installed")]
    RegistryNotInstalled,
    #[error("prepared module index {0} is out of range")]
    IndexOutOfRange(usize),
}

impl<'v, 'a, 'e> Evaluator<'v, 'a, 'e> {
    fn with_module<R>(
        &mut self,
        module: &'v Module,
        suspend_gc: bool,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let old_module = mem::replace(&mut self.module_env, module);
        let old_disable_gc = self.disable_gc;
        if suspend_gc || !std::ptr::eq(old_module, module) {
            self.disable_gc = true;
        }
        let result = f(self);
        self.module_env = old_module;
        self.disable_gc = old_disable_gc;
        result
    }

    /// Scope-check and compile `ast` for `module` without running it.
    pub fn prepare_module_in(
        &mut self,
        module: &'v Module,
        ast: AstModule,
        globals: &Globals,
    ) -> crate::Result<PreparedModule<'v>> {
        self.with_module(module, false, |eval| {
            let (codemap, statement, dialect, typecheck) = ast.into_parts();
            let codemap = eval.module_env.frozen_heap().alloc_any(codemap.dupe());
            let globals = eval.module_env.frozen_heap().alloc_any(globals.dupe());
            if let Some(docstring) = DocString::extract_raw_starlark_docstring(&statement) {
                eval.module_env.set_docstring(docstring);
            }
            let ModuleScopes {
                cst,
                module_slot_count,
                scope_data,
                top_level_stmt_count,
            } = ModuleScopes::check_module_err(
                eval.module_env.mutable_names(),
                eval.module_env.frozen_heap(),
                &HashMap::new(),
                statement,
                ScopeResolverGlobals {
                    globals: Some(globals),
                },
                codemap,
                &dialect,
            )?;
            let scope_names = scope_data.get_scope(ScopeId::module());
            let local_names = eval.frozen_heap().alloc_any_slice(&scope_names.used);
            eval.module_env.slots().ensure_slots(module_slot_count);
            let def_info = eval.module_env.frozen_heap().alloc_any(DefInfo::for_module(
                codemap,
                local_names,
                eval.module_env
                    .frozen_heap()
                    .alloc_any_slice(&scope_names.parent),
                globals,
            ));
            let old_def_info = mem::replace(&mut eval.module_def_info, def_info);
            let mut compiler = Compiler {
                scope_data,
                locals: Vec::new(),
                globals,
                codemap,
                eval,
                check_types: dialect.enable_types == DialectTypes::Enable,
                top_level_stmt_count,
                typecheck,
            };
            let bytecode = compiler.prepare_module(cst, local_names);
            compiler.eval.module_def_info = old_def_info;
            bytecode
                .map(|bytecode| PreparedModule {
                    module,
                    bytecode,
                    def_info,
                })
                .map_err(|e| e.into_error())
        })
    }

    /// Scope-check and compile `ast` for this evaluator's current module.
    pub fn prepare_module(
        &mut self,
        ast: AstModule,
        globals: &Globals,
    ) -> crate::Result<PreparedModule<'v>> {
        self.prepare_module_in(self.module_env, ast, globals)
    }

    /// Install the one-shot prepared-program registry before execution.
    pub fn set_prepared_modules(&mut self, modules: Vec<PreparedModule<'v>>) -> crate::Result<()> {
        if self.prepared_modules.is_some() {
            return Err(crate::Error::new_other(
                PreparedModuleError::RegistryAlreadyInstalled,
            ));
        }
        self.prepared_modules = Some(modules.into());
        Ok(())
    }

    /// Execute the registered program at `index` without exposing its contents.
    pub fn eval_prepared_module_index(&mut self, index: usize) -> crate::Result<Value<'v>> {
        let modules = self
            .prepared_modules
            .dupe()
            .ok_or_else(|| crate::Error::new_other(PreparedModuleError::RegistryNotInstalled))?;
        let prepared = modules
            .get(index)
            .ok_or_else(|| crate::Error::new_other(PreparedModuleError::IndexOutOfRange(index)))?;
        self.eval_prepared_module(prepared)
    }

    /// Execute a prepared program, preserving the native caller frame when nested.
    pub fn eval_prepared_module(
        &mut self,
        prepared: &PreparedModule<'v>,
    ) -> crate::Result<Value<'v>> {
        #[cfg(not(target_arch = "wasm32"))]
        let start = Instant::now();
        let initial_call = self.call_stack_count() == 0;
        self.with_module(prepared.module, !initial_call, |eval| {
            if initial_call {
                eval.call_stack.alloc_if_needed(
                    eval.max_callstack_size
                        .unwrap_or(evaluator::DEFAULT_STACK_SIZE),
                )?;
                eval.call_stack.push(Value::new_none(), None)?;
            }
            let old_def_info = mem::replace(&mut eval.module_def_info, prepared.def_info);
            let result =
                crate::eval::compiler::module::eval_prepared_module(eval, &prepared.bytecode)
                    .map_err(|e| e.into_error());
            if initial_call {
                eval.call_stack.pop();
            }
            eval.module_def_info = old_def_info;
            #[cfg(not(target_arch = "wasm32"))]
            eval.module_env.add_eval_duration(start.elapsed());
            result
        })
    }

    /// Evaluate an [`AstModule`] with this [`Evaluator`], modifying the in-scope
    /// [`Module`](crate::environment::Module) as appropriate.
    pub fn eval_module(&mut self, ast: AstModule, globals: &Globals) -> crate::Result<Value<'v>> {
        #[cfg(not(target_arch = "wasm32"))]
        let start = Instant::now();

        let (codemap, statement, dialect, typecheck) = ast.into_parts();

        let codemap = self.module_env.frozen_heap().alloc_any(codemap.dupe());

        let globals = self.module_env.frozen_heap().alloc_any(globals.dupe());

        if let Some(docstring) = DocString::extract_raw_starlark_docstring(&statement) {
            self.module_env.set_docstring(docstring)
        }

        let ModuleScopes {
            cst,
            module_slot_count,
            scope_data,
            top_level_stmt_count,
        } = ModuleScopes::check_module_err(
            self.module_env.mutable_names(),
            self.module_env.frozen_heap(),
            &HashMap::new(),
            statement,
            ScopeResolverGlobals {
                globals: Some(globals),
            },
            codemap,
            &dialect,
        )?;

        let scope_names = scope_data.get_scope(ScopeId::module());
        let local_names = self.frozen_heap().alloc_any_slice(&scope_names.used);

        self.module_env.slots().ensure_slots(module_slot_count);
        let old_def_info = mem::replace(
            &mut self.module_def_info,
            self.module_env.frozen_heap().alloc_any(DefInfo::for_module(
                codemap,
                local_names,
                self.module_env
                    .frozen_heap()
                    .alloc_any_slice(&scope_names.parent),
                globals,
            )),
        );

        self.call_stack.alloc_if_needed(
            self.max_callstack_size
                .unwrap_or(evaluator::DEFAULT_STACK_SIZE),
        )?;

        // Set up the world to allow evaluation (do NOT use ? from now on)

        self.call_stack.push(Value::new_none(), None).unwrap();

        // Evaluation
        let mut compiler = Compiler {
            scope_data,
            locals: Vec::new(),
            globals,
            codemap,
            eval: self,
            check_types: dialect.enable_types == DialectTypes::Enable,
            top_level_stmt_count,
            typecheck,
        };

        let res = compiler.eval_module(cst, local_names);

        // Clean up the world, putting everything back
        self.call_stack.pop();

        self.module_def_info = old_def_info;

        #[cfg(not(target_arch = "wasm32"))]
        self.module_env.add_eval_duration(start.elapsed());

        // Return the result of evaluation
        res.map_err(|e| e.into_error())
    }

    /// Evaluate a function stored in a [`Value`], passing in `positional` and `named` arguments.
    pub fn eval_function(
        &mut self,
        function: Value<'v>,
        positional: &[Value<'v>],
        named: &[(&str, Value<'v>)],
    ) -> crate::Result<Value<'v>> {
        let names = named.map(|(s, _)| (Symbol::new(s), self.heap().alloc_str(s)));
        let named = named.map(|x| x.1);
        let params = Arguments(ArgumentsFull {
            pos: positional,
            named: &named,
            names: ArgNames::new_check_unique(&names)?,
            args: None,
            kwargs: None,
        });
        self.call_stack.alloc_if_needed(
            self.max_callstack_size
                .unwrap_or(evaluator::DEFAULT_STACK_SIZE),
        )?;
        // eval_module pushes an "empty" call stack frame. other places expect that first frame to be ignorable, and
        // so we push an empty frame too (otherwise things would ignore this function's own frame).
        self.with_call_stack(Value::new_none(), None, |this| {
            function.invoke(&params, this)
        })
        .map_err(Into::into)
    }
}

#[cfg(test)]
mod prepared_module_tests {
    use super::*;
    use crate as starlark;
    use crate::environment::GlobalsBuilder;
    use crate::environment::Module;
    use crate::starlark_module;
    use crate::syntax::Dialect;
    use crate::values::none::NoneType;

    #[starlark_module]
    fn prepared_module_globals(builder: &mut GlobalsBuilder) {
        fn dispatch_prepared(index: i32, eval: &mut Evaluator) -> anyhow::Result<NoneType> {
            eval.trigger_gc();
            eval.eval_prepared_module_index(index as usize)
                .map_err(starlark::Error::into_anyhow)?;
            Ok(NoneType)
        }

        fn assert_gc_enabled(eval: &mut Evaluator) -> anyhow::Result<NoneType> {
            anyhow::ensure!(!eval.disable_gc, "GC is unexpectedly suspended");
            Ok(NoneType)
        }

        fn assert_gc_disabled(eval: &mut Evaluator) -> anyhow::Result<NoneType> {
            anyhow::ensure!(eval.disable_gc, "GC is not suspended");
            Ok(NoneType)
        }
    }

    fn parse(name: &str, source: &str) -> AstModule {
        AstModule::parse(name, source.to_owned(), &Dialect::Standard).unwrap()
    }

    #[test]
    fn prepared_registry_is_reusable_and_restores_after_error() {
        let root = Module::new();
        let child = Module::new();
        let globals = Globals::standard();
        let mut prepare = Evaluator::new(&root);
        let root_program = prepare
            .prepare_module(parse("root", "root_value = 1"), &globals)
            .unwrap();
        let child_program = prepare
            .prepare_module_in(&child, parse("child", "child_value = 1"), &globals)
            .unwrap();
        let error_program = prepare
            .prepare_module_in(&child, parse("bad", "1 // 0"), &globals)
            .unwrap();
        drop(prepare);

        let mut eval = Evaluator::new(&root);
        eval.set_prepared_modules(vec![root_program, child_program, error_program])
            .unwrap();
        eval.eval_prepared_module_index(0).unwrap();
        eval.eval_prepared_module_index(1).unwrap();
        eval.eval_prepared_module_index(1).unwrap();
        assert!(root.get("root_value").is_some());
        assert!(root.get("child_value").is_none());
        assert!(child.get("child_value").is_some());
        assert!(
            eval.eval_prepared_module_index(2)
                .unwrap_err()
                .to_string()
                .contains("bad")
        );
        assert!(std::ptr::eq(eval.module(), &root));
        assert!(!eval.disable_gc);
        assert_eq!(eval.call_stack_count(), 0);
        eval.eval_prepared_module_index(0).unwrap();
        assert!(
            eval.set_prepared_modules(Vec::new())
                .unwrap_err()
                .to_string()
                .contains("already installed")
        );
        assert!(
            eval.eval_prepared_module_index(3)
                .unwrap_err()
                .to_string()
                .contains("out of range")
        );
    }

    #[test]
    fn same_module_nested_dispatch_suspends_gc_compositionally() {
        let module = Module::new();
        let globals = GlobalsBuilder::extended()
            .with(prepared_module_globals)
            .build();
        let mut prepare = Evaluator::new(&module);
        let root_program = prepare
            .prepare_module(
                parse(
                    "root",
                    "assert_gc_enabled()\ndispatch_prepared(1)\nassert_gc_enabled()",
                ),
                &globals,
            )
            .unwrap();
        let child_program = prepare
            .prepare_module(
                parse("child", "assert_gc_disabled()\nchild_value = 1"),
                &globals,
            )
            .unwrap();
        drop(prepare);

        let mut eval = Evaluator::new(&module);
        eval.set_prepared_modules(vec![root_program, child_program])
            .unwrap();
        eval.eval_prepared_module_index(0).unwrap();

        assert!(module.get("child_value").is_some());
        assert!(!eval.disable_gc);
        assert_eq!(eval.call_stack_count(), 0);
    }
}
