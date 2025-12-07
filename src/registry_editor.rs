use crate::{
    architecture::Architecture, registry_comparator::RegistryComparator, registry_key::KeyNode,
    registry_parser::{LoadResult, RegistryParser}, registry_writer::RegistryWriter,
};

/// Options for writing/serializing registry data.
#[derive(Debug, Clone)]
pub struct EditorOptions {
    pub relative_base: String,
    pub architecture: Architecture,
}

impl Default for EditorOptions {
    fn default() -> Self {
        Self {
            relative_base: String::new(),
            architecture: Architecture::Unknown,
        }
    }
}

/// Convenience facade mirroring KRegEdit's `RegistryEditor` object.
pub struct RegistryEditor;

impl RegistryEditor {
    /// Load a registry file from disk.
    pub fn load_from_file(filename: &str) -> Result<LoadResult, crate::registry_parser::ParseError> {
        let parser = RegistryParser;
        parser.load_from_file(filename)
    }

    /// Load a registry from an in-memory string.
    pub fn load_from_text(text: &str) -> Result<LoadResult, crate::registry_parser::ParseError> {
        let parser = RegistryParser;
        parser.load_from_text(text)
    }

    /// Write a registry tree to a file with the provided options.
    pub fn write_to_file_with_options(
        key: &KeyNode,
        filename: &str,
        options: EditorOptions,
    ) -> std::io::Result<()> {
        let writer = RegistryWriter {
            relative_base: options.relative_base,
            architecture: options.architecture,
        };
        writer.write_to_file(key, filename)
    }

    /// Convenience write with defaults (no relative base, unknown arch).
    pub fn write_to_file_default(key: &KeyNode, filename: &str) -> std::io::Result<()> {
        Self::write_to_file_with_options(key, filename, EditorOptions::default())
    }

    /// Serialize using options.
    pub fn write_to_string_with_options(key: &KeyNode, options: EditorOptions) -> String {
        let writer = RegistryWriter {
            relative_base: options.relative_base,
            architecture: options.architecture,
        };
        writer.write_to_string(key)
    }

    /// Shorthand with defaults (no relative base, unknown arch).
    pub fn write_to_string_default(key: &KeyNode) -> String {
        Self::write_to_string_with_options(key, EditorOptions::default())
    }

    /// Compare two registries and return the diff.
    pub fn compare_registries(key1: &KeyNode, key2: &KeyNode) -> crate::registry_comparator::DiffResult {
        let comparator = RegistryComparator;
        comparator.compare_registries(key1, key2)
    }
}

