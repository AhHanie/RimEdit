use super::model::{
    DefTypeSchema, FieldSchema, ObjectTypeSchema, PatchOperationMetadata, SchemaCatalog,
};
use std::collections::BTreeMap;
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

/// Look up a field by name or XML alias on an object type, walking the `inherits` chain, and
/// return the *canonical* field name alongside its schema. Used by patch XPath completion
/// (`patches::xpath`), which must report which canonical field a typed alias resolves to for
/// `XPathResolvedField.fieldName` -- `lookup_object_field_inherited` above only returns the
/// `FieldSchema`, not the name that resolved it. Search order and cycle guard mirror
/// `lookup_object_field_recursive`.
pub fn lookup_object_field_with_alias<'a>(
    catalog: &'a SchemaCatalog,
    object_type: &str,
    field_name: &str,
) -> Option<(&'a str, &'a FieldSchema)> {
    let mut visited: HashSet<String> = HashSet::new();
    lookup_object_field_with_alias_recursive(catalog, object_type, field_name, &mut visited)
}

fn lookup_object_field_with_alias_recursive<'a>(
    catalog: &'a SchemaCatalog,
    object_type: &str,
    field_name: &str,
    visited: &mut HashSet<String>,
) -> Option<(&'a str, &'a FieldSchema)> {
    if visited.contains(object_type) {
        return None;
    }
    visited.insert(object_type.to_string());

    let schema = catalog.object_types.get(object_type)?;

    if let Some((canonical, field)) = schema.fields.get_key_value(field_name) {
        return Some((canonical.as_str(), field));
    }
    for (canonical, field) in &schema.fields {
        if field.xml_aliases.iter().any(|a| a == field_name) {
            return Some((canonical.as_str(), field));
        }
    }

    for parent in &schema.inherits {
        if let Some(found) =
            lookup_object_field_with_alias_recursive(catalog, parent.as_str(), field_name, visited)
        {
            return Some(found);
        }
    }

    None
}

/// Collect every field reachable on an object type through its `inherits` chain, each canonical
/// name appearing once (own fields win over a same-named inherited field, mirroring
/// `lookup_object_field_with_alias`'s own-fields-first search order). Used to build patch XPath
/// autocomplete suggestions for an object-typed path segment (`patches::xpath`), which -- unlike
/// Def-type field completion -- has no "direct fields only" restriction: object-type inheritance
/// is ordinary C# inheritance already resolved on the object instance, not RimWorld's
/// before-patches Def XML inheritance, so every inherited field is a legitimate patch target.
pub fn collect_object_fields_ordered<'a>(
    catalog: &'a SchemaCatalog,
    object_type: &str,
) -> Vec<(&'a str, &'a FieldSchema)> {
    let mut out: Vec<(&str, &FieldSchema)> = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut visited: HashSet<String> = HashSet::new();
    collect_object_fields_ordered_recursive(
        catalog,
        object_type,
        &mut out,
        &mut seen,
        &mut visited,
    );
    out
}

