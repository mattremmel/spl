//! Module tree for cross-module name resolution.
//!
//! This module provides the infrastructure for tracking module hierarchy and
//! resolving paths across module boundaries.
//!
//! # Terminology
//!
//! In SPL (per docs/module-system.md):
//! - **Package** = The whole project/compilation unit (like Rust's crate)
//! - **Module** = A directory of source files (like Rust's module)
//!
//! # Module Hierarchy
//!
//! Modules form a tree rooted at the package root. Each module can contain:
//! - Child modules (subdirectories)
//! - Items (functions, structs, type aliases, etc.)
//! - Exports (publicly visible items)
//!
//! # Path Resolution
//!
//! Paths like `module.utils.helpers` are resolved by walking the module tree:
//! 1. Start at the specified anchor (`module`, `super`, `self`, or current module)
//! 2. Follow each segment to a child module
//! 3. Look up the final segment in the target module's exports

use lasso::Spur;
use rustc_hash::FxHashMap;

use super::symbol::DefId;
use crate::package::Package;

/// A unique identifier for each module in the crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModuleId(pub u32);

/// A module in the module tree.
#[derive(Clone, Debug)]
pub struct Module {
    id: ModuleId,
    name: Spur,
    parent: Option<ModuleId>,
    children: FxHashMap<Spur, ModuleId>,
    /// Items defined in this module (name -> `DefId`).
    items: FxHashMap<Spur, DefId>,
    /// Publicly exported items (subset of items or re-exports).
    exports: FxHashMap<Spur, DefId>,
}

impl Module {
    fn new(id: ModuleId, name: Spur, parent: Option<ModuleId>) -> Self {
        Self {
            id,
            name,
            parent,
            children: FxHashMap::default(),
            items: FxHashMap::default(),
            exports: FxHashMap::default(),
        }
    }

    /// Get the module's ID.
    pub fn id(&self) -> ModuleId {
        self.id
    }

    /// Get the module's name.
    pub fn name(&self) -> Spur {
        self.name
    }

    /// Get the parent module ID, if any.
    pub fn parent(&self) -> Option<ModuleId> {
        self.parent
    }

    /// Get the module's exports.
    pub fn exports(&self) -> &FxHashMap<Spur, DefId> {
        &self.exports
    }

    /// Get the module's items (all items, not just public).
    pub fn items(&self) -> &FxHashMap<Spur, DefId> {
        &self.items
    }

    /// Get the module's children.
    pub fn children(&self) -> &FxHashMap<Spur, ModuleId> {
        &self.children
    }
}

/// The module tree for a crate.
///
/// Owns all modules and provides path resolution.
pub struct ModuleTree {
    modules: Vec<Module>,
    /// String interner for module names and item names.
    pub interner: lasso::Rodeo,
}

impl Default for ModuleTree {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleTree {
    /// Create a new module tree with a root module (package root).
    pub fn new() -> Self {
        let mut interner = lasso::Rodeo::default();
        // The root module represents the package (project) root
        let root_name = interner.get_or_intern("module");
        let root = Module::new(ModuleId(0), root_name, None);

        Self {
            modules: vec![root],
            interner,
        }
    }

    /// Get the root module ID.
    pub fn root_id(&self) -> ModuleId {
        ModuleId(0)
    }

    /// Get the root module.
    pub fn root(&self) -> &Module {
        &self.modules[0]
    }

    /// Get a module by ID.
    pub fn get(&self, id: ModuleId) -> &Module {
        debug_assert!(
            (id.0 as usize) < self.modules.len(),
            "precondition: ModuleId {} must be valid (< {})",
            id.0,
            self.modules.len()
        );
        &self.modules[id.0 as usize]
    }

    /// Get a mutable reference to a module by ID.
    pub fn get_mut(&mut self, id: ModuleId) -> &mut Module {
        debug_assert!(
            (id.0 as usize) < self.modules.len(),
            "precondition: ModuleId {} must be valid (< {})",
            id.0,
            self.modules.len()
        );
        &mut self.modules[id.0 as usize]
    }

    /// Add a child module to a parent module.
    pub fn add_child(&mut self, parent: ModuleId, name: &str) -> ModuleId {
        let id = ModuleId(self.modules.len() as u32);
        let name_spur = self.interner.get_or_intern(name);
        let module = Module::new(id, name_spur, Some(parent));
        self.modules.push(module);

        // Register in parent's children
        self.modules[parent.0 as usize]
            .children
            .insert(name_spur, id);

        id
    }

    /// Add an item to a module.
    pub fn add_item(&mut self, module_id: ModuleId, name: &str, def_id: DefId) {
        let name_spur = self.interner.get_or_intern(name);
        self.modules[module_id.0 as usize]
            .items
            .insert(name_spur, def_id);
    }

