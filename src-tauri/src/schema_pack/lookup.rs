use super::model::{
    DefTypeSchema, FieldSchema, ObjectTypeSchema, PatchOperationMetadata, SchemaCatalog,
};
use std::collections::HashSet;

/// Look up patch operation metadata by `className`. Covers both built-in operations (shipped as
/// metadata) and custom/mod-defined operations.
pub fn lookup_patch_operation_metadata<'a>(
    catalog: &'a SchemaCatalog,
    class_name: &str,
) -> Option<&'a PatchOperationMetadata> {
    catalog.patch_operations.get(class_name)
}

/// Return all def type names that are equal to `base` or inherit from it, transitively.
///
/// For example, given base = "LayoutDef", returns {"LayoutDef", "StructureLayoutDef",
/// "ComplexLayoutDef", ...} so that a reference field typed as LayoutDef can resolve
/// against any concrete subtype that is actually stored in the index.
pub fn collect_def_subtypes(catalog: &SchemaCatalog, base: &str) -> Vec<String> {
    let mut types: Vec<String> = vec![base.to_string()];
    // Expand transitively until stable.
    loop {
        let prev_len = types.len();
        for (name, schema) in &catalog.def_types {
            if !types.iter().any(|t| t == name)
                && schema
                    .inherits
                    .iter()
                    .any(|p| types.iter().any(|t| t == p.as_str()))
            {
                types.push(name.clone());
            }
        }
        if types.len() == prev_len {
            break;
        }
    }
    types
}

pub fn lookup_def_type<'a>(
    catalog: &'a SchemaCatalog,
    def_type: &str,
) -> Option<&'a DefTypeSchema> {
    catalog.def_types.get(def_type)
}

/// Look up a field by name on a def type, walking the inherits chain.
///
/// Inheritance is resolved shallowly from the merged catalog (each parent's fields
/// are already in the catalog). The search order is: own fields first, then each
/// entry in `inherits` left-to-right, depth-first.
///
/// A cycle guard prevents infinite loops from malformed schemas.
pub fn lookup_field<'a>(
    catalog: &'a SchemaCatalog,
    def_type: &str,
    field_name: &str,
) -> Option<&'a FieldSchema> {
    let mut visited: HashSet<String> = HashSet::new();
    lookup_field_recursive(catalog, def_type, field_name, &mut visited)
}

#[allow(dead_code)]
pub fn lookup_object_type<'a>(
    catalog: &'a SchemaCatalog,
    object_type: &str,
) -> Option<&'a ObjectTypeSchema> {
    catalog.object_types.get(object_type)
}

#[allow(dead_code)]
pub fn lookup_object_field<'a>(
    catalog: &'a SchemaCatalog,
    object_type: &str,
    field_name: &str,
) -> Option<&'a FieldSchema> {
    catalog
        .object_types
        .get(object_type)
        .and_then(|o| o.fields.get(field_name))
}

/// Look up a field by name on an object type, walking the `inherits` chain.
#[allow(dead_code)]
pub fn lookup_object_field_inherited<'a>(
    catalog: &'a SchemaCatalog,
    object_type: &str,
    field_name: &str,
) -> Option<&'a FieldSchema> {
    let mut visited: HashSet<String> = HashSet::new();
    lookup_object_field_recursive(catalog, object_type, field_name, &mut visited)
}

/// Collect all field names accessible on an object type, including inherited fields.
pub fn collect_all_object_inherited_fields(
    catalog: &SchemaCatalog,
    object_type: &str,
) -> HashSet<String> {
    let mut fields = HashSet::new();
    let mut visited = HashSet::new();
    collect_object_fields_recursive(catalog, object_type, &mut fields, &mut visited);
    fields
}

fn lookup_object_field_recursive<'a>(
    catalog: &'a SchemaCatalog,
    object_type: &str,
    field_name: &str,
    visited: &mut HashSet<String>,
) -> Option<&'a FieldSchema> {
    if visited.contains(object_type) {
        return None;
    }
    visited.insert(object_type.to_string());

    let schema = catalog.object_types.get(object_type)?;

    if let Some(field) = schema.fields.get(field_name) {
        return Some(field);
    }
    for field in schema.fields.values() {
        if field.xml_aliases.iter().any(|a| a == field_name) {
            return Some(field);
        }
    }

    for parent in &schema.inherits {
        if let Some(field) =
            lookup_object_field_recursive(catalog, parent.as_str(), field_name, visited)
        {
            return Some(field);
        }
    }

    None
}

fn collect_object_fields_recursive(
    catalog: &SchemaCatalog,
    object_type: &str,
    fields: &mut HashSet<String>,
    visited: &mut HashSet<String>,
) {
    if !visited.insert(object_type.to_string()) {
        return;
    }
    if let Some(schema) = catalog.object_types.get(object_type) {
        for parent in &schema.inherits {
            collect_object_fields_recursive(catalog, parent, fields, visited);
        }
        for (field_name, field_schema) in &schema.fields {
            fields.insert(field_name.clone());
            for alias in &field_schema.xml_aliases {
                fields.insert(alias.clone());
            }
        }
    }
}

fn lookup_field_recursive<'a>(
    catalog: &'a SchemaCatalog,
    def_type: &str,
    field_name: &str,
    visited: &mut HashSet<String>,
) -> Option<&'a FieldSchema> {
    if visited.contains(def_type) {
        return None;
    }
    visited.insert(def_type.to_string());

    let schema = catalog.def_types.get(def_type)?;

    if let Some(field) = schema.fields.get(field_name) {
        return Some(field);
    }
    for field in schema.fields.values() {
        if field.xml_aliases.iter().any(|a| a == field_name) {
            return Some(field);
        }
    }

    for parent in &schema.inherits {
        if let Some(field) = lookup_field_recursive(catalog, parent.as_str(), field_name, visited) {
            return Some(field);
        }
    }

    None
}