fn collect_object_fields_ordered_recursive<'a>(
    catalog: &'a SchemaCatalog,
    object_type: &str,
    out: &mut Vec<(&'a str, &'a FieldSchema)>,
    seen: &mut HashSet<&'a str>,
    visited: &mut HashSet<String>,
) {
    if !visited.insert(object_type.to_string()) {
        return;
    }
    let Some(schema) = catalog.object_types.get(object_type) else {
        return;
    };
    for (name, field) in &schema.fields {
        if seen.insert(name.as_str()) {
            out.push((name.as_str(), field));
        }
    }
    for parent in &schema.inherits {
        collect_object_fields_ordered_recursive(catalog, parent, out, seen, visited);
    }
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

/// Ancestor-first, keep-first-occurrence collection of a Def type's effective top-level field
/// names, mirroring the frontend form renderer's `getAllSchemaFields`
/// (`src/features/xml-editor/lib/formDescriptors.ts`) rather than this module's own
/// `lookup_field`, which searches own fields first. Plan.md section 5 explicitly flags this
/// discrepancy and requires Form View field-reference validation (issue 03) to match whichever
/// field definition the form actually renders when a Def type and one of its ancestors both
/// declare a field with the same name -- that is the ancestor's definition, kept from the first
/// (parent-first) occurrence, with the child's own same-named redeclaration ignored for identity
/// purposes (though its presence still keeps the name itself known).
///
/// Returns `(name, field)` pairs in the same order `getAllSchemaFields` would build them, so a
/// caller that needs the resolved `FieldSchema` (not just the name) for a duplicate-named field
/// gets the ancestor's version, not `lookup_field`'s own-first version. Most callers only need the
/// name for a known/unknown-field-id membership check; use `.iter().map(|(n, _)| n)` for that.
///
/// A cycle in `inherits` is guarded the same way `lookup_field_recursive` guards field lookup.
// Only exercised from `schema_pack::tests` today; a public entry point for future consumers
// (e.g. issue 05+ frontend-facing commands), matching `lookup_object_type`/`lookup_object_field`'s
// existing `#[allow(dead_code)]` convention below for the same reason.
#[allow(dead_code)]
pub fn collect_effective_top_level_def_fields(
    catalog: &SchemaCatalog,
    def_type: &str,
) -> Vec<(String, FieldSchema)> {
    collect_effective_top_level_def_fields_from_map(def_type, &catalog.def_types)
}

/// Same as `collect_effective_top_level_def_fields`, but operating directly on a
/// `BTreeMap<String, DefTypeSchema>` under construction -- used by `merge::resolve_all_form_views`
/// before a full `SchemaCatalog` exists (mirrors why `merge::collect_all_inherited_fields` takes
/// the same shape of parameter for its own post-merge diagnostics).
pub(crate) fn collect_effective_top_level_def_fields_from_map(
    def_type: &str,
    def_types: &BTreeMap<String, DefTypeSchema>,
) -> Vec<(String, FieldSchema)> {
    let mut ordered: Vec<(String, FieldSchema)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut visited: HashSet<String> = HashSet::new();
    collect_effective_fields_recursive(def_type, def_types, &mut ordered, &mut seen, &mut visited);
    ordered
}

fn collect_effective_fields_recursive(
    def_type: &str,
    def_types: &BTreeMap<String, DefTypeSchema>,
    ordered: &mut Vec<(String, FieldSchema)>,
    seen: &mut HashSet<String>,
    visited: &mut HashSet<String>,
) {
    if !visited.insert(def_type.to_string()) {
        return;
    }
    let Some(schema) = def_types.get(def_type) else {
        return;
    };
    // Ancestors first so an inherited base field's definition precedes (and, on a duplicate
    // name, wins over) the concrete type's own redeclaration -- matching
    // `formDescriptors.ts`'s `getAllSchemaFields`.
    for parent in &schema.inherits {
        collect_effective_fields_recursive(parent, def_types, ordered, seen, visited);
    }
    for name in ordered_own_field_names(schema) {
        if seen.insert(name.clone()) {
            if let Some(field) = schema.fields.get(&name) {
                ordered.push((name, field.clone()));
            }
        }
    }
}

/// Order a Def type's own field names the same way `getOrderedSchemaFields` does on the frontend:
/// `fieldOrder` entries that resolve to a known field, in that order, followed by any remaining
/// known fields not mentioned in `fieldOrder`. `DefTypeSchema.field_order` may itself contain
/// stray entries that don't resolve to a known field (see `schema_pack_field_order_unknown`), so
/// this filters those out rather than assuming `field_order` is already a clean field-name list.
fn ordered_own_field_names(schema: &DefTypeSchema) -> Vec<String> {
    let mut ordered: Vec<String> = schema
        .field_order
        .iter()
        .filter(|n| schema.fields.contains_key(*n))
        .cloned()
        .collect();
    for name in schema.fields.keys() {
        if !schema.field_order.contains(name) {
            ordered.push(name.clone());
        }
    }
    ordered
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
