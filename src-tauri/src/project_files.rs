mod error;
mod model;
mod mutations;
mod paths;
mod read;
mod scan;

#[cfg(test)]
mod tests;

pub use error::ProjectFileError;
pub use model::{
    LocationXmlFileScan, ProjectFileContent, ProjectFileEntry, ProjectFileKind, ProjectFileScan,
    ProjectFolderEntry, ProjectPathMutationResult,
};
pub use mutations::{
    create_project_file, create_project_folder, delete_project_path, rename_project_path,
};
#[allow(unused_imports)]
pub(crate) use paths::resolve_project_root;
pub use read::{read_xml_file, validate_and_resolve, validate_and_resolve_location};
pub(crate) use scan::scan_indexable_def_xml_files;
pub use scan::{scan_all_project_files, scan_xml_files};
