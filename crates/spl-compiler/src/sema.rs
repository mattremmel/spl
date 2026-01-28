//! Sema re-exports from spl-sema crate with package-level functions.
//!
//! This module re-exports the semantic analysis types from `spl_sema` and
//! adds package-level functions for multi-file compilation.

use rustc_hash::FxHashMap;
use spl_ast::Item;

use crate::package::Package;

// Re-export everything from spl-sema
pub use spl_sema::*;

// =============================================================================
// Package-Level Extensions
// =============================================================================

/// Extension trait for `ModuleTree` to work with `Package`.
///
/// This function is in spl-compiler because it depends on the Package type.
fn module_tree_from_package_structure(package: &Package) -> ModuleTree {
    let mut tree = ModuleTree::new();
    let root_id = tree.root_id();
    build_module_structure(&mut tree, root_id, package);
    tree
}

fn build_module_structure(tree: &mut ModuleTree, parent_id: ModuleId, package: &Package) {
    // Recursively create child modules for child modules
    for child_mod in package.modules() {
        let child_id = tree.add_child(parent_id, child_mod.name());
        build_module_structure(tree, child_id, child_mod);
    }
}

/// Resolve names in a package (multi-file compilation).
///
/// This function resolves all modules in the package hierarchy,
/// building a complete symbol table with cross-module references.
///
/// # Arguments
///
/// * `package` - The root package to resolve
///
/// # Returns
///
/// A `ResolveResult` containing the semantic context with resolved symbols
/// and any diagnostics produced during resolution.
pub fn resolve_package(package: &Package) -> ResolveResult {
    // Build module tree from package hierarchy
    let module_tree = module_tree_from_package_structure(package);
    let root_id = module_tree.root_id();

    // Create context with module tree for cross-module resolution
    let mut ctx = SemanticContext::with_module_tree(module_tree, root_id);
    define_builtins(&mut ctx);

    // Use a single Resolver for the entire package hierarchy
    let (resolutions, diagnostics, inline_module_scopes) = {
        let mut resolver = Resolver::new(&mut ctx);

        // Map from ModuleId to ScopeId (populated during item collection)
        let mut module_scopes: FxHashMap<ModuleId, ScopeId> = FxHashMap::default();

        // Phase 1: Collect all items from ALL modules (through Resolver to track resolutions)
        collect_all_items_through_resolver(package, &mut resolver, root_id, &mut module_scopes);

        // Phase 2: Resolve imports for all modules
        resolve_all_imports(package, &mut resolver, root_id, &module_scopes);

        // Phase 3: Resolve bodies for all modules
        resolve_all_bodies(package, &mut resolver, root_id, &module_scopes);

        (
            resolver.resolutions().clone(),
            resolver.take_diagnostics(),
            resolver.module_scopes().clone(),
        )
    };

    ResolveResult {
        ctx,
        resolutions,
        diagnostics,
        module_scopes: inline_module_scopes,
    }
}

/// Phase 1: Collect all items from all modules through Resolver.
///
/// This populates:
/// - The symbol table (via `Resolver.collect_item`)
/// - The resolutions map (span→DefId for definitions)
/// - The module tree (items and exports)
fn collect_all_items_through_resolver(
    package: &Package,
    resolver: &mut Resolver,
    module_id: ModuleId,
    module_scopes: &mut FxHashMap<ModuleId, ScopeId>,
) {
    // Enter a new scope for this module
    let module_scope = resolver.ctx().enter_scope(ScopeKind::Module);
    module_scopes.insert(module_id, module_scope);
    resolver.ctx().current_module = module_id;

    // Collect items (but not use declarations yet - those come in phase 2)
    let source_map = package.compilation_unit().source_map();
    for (file_id, source_file) in package.compilation_unit().source_files() {
        if let Some(path) = source_map.get_path(file_id) {
            resolver.set_current_file_path(path);
        }
        for item in source_file.items() {
            match &item {
                Item::Use(_) => {
                    // Skip use declarations in phase 1
                }
                _ => {
                    resolver.collect_item(&item);
                }
            }
        }
    }

    // Populate module tree with items from this package
    populate_module_tree_from_scope(package, resolver, module_id);

    // Recurse into child modules
    for child_mod in package.modules() {
        let child_id = {
            let tree = resolver.ctx().module_tree.as_ref().unwrap();
            let module = tree.get(module_id);
            let name_spur = tree.interner.get(child_mod.name());
            name_spur
                .and_then(|spur| module.children().get(&spur).copied())
                .expect("child module should exist in tree")
        };
        collect_all_items_through_resolver(child_mod, resolver, child_id, module_scopes);
    }

    // Exit this module's scope
    resolver.ctx().exit_scope();
}

