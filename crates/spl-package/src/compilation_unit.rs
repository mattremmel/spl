//! Multi-file AST aggregation.
//!
//! A [`CompilationUnit`] combines multiple parsed source files into a single
//! unit, tracking the origin of each item for error reporting.

use super::{FileId, SourceMap};
use rowan::ast::AstNode;
use spl_ast::{Item, SourceFile as AstSourceFile};
use spl_parser::{Parse, ParseError, parse};

/// Aggregated AST from multiple source files.
pub struct CompilationUnit {
    source_map: SourceMap,
    parsed_files: Vec<(FileId, Parse)>,
}

impl CompilationUnit {
    /// Create a compilation unit by parsing files from a source map.
    ///
    /// Parses each file identified by `file_ids` using the content from `source_map`.
    pub fn parse(source_map: SourceMap, file_ids: &[FileId]) -> Self {
        let parsed_files = file_ids
            .iter()
            .filter_map(|&id| {
                let content = source_map.get_content(id)?;
                Some((id, parse(content)))
            })
            .collect();

        Self {
            source_map,
            parsed_files,
        }
    }

    /// Returns the source map.
    pub fn source_map(&self) -> &SourceMap {
        &self.source_map
    }

    /// Returns an iterator over parsed source files.
    ///
    /// Each item is a tuple of (`FileId`, `SourceFile` AST node).
    pub fn source_files(&self) -> impl Iterator<Item = (FileId, AstSourceFile)> + '_ {
        self.parsed_files.iter().filter_map(|(id, parse)| {
            let sf = AstSourceFile::cast(parse.syntax())?;
            Some((*id, sf))
        })
    }

    /// Returns an iterator over all items from all files.
    ///
    /// Each item is a tuple of (`FileId`, Item).
    pub fn items(&self) -> impl Iterator<Item = (FileId, Item)> + '_ {
        self.source_files().flat_map(|(id, sf)| {
            // Collect items to avoid lifetime issues with the iterator
            sf.items()
                .collect::<Vec<_>>()
                .into_iter()
                .map(move |item| (id, item))
        })
    }

    /// Returns an iterator over all parse errors from all files.
    ///
    /// Each error is a tuple of (`FileId`, `ParseError`).
    pub fn errors(&self) -> impl Iterator<Item = (FileId, &ParseError)> + '_ {
        self.parsed_files
            .iter()
            .flat_map(|(id, parse)| parse.errors().iter().map(move |err| (*id, err)))
    }

    /// Returns `true` if any file has parse errors.
    pub fn has_errors(&self) -> bool {
        self.parsed_files.iter().any(|(_, parse)| !parse.ok())
    }

    /// Returns the number of files in this compilation unit.
    pub fn file_count(&self) -> usize {
        self.parsed_files.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compilation_unit_from_single_file() {
        let mut source_map = SourceMap::new();
        let id = source_map.add_file("main.spl", "fn main() {}".to_string());

        let unit = CompilationUnit::parse(source_map, &[id]);

        assert_eq!(unit.file_count(), 1);
        assert!(!unit.has_errors());
        assert_eq!(unit.items().count(), 1);
    }

    #[test]
    fn compilation_unit_from_multiple_files() {
        let mut source_map = SourceMap::new();
        let id1 = source_map.add_file("main.spl", "fn main() {}".to_string());
        let id2 = source_map.add_file("lib.spl", "fn helper() {}".to_string());
        let id3 = source_map.add_file("utils.spl", "fn util() {}".to_string());

        let unit = CompilationUnit::parse(source_map, &[id1, id2, id3]);

        assert_eq!(unit.file_count(), 3);
        assert!(!unit.has_errors());
    }

    #[test]
    fn compilation_unit_collects_all_items() {
        let mut source_map = SourceMap::new();
        let id1 = source_map.add_file("a.spl", "fn a() {} fn b() {}".to_string());
        let id2 = source_map.add_file("b.spl", "fn c() {}".to_string());

        let unit = CompilationUnit::parse(source_map, &[id1, id2]);

        let items: Vec<_> = unit.items().collect();
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn compilation_unit_tracks_item_origin() {
        let mut source_map = SourceMap::new();
        let id1 = source_map.add_file("first.spl", "fn from_first() {}".to_string());
        let id2 = source_map.add_file("second.spl", "fn from_second() {}".to_string());

        let unit = CompilationUnit::parse(source_map, &[id1, id2]);

        let items: Vec<_> = unit.items().collect();
        assert_eq!(items.len(), 2);

        // Check that each item tracks its source file
        assert_eq!(items[0].0, id1);
        assert_eq!(items[1].0, id2);
    }

    #[test]
    fn compilation_unit_collects_all_errors() {
        let mut source_map = SourceMap::new();
        let id1 = source_map.add_file("good.spl", "fn good() {}".to_string());
        // Use recoverable syntax error (invalid tokens between items)
        let id2 = source_map.add_file("bad.spl", "@@@ fn bad() {}".to_string());

        let unit = CompilationUnit::parse(source_map, &[id1, id2]);

        assert!(unit.has_errors());

        let errors: Vec<_> = unit.errors().collect();
        assert!(!errors.is_empty());

        // Error should be from the bad file
        assert!(errors.iter().any(|(file_id, _)| *file_id == id2));
    }

    #[test]
    fn compilation_unit_empty() {
        let source_map = SourceMap::new();
        let unit = CompilationUnit::parse(source_map, &[]);

        assert_eq!(unit.file_count(), 0);
        assert!(!unit.has_errors());
        assert_eq!(unit.items().count(), 0);
    }

    #[test]
    fn compilation_unit_source_files_iterator() {
        let mut source_map = SourceMap::new();
        let id1 = source_map.add_file("a.spl", "fn a() {}".to_string());
        let id2 = source_map.add_file("b.spl", "fn b() {}".to_string());

        let unit = CompilationUnit::parse(source_map, &[id1, id2]);

        let source_files: Vec<_> = unit.source_files().collect();
        assert_eq!(source_files.len(), 2);
    }

    #[test]
    fn compilation_unit_source_map_access() {
        let mut source_map = SourceMap::new();
        let id = source_map.add_file("test.spl", "fn test() {}".to_string());

        let unit = CompilationUnit::parse(source_map, &[id]);

        // Can access source map through unit
        assert_eq!(unit.source_map().get_content(id), Some("fn test() {}"));
    }

    #[test]
    fn compilation_unit_skips_invalid_file_ids() {
        let mut source_map = SourceMap::new();
        let id1 = source_map.add_file("valid.spl", "fn valid() {}".to_string());
        let invalid_id = FileId(999); // Not in source map

        let unit = CompilationUnit::parse(source_map, &[id1, invalid_id]);

        // Should only have one file (the valid one)
        assert_eq!(unit.file_count(), 1);
    }
}
