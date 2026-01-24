//! Source map for tracking file origins in multi-file compilation.
//!
//! The [`SourceMap`] provides a mapping from [`FileId`] to file path and content,
//! enabling error messages to reference the correct source file.

use std::path::{Path, PathBuf};

/// Unique identifier for a source file within a compilation unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(pub(crate) u32);

impl FileId {
    /// Returns the raw index of this file ID.
    pub fn index(self) -> u32 {
        self.0
    }
}

/// Internal representation of a source file.
struct SourceFile {
    path: PathBuf,
    content: String,
}

/// Maps file IDs to their paths and contents.
///
/// Used to track the origin of AST nodes for error reporting in multi-file
/// compilation.
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    /// Creates a new empty source map.
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    /// Adds a file to the source map and returns its unique ID.
    pub fn add_file(&mut self, path: impl AsRef<Path>, content: String) -> FileId {
        let id = FileId(self.files.len() as u32);
        self.files.push(SourceFile {
            path: path.as_ref().to_path_buf(),
            content,
        });
        id
    }

    /// Returns the content of a file by ID.
    pub fn get_content(&self, id: FileId) -> Option<&str> {
        self.files.get(id.0 as usize).map(|f| f.content.as_str())
    }

    /// Returns the path of a file by ID.
    pub fn get_path(&self, id: FileId) -> Option<&Path> {
        self.files.get(id.0 as usize).map(|f| f.path.as_path())
    }

    /// Returns the number of files in the source map.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Returns `true` if the source map contains no files.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Returns an iterator over all file IDs.
    pub fn file_ids(&self) -> impl Iterator<Item = FileId> {
        (0..self.files.len() as u32).map(FileId)
    }
}

impl Default for SourceMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_map_add_file_returns_unique_id() {
        let mut map = SourceMap::new();

        let id1 = map.add_file("file1.spl", "fn foo() {}".to_string());
        let id2 = map.add_file("file2.spl", "fn bar() {}".to_string());

        assert_ne!(id1, id2);
        assert_eq!(id1.index(), 0);
        assert_eq!(id2.index(), 1);
    }

    #[test]
    fn source_map_get_file_content() {
        let mut map = SourceMap::new();
        let content = "fn main() { 42 }";
        let id = map.add_file("main.spl", content.to_string());

        assert_eq!(map.get_content(id), Some(content));
    }

    #[test]
    fn source_map_get_file_path() {
        let mut map = SourceMap::new();
        let id = map.add_file("src/main.spl", "fn main() {}".to_string());

        assert_eq!(map.get_path(id), Some(Path::new("src/main.spl")));
    }

    #[test]
    fn source_map_unknown_id_returns_none() {
        let map = SourceMap::new();
        let unknown_id = FileId(999);

        assert_eq!(map.get_content(unknown_id), None);
        assert_eq!(map.get_path(unknown_id), None);
    }

    #[test]
    fn source_map_len_and_is_empty() {
        let mut map = SourceMap::new();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);

        map.add_file("file.spl", String::new());
        assert!(!map.is_empty());
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn source_map_file_ids_iterator() {
        let mut map = SourceMap::new();
        map.add_file("a.spl", String::new());
        map.add_file("b.spl", String::new());
        map.add_file("c.spl", String::new());

        let ids: Vec<_> = map.file_ids().collect();
        assert_eq!(ids.len(), 3);
        assert_eq!(ids[0].index(), 0);
        assert_eq!(ids[1].index(), 1);
        assert_eq!(ids[2].index(), 2);
    }

    #[test]
    fn source_map_default() {
        let map = SourceMap::default();
        assert!(map.is_empty());
    }
}