    /// Add an export to a module.
    pub fn add_export(&mut self, module_id: ModuleId, name: &str, def_id: DefId) {
        let name_spur = self.interner.get_or_intern(name);
        self.modules[module_id.0 as usize]
            .exports
            .insert(name_spur, def_id);
    }

    /// Resolve a child path from a starting module.
    ///
    /// Given a path like `["utils", "helpers"]`, resolves from the starting
    /// module through child modules.
    pub fn resolve_child_path(&self, from: ModuleId, path: &[&str]) -> Option<ModuleId> {
        let mut current = from;

        for segment in path {
            let segment_spur = self.interner.get(segment)?;
            let module = &self.modules[current.0 as usize];
            current = *module.children.get(&segment_spur)?;
        }

        Some(current)
    }

    /// Resolve a path with possible prefix (module, super, self).
    ///
    /// Returns the target module and whether resolution succeeded.
    /// - `module` → jump to package root (root module)
    /// - `super` → jump to parent module (error if at root)
    /// - `self` → stay at current module
    pub fn resolve_path(
        &self,
        from: ModuleId,
        segments: &[&str],
    ) -> Result<ModuleId, PathResolveError> {
        if segments.is_empty() {
            return Ok(from);
        }

        let (start, rest) = match segments[0] {
            "module" => (self.root_id(), &segments[1..]),
            "super" => {
                let module = self.get(from);
                match module.parent() {
                    Some(parent) => (parent, &segments[1..]),
                    None => return Err(PathResolveError::SuperAtRoot),
                }
            }
            "self" => (from, &segments[1..]),
            _ => (from, segments),
        };

        // Resolve remaining segments as child path
        let string_segments: Vec<&str> = rest.to_vec();
        self.resolve_child_path(start, &string_segments)
            .ok_or(PathResolveError::ModuleNotFound)
    }

    /// Intern a string.
    pub fn intern(&mut self, s: &str) -> Spur {
        self.interner.get_or_intern(s)
    }

    /// Resolve an interned string.
    pub fn resolve_str(&self, spur: Spur) -> &str {
        self.interner.resolve(&spur)
    }

    /// Build a `ModuleTree` from a Package hierarchy.
    ///
    /// Creates module nodes for each child module. Items and exports are populated
    /// later during resolution after `DefIds` are assigned.
    pub fn from_package_structure(package: &Package) -> Self {
        let mut tree = Self::new();
        let root_id = tree.root_id();
        Self::build_module_structure(&mut tree, root_id, package);
        tree
    }

    fn build_module_structure(tree: &mut ModuleTree, parent_id: ModuleId, package: &Package) {
        // Recursively create child modules for child modules
        for child_mod in package.modules() {
            let child_id = tree.add_child(parent_id, child_mod.name());
            Self::build_module_structure(tree, child_id, child_mod);
        }
    }
}

/// Errors that can occur during path resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathResolveError {
    /// Cannot use `super` at the package root.
    SuperAtRoot,
    /// Module not found in path.
    ModuleNotFound,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_tree_single_root() {
        let tree = ModuleTree::new();
        let root_name = tree.resolve_str(tree.root().name());
        assert_eq!(root_name, "module");
    }

    #[test]
    fn module_tree_add_child() {
        let mut tree = ModuleTree::new();
        let child = tree.add_child(tree.root_id(), "utils");
        assert_eq!(tree.get(child).parent(), Some(tree.root_id()));
    }

    #[test]
    fn module_tree_path_lookup() {
        let mut tree = ModuleTree::new();
        let utils = tree.add_child(tree.root_id(), "utils");
        tree.add_child(utils, "helpers");

        let found = tree.resolve_child_path(tree.root_id(), &["utils", "helpers"]);
        assert!(found.is_some());
    }

    #[test]
    fn module_exports_tracking() {
        let mut tree = ModuleTree::new();
        tree.add_export(tree.root_id(), "Foo", DefId(1));

        let foo_spur = tree.interner.get("Foo").unwrap();
        assert_eq!(tree.root().exports().get(&foo_spur), Some(&DefId(1)));
    }

    #[test]
    fn module_tree_nested_children() {
        let mut tree = ModuleTree::new();
        let a = tree.add_child(tree.root_id(), "a");
        let b = tree.add_child(a, "b");
        let c = tree.add_child(b, "c");

        let found = tree.resolve_child_path(tree.root_id(), &["a", "b", "c"]);
        assert_eq!(found, Some(c));
    }

    #[test]
    fn module_tree_invalid_path() {
        let tree = ModuleTree::new();
        let found = tree.resolve_child_path(tree.root_id(), &["nonexistent"]);
        assert!(found.is_none());
    }