/// Populate the module tree with items defined in the current scope.
fn populate_module_tree_from_scope(
    package: &Package,
    resolver: &mut Resolver,
    module_id: ModuleId,
) {
    for (_file_id, source_file) in package.compilation_unit().source_files() {
        for item in source_file.items() {
            if let Some((name, is_pub)) = get_item_name_and_visibility(&item) {
                let ctx = resolver.ctx();
                let name_spur = ctx.intern(&name);
                let current_scope = ctx.current_scope_id();
                if let Some(def_id) = ctx.lookup_in_scope(name_spur, current_scope) {
                    let tree = ctx.module_tree.as_mut().unwrap();
                    tree.add_item(module_id, &name, def_id);
                    if is_pub {
                        tree.add_export(module_id, &name, def_id);
                    }
                }
            }
        }
    }
}

/// Get the name and visibility of an item.
fn get_item_name_and_visibility(item: &Item) -> Option<(String, bool)> {
    match item {
        Item::Function(func) => {
            let name = func.name()?;
            let name_token = name.ident_token()?;
            Some((name_token.text().to_string(), func.visibility().is_some()))
        }
        Item::Struct(struct_def) => {
            let name = struct_def.name()?;
            let name_token = name.ident_token()?;
            Some((
                name_token.text().to_string(),
                struct_def.visibility().is_some(),
            ))
        }
        Item::TypeAlias(type_alias) => {
            let name = type_alias.name()?;
            let name_token = name.ident_token()?;
            Some((
                name_token.text().to_string(),
                type_alias.visibility().is_some(),
            ))
        }
        Item::Extern(extern_block) => {
            // Return first extern fn for simplicity
            for extern_fn in extern_block.extern_fns() {
                if let Some(name) = extern_fn.name()
                    && let Some(name_token) = name.ident_token()
                {
                    return Some((
                        name_token.text().to_string(),
                        extern_fn.visibility().is_some(),
                    ));
                }
            }
            None
        }
        Item::Impl(_) | Item::Use(_) => None,
        Item::Module(module_def) => {
            let name = module_def.name()?;
            let name_token = name.ident_token()?;
            Some((
                name_token.text().to_string(),
                module_def.visibility().is_some(),
            ))
        }
    }
}

/// Phase 2: Collect and resolve imports for all modules.
fn resolve_all_imports(
    package: &Package,
    resolver: &mut Resolver,
    module_id: ModuleId,
    module_scopes: &FxHashMap<ModuleId, ScopeId>,
) {
    // Switch to this module's scope
    let module_scope = module_scopes
        .get(&module_id)
        .expect("module scope should exist");
    resolver.ctx().set_current_scope(*module_scope);
    resolver.ctx().current_module = module_id;

    // Collect use declarations
    let source_map = package.compilation_unit().source_map();
    for (file_id, source_file) in package.compilation_unit().source_files() {
        if let Some(path) = source_map.get_path(file_id) {
            resolver.set_current_file_path(path);
        }
        for item in source_file.items() {
            if let Item::Use(use_decl) = item {
                resolver.collect_item(&Item::Use(use_decl));
            }
        }
    }

    // Resolve imports
    resolver.resolve_imports();

    // Recurse into child modules
    for child_mod in package.modules() {
        let child_id = {
            let tree = resolver.ctx().module_tree.as_ref().unwrap();
            let module = tree.get(module_id);
            let name_spur = tree.interner.get(child_mod.name());
            name_spur
                .and_then(|spur| module.children().get(&spur).copied())
                .expect("child module should exist in tree")
        };
        resolve_all_imports(child_mod, resolver, child_id, module_scopes);
    }
}

