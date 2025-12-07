mod architecture;
mod registry_value;
mod registry_key;
mod registry_utils;
mod registry_parser;
mod registry_writer;
mod registry_comparator;
mod registry_patcher;
mod registry_text_diff;
mod registry_dsl;
mod registry_editor;

pub use architecture::Architecture;
pub use registry_value::{
    RegistryValue, RegistryValueData, REG_BINARY, REG_DWORD, REG_EXPAND_SZ, REG_MULTI_SZ, REG_QWORD,
    REG_SZ,
};
pub use registry_key::{KeyNode, RegistryKey, RegistryKeyExt};
pub use registry_utils::*;
pub use registry_parser::{LoadResult, ParseError, RegistryParser};
pub use registry_writer::RegistryWriter;
pub use registry_comparator::{DiffResult, KeyPropertyChange, RegistryChange, RegistryComparator};
pub use registry_patcher::{PatchFailure, PatchOptions, PatchResult, RegistryPatcher};
pub use registry_text_diff::{TextDiffExporter, TextDiffParser};
pub use registry_dsl::{load_registry, modify_registry, registry, RegistryKeyDsl, RegistryResult};
pub use registry_editor::RegistryEditor;