    #[test]
    fn module_items_tracking() {
        let mut tree = ModuleTree::new();
        tree.add_item(tree.root_id(), "main", DefId(0));
        tree.add_item(tree.root_id(), "helper", DefId(1));

        let main_spur = tree.interner.get("main").unwrap();
        let helper_spur = tree.interner.get("helper").unwrap();

        assert_eq!(tree.root().items().get(&main_spur), Some(&DefId(0)));
        assert_eq!(tree.root().items().get(&helper_spur), Some(&DefId(1)));
    }

    #[test]
    fn resolve_path_module_prefix() {
        let mut tree = ModuleTree::new();
        let child = tree.add_child(tree.root_id(), "child");

        // From child, resolve module path (goes to root)
        let result = tree.resolve_path(child, &["module"]);
        assert_eq!(result, Ok(tree.root_id()));
    }

    #[test]
    fn resolve_path_super_prefix() {
        let mut tree = ModuleTree::new();
        let child = tree.add_child(tree.root_id(), "child");

        let result = tree.resolve_path(child, &["super"]);
        assert_eq!(result, Ok(tree.root_id()));
    }

    #[test]
    fn resolve_path_super_at_root_error() {
        let tree = ModuleTree::new();
        let result = tree.resolve_path(tree.root_id(), &["super"]);
        assert_eq!(result, Err(PathResolveError::SuperAtRoot));
    }

    #[test]
    fn resolve_path_self_prefix() {
        let mut tree = ModuleTree::new();
        let child = tree.add_child(tree.root_id(), "child");

        let result = tree.resolve_path(child, &["self"]);
        assert_eq!(result, Ok(child));
    }

    #[test]
    fn resolve_path_combined() {
        let mut tree = ModuleTree::new();
        let parent = tree.add_child(tree.root_id(), "parent");
        let child = tree.add_child(parent, "child");
        let sibling = tree.add_child(parent, "sibling");

        // From child, resolve super.sibling
        let result = tree.resolve_path(child, &["super", "sibling"]);
        assert_eq!(result, Ok(sibling));
    }

    // ===== Phase 7: ModuleTree Test Coverage Gaps =====

    #[test]
    fn resolve_path_multiple_super() {
        let mut tree = ModuleTree::new();
        let child = tree.add_child(tree.root_id(), "child");
        let grandchild = tree.add_child(child, "grandchild");

        // From grandchild, super goes to child, then child.grandchild error since "super" isn't consumed as a path
        // Actually we need to test chained super which isn't in a single path segment.
        // Let's test the scenario from grandchild: super gets us to child, then we need another super
        // But resolve_path only handles one super at the start. Let me verify the actual behavior.

        // First super at start: goes to parent (child)
        let result = tree.resolve_path(grandchild, &["super"]);
        assert_eq!(result, Ok(child));

        // From child, super goes to root
        let result = tree.resolve_path(child, &["super"]);
        assert_eq!(result, Ok(tree.root_id()));
    }

    #[test]
    fn resolve_path_module_with_segments() {
        let mut tree = ModuleTree::new();
        let child = tree.add_child(tree.root_id(), "child");
        let grandchild = tree.add_child(child, "grandchild");

        // module.child.grandchild from anywhere should work
        let result = tree.resolve_path(tree.root_id(), &["module", "child", "grandchild"]);
        assert_eq!(result, Ok(grandchild));

        // Also works from the grandchild itself
        let result = tree.resolve_path(grandchild, &["module", "child", "grandchild"]);
        assert_eq!(result, Ok(grandchild));
    }

    #[test]
    fn resolve_empty_path_returns_current() {
        let mut tree = ModuleTree::new();
        let child = tree.add_child(tree.root_id(), "child");

        // Empty path should return current module
        let result = tree.resolve_path(child, &[]);
        assert_eq!(result, Ok(child));

        let result = tree.resolve_path(tree.root_id(), &[]);
        assert_eq!(result, Ok(tree.root_id()));
    }

    #[test]
    fn resolve_path_self_with_children() {
        let mut tree = ModuleTree::new();
        let child = tree.add_child(tree.root_id(), "child");
        let grandchild = tree.add_child(child, "grandchild");

        // self.grandchild from child
        let result = tree.resolve_path(child, &["self", "grandchild"]);
        assert_eq!(result, Ok(grandchild));
    }

    #[test]
    fn resolve_path_invalid_child() {
        let tree = ModuleTree::new();

        // Trying to access non-existent child should fail
        let result = tree.resolve_path(tree.root_id(), &["nonexistent"]);
        assert_eq!(result, Err(PathResolveError::ModuleNotFound));
    }
}
