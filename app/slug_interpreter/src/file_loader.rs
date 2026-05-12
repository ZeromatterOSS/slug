/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use allocative::Allocative;
use derivative::Derivative;
use dupe::Dupe;
use slug_core::bzl::ImportPath;
use starlark::codemap::FileSpan;
use starlark::environment::FrozenModule;
use starlark::eval::FileLoader;
use starlark_map::ordered_map::OrderedMap;

use crate::paths::module::OwnedStarlarkModulePath;
use crate::paths::module::StarlarkModulePath;

#[derive(Default, Clone, Allocative, Debug)]
pub struct LoadedModules {
    pub map: OrderedMap<OwnedStarlarkModulePath, LoadedModule>,
}

impl LoadedModules {
    pub fn imports(&self) -> impl Iterator<Item = &ImportPath> {
        self.map.values().map(|module| match module.path() {
            StarlarkModulePath::LoadFile(p)
            | StarlarkModulePath::JsonFile(p)
            | StarlarkModulePath::TomlFile(p) => p,
            _ => panic!("imports should only be bzl, json, or toml files"),
        })
    }
}

pub trait LoadResolver {
    fn resolve_load(
        &self,
        path: &str,
        location: Option<&FileSpan>,
    ) -> slug_error::Result<OwnedStarlarkModulePath>;
}

pub struct ModuleDeps(pub Vec<LoadedModule>);

impl ModuleDeps {
    pub fn get_loaded_modules(&self) -> LoadedModules {
        let mut map = OrderedMap::with_capacity(self.0.len());
        for dep in &*self.0 {
            map.insert(dep.path().to_owned(), dep.dupe());
        }
        LoadedModules { map }
    }
}

#[derive(Clone, Dupe, Allocative, Debug)]
pub struct LoadedModule(Arc<LoadedModuleData>);

#[derive(Derivative, Allocative)]
#[derivative(Debug)]
struct LoadedModuleData {
    path: OwnedStarlarkModulePath,
    #[derivative(Debug = "ignore")]
    imports: Vec<OwnedStarlarkModulePath>,
    #[derivative(Debug = "ignore")]
    env: FrozenModule,
}

impl LoadedModule {
    pub fn new(
        path: OwnedStarlarkModulePath,
        loaded_modules: LoadedModules,
        env: FrozenModule,
    ) -> Self {
        let imports = loaded_modules.map.keys().cloned().collect();
        Self::new_with_direct_imports(path, imports, env)
    }

    pub fn new_with_direct_imports(
        path: OwnedStarlarkModulePath,
        imports: Vec<OwnedStarlarkModulePath>,
        env: FrozenModule,
    ) -> Self {
        Self(Arc::new(LoadedModuleData { path, imports, env }))
    }

    pub fn imports(&self) -> impl Iterator<Item = &ImportPath> {
        self.0.imports.iter().map(|path| match path.borrow() {
            StarlarkModulePath::LoadFile(path)
            | StarlarkModulePath::JsonFile(path)
            | StarlarkModulePath::TomlFile(path) => path,
            StarlarkModulePath::BxlFile(_) => panic!("imports should not contain bxl files"),
        })
    }

    pub fn direct_imports(&self) -> &[OwnedStarlarkModulePath] {
        &self.0.imports
    }

    pub fn import_count(&self) -> usize {
        self.0.imports.len()
    }

    pub fn import_path_bytes(&self) -> usize {
        self.0
            .imports
            .iter()
            .map(|path| path.to_string().len())
            .sum()
    }

    pub fn path(&self) -> StarlarkModulePath<'_> {
        self.0.path.borrow()
    }

    pub fn env(&self) -> &FrozenModule {
        &self.0.env
    }
}

pub struct InterpreterFileLoader {
    loaded_modules: LoadedModules,
    info: Arc<dyn LoadResolver>,
}

impl InterpreterFileLoader {
    pub fn new(loaded_modules: LoadedModules, info: Arc<dyn LoadResolver>) -> Self {
        Self {
            loaded_modules,
            info,
        }
    }
}

fn to_diagnostic(err: &slug_error::Error, id: &str) -> slug_error::Error {
    slug_error::slug_error!(
        slug_error::ErrorTag::Tier0,
        "UnknownError in {}: {}",
        id,
        err
    )
}

impl InterpreterFileLoader {
    /// Used for looking up modules by id.
    fn find_module(&self, id: StarlarkModulePath) -> slug_error::Result<&FrozenModule> {
        match self.loaded_modules.map.get(&id) {
            Some(v) => Ok(&v.0.env),
            None => Err(to_diagnostic(
                &slug_error::slug_error!(
                    slug_error::ErrorTag::Input,
                    "Should have had an env for {}. had <{:?}>",
                    id,
                    self.loaded_modules.map.keys().collect::<Vec<_>>()
                ),
                &id.to_string(),
            )),
        }
    }
}

