use crate::def_index::DefIndex;
use crate::schema_pack::SchemaCatalog;

pub struct ValidationContext<'a> {
    pub catalog: &'a SchemaCatalog,
    pub def_index: &'a DefIndex,
    /// Set only when the document being validated is itself a read-only source document that
    /// was indexed under this location ID. Lets `validate_def_identity` recognize the document's
    /// own indexed Defs as their own identity rather than a source-duplicate conflict, without
    /// changing `DefIndex::find_source_duplicates`'s general "all matching indexed source Defs"
    /// contract. Unset for project documents, save validation, whole-project validation, and unit
    /// helpers that don't exercise source-document self-match behavior.
    pub source_document_location_id: Option<&'a str>,
}