/// Phase 3: Resolve all bodies for all modules.
fn resolve_all_bodies(
    package: &Package,
    resolver: &mut Resolver,
    module_id: ModuleId,
    module_scopes: &FxHashMap<ModuleId, ScopeId>,
) {
    // Switch to this module's scope
    let module_scope = module_scopes
        .get(&module_id)
        .expect("module scope should exist");
    resolver.ctx().set_current_scope(*module_scope);
    resolver.ctx().current_module = module_id;

    // Resolve bodies
    let source_map = package.compilation_unit().source_map();
    for (file_id, source_file) in package.compilation_unit().source_files() {
        if let Some(path) = source_map.get_path(file_id) {
            resolver.set_current_file_path(path);
        }
        resolver.resolve_source_file(&source_file);
    }

    // Recurse into child modules
    for child_mod in package.modules() {
        let child_id = {
            let tree = resolver.ctx().module_tree.as_ref().unwrap();
            let module = tree.get(module_id);
            let name_spur = tree.interner.get(child_mod.name());
            name_spur
                .and_then(|spur| module.children().get(&spur).copied())
                .expect("child module should exist in tree")
        };
        resolve_all_bodies(child_mod, resolver, child_id, module_scopes);
    }
}

/// Run type inference on a package (multi-file compilation).
///
/// Takes the resolved package and produces type assignments for all expressions
/// and bindings across all files.
pub fn infer_package(package: &Package, resolve_result: &ResolveResult) -> InferResult {
    let mut engine = InferEngine::new(resolve_result);

    // Phase 1: Collect signatures from ALL modules first
    collect_all_signatures(package, &mut engine);

    // Phase 2: Infer all bodies
    infer_all_bodies(package, &mut engine);

    engine.apply_defaults();
    engine.into_result()
}

/// Phase 1: Collect function signatures, struct info, and type aliases from all modules.
fn collect_all_signatures(package: &Package, engine: &mut InferEngine) {
    for (_file_id, source_file) in package.compilation_unit().source_files() {
        for item in source_file.items() {
            match &item {
                Item::Function(func) => engine.collect_function_signature(func),
                Item::Struct(struct_def) => engine.collect_struct_info(struct_def),
                Item::TypeAlias(type_alias) => engine.collect_type_alias_info(type_alias),
                Item::Impl(impl_block) => engine.collect_impl_signatures(impl_block),
                Item::Extern(extern_block) => engine.collect_extern_signatures(extern_block),
                Item::Module(module_def) => engine.collect_module_signatures(module_def),
                Item::Use(_) => {} // Skip use declarations
            }
        }
    }

    // Recurse into modules
    for child_mod in package.modules() {
        collect_all_signatures(child_mod, engine);
    }
}

/// Phase 2: Infer function bodies from all modules.
fn infer_all_bodies(package: &Package, engine: &mut InferEngine) {
    for (_file_id, source_file) in package.compilation_unit().source_files() {
        for item in source_file.items() {
            match &item {
                Item::Function(func) => engine.infer_function_body(func),
                Item::Impl(impl_block) => engine.infer_impl_bodies(impl_block),
                Item::Module(module_def) => engine.infer_module_bodies(module_def),
                _ => {} // Other items don't have bodies to infer
            }
        }
    }

    // Recurse into modules
    for child_mod in package.modules() {
        infer_all_bodies(child_mod, engine);
    }
}