impl FileLoader for InterpreterFileLoader {
    /// The Interpreter will call this to resolve and load imports for load()
    /// statements.
    fn load(&self, path: &str) -> starlark::Result<FrozenModule> {
        match self.info.resolve_load(path, None) {
            Ok(import) => Ok(self.find_module(import.borrow())?.dupe()),
            Err(e) => Err(to_diagnostic(&e, path).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use slug_error::slug_error;
    use starlark::environment::Module;

    use super::*;

    struct TestLoadResolver {}

    impl LoadResolver for TestLoadResolver {
        fn resolve_load(
            &self,
            path: &str,
            _location: Option<&FileSpan>,
        ) -> slug_error::Result<OwnedStarlarkModulePath> {
            match path {
                "//some/package:import.bzl" => Ok(OwnedStarlarkModulePath::LoadFile(
                    ImportPath::testing_new("root//some/package:import.bzl"),
                )),
                "cell1//next/package:import.bzl" => Ok(OwnedStarlarkModulePath::LoadFile(
                    ImportPath::testing_new("cell1//next/package:import.bzl"),
                )),
                "alias2//last/package:import.bzl" => Ok(OwnedStarlarkModulePath::LoadFile(
                    ImportPath::testing_new("cell2//last/package:import.bzl"),
                )),
                _ => Err(slug_error!(slug_error::ErrorTag::Tier0, "error")),
            }
        }
    }

    fn resolver() -> Arc<TestLoadResolver> {
        Arc::new(TestLoadResolver {})
    }

    fn env(name: StarlarkModulePath) -> FrozenModule {
        let m = Module::new();
        m.set("name", m.heap().alloc(name.to_string()));
        m.freeze().unwrap()
    }

    fn loaded_modules() -> LoadedModules {
        let mut loaded_modules = LoadedModules::default();
        let resolver = resolver();

        let mut insert = |path| {
            let import_path = resolver.resolve_load(path, None).unwrap();
            let module = LoadedModule::new(
                import_path.clone(),
                LoadedModules::default(),
                env(import_path.borrow()),
            );
            loaded_modules.map.insert(import_path, module);
        };

        // Insert the same things that the TestLoadResolver supports.
        insert("//some/package:import.bzl");
        insert("cell1//next/package:import.bzl");
        insert("alias2//last/package:import.bzl");

        loaded_modules
    }

    #[test]
    fn no_resolution() -> slug_error::Result<()> {
        let path = "some//random:file.bzl".to_owned();
        let loader = InterpreterFileLoader::new(loaded_modules(), resolver());
        match loader.load(&path) {
            Ok(_) => panic!("Expected load failure for {path}"),
            Err(_) => {
                // TODO: verify the error is correct
            }
        }
        Ok(())
    }

    #[test]
    fn missing_in_loaded_modules() -> slug_error::Result<()> {
        let path = "cell1//next/package:import.bzl".to_owned();
        let resolver = resolver();
        let id = resolver.resolve_load(&path, None)?;

        let mut loaded_modules = loaded_modules();
        loaded_modules.map.remove(&id);
        let loader = InterpreterFileLoader::new(loaded_modules, resolver);
        match loader.load(&path) {
            Ok(_) => panic!("Expected load failure for {path}"),
            Err(_) => {
                // TODO: verify the error is correct
            }
        }
        Ok(())
    }

    #[test]
    fn valid_load() -> slug_error::Result<()> {
        let path = "cell1//next/package:import.bzl".to_owned();
        let resolver = resolver();
        let id = resolver.resolve_load(&path, None)?.to_string();

        let loader = InterpreterFileLoader::new(loaded_modules(), resolver);
        let loaded = loader.load(&path)?;

        let v = loaded.get("name").unwrap();
        assert_eq!(v.value().unpack_str(), Some(id.as_str()));

        Ok(())
    }

    #[test]
    fn valid_find() -> slug_error::Result<()> {
        let path = "cell1//next/package:import.bzl".to_owned();
        let resolver = resolver();
        let resolved = resolver.resolve_load(&path, None)?;
        let borrow = resolved.borrow();

        let loader = InterpreterFileLoader::new(loaded_modules(), resolver);
        let found = loader.find_module(borrow)?;

        let v = found.get("name").unwrap();
        assert_eq!(v.value().unpack_str(), Some(borrow.to_string().as_str()));

        Ok(())
    }
}
