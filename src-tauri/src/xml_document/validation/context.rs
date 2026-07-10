use crate::def_index::DefIndex;
use crate::schema_pack::SchemaCatalog;

pub struct ValidationContext<'a> {
    pub catalog: &'a SchemaCatalog,
    pub def_index: &'a DefIndex,
}
