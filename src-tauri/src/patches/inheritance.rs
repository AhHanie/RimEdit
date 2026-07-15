//! Port of RimWorld's `Verse.XmlInheritance` (see
//! `C:\Me\Games\Rimworld\Rimworld Source Code\Verse\XmlInheritance.cs`), adapted to run once over
//! an already-patched combined `<Defs>` document (`patches::dom`) rather than incrementally as
//! mods load. Preview always has the full combined, post-patch document up front, so this module
//! only needs `XmlInheritance`'s `Resolve()` step, not its incremental `TryRegister` step.
//!
//! Kept isolated from `patches::apply` and fixture-heavy per
//! `docs/patches-editor/07-preview-engine.md`'s "XML Inheritance Workstream" -- inheritance
//! parity is the largest fidelity risk in the whole preview engine.

use std::collections::HashMap;

use sxd_document::dom::{ChildOfElement, Document, Element, ParentOfChild};

use super::dom::{clone_child_of_element, clone_element, first_child_element_named};

/// Field names carrying RimWorld's `[XmlInheritanceAllowDuplicateNodes]` attribute in the
/// decompiled source (`Verse.Def.descriptionHyperlinks`, `RimWorld.ThoughtDef.nullifyingTraitDegrees`,
/// `Verse.PawnKindDef.{forcedTraits,disallowedTraitsWithDegree}`,
/// `RimWorld.MemeDef.{agreeableTraits,disagreeableTraits}`). Children of these fields merge like
/// `<li>` list items (appended, never matched-by-name) even though their own element name isn't
/// literally `li`. This is a small, fixed list baked into the game's C# reflection metadata that
/// RimEdit cannot discover generically without running that C# -- hardcoded here for parity.
const ALLOW_DUPLICATE_NODES_FIELD_NAMES: &[&str] = &[
    "descriptionHyperlinks",
    "nullifyingTraitDegrees",
    "forcedTraits",
    "disallowedTraitsWithDegree",
    "agreeableTraits",
    "disagreeableTraits",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InheritanceDiagnosticSeverity {
    Error,
    Warning,
}

// Not `Eq`: `args` can carry a `DiagnosticArgValue::Float`, and `f64` has no `Eq` impl.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InheritanceDiagnostic {
    pub severity: InheritanceDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub def_type: Option<String>,
    pub def_name: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "crate::diagnostics::DiagnosticArgs::is_empty"
    )]
    pub args: crate::diagnostics::DiagnosticArgs,
}

impl InheritanceDiagnostic {
    fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        def_type: impl Into<String>,
        def_name: Option<String>,
    ) -> Self {
        Self {
            severity: InheritanceDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            def_type: Some(def_type.into()),
            def_name,
            args: crate::diagnostics::DiagnosticArgs::new(),
        }
    }

    /// Attaches typed args for `code`. Additive on top of the still-English `message`.
    fn with_args(mut self, args: crate::diagnostics::DiagnosticArgs) -> Self {
        self.args.extend(args);
        self
    }
}

/// Maps every top-level `<Defs>` child that declared a `ParentName` to its resolved (parent-chain
/// merged) element. A top-level child *without* `ParentName` is intentionally absent here --
/// mirroring `XmlInheritance.GetResolvedNodeFor` (only `ParentName`-bearing nodes are ever
/// substituted), callers should use the original, already-patched element unchanged for those.
pub struct InheritanceResolution<'d> {
    resolved: HashMap<Element<'d>, Element<'d>>,
    pub diagnostics: Vec<InheritanceDiagnostic>,
}

impl<'d> InheritanceResolution<'d> {
    /// The final element to use for `original` (a top-level `<Defs>` child) after inheritance:
    /// its resolved clone if it had a `ParentName`, otherwise `original` itself unchanged.
    pub fn resolve(&self, original: Element<'d>) -> Element<'d> {
        self.resolved.get(&original).copied().unwrap_or(original)
    }
}

struct RegisteredNode<'d> {
    xml: Element<'d>,
    def_type: String,
    def_name: Option<String>,
    parent_name: Option<String>,
    parent: Option<usize>,
    children: Vec<usize>,
    resolved: Option<Element<'d>>,
}

fn def_name_of(el: Element<'_>) -> Option<String> {
    first_child_element_named(el, "defName").and_then(super::dom::element_text)
}

/// Resolves XML inheritance over `top_level_defs` (the already-patched, top-level children of
/// the combined `<Defs>` document). Registration (`XmlInheritance.TryRegister`) only considers
/// elements carrying `Name` and/or `ParentName`, so callers do not need to pre-filter.
///
/// Known simplification vs. the decompiled source: RimWorld disambiguates multiple nodes sharing
/// the same `Name` by which mod registered them and mod load order (`GetBestParentFor`). RimEdit's
/// combined document does not carry a per-node "owning mod" tag through patching, so ties are
/// broken by first-registered (i.e. combined-document/load order) instead -- the common case of a
/// single definition per `Name` is unaffected; only same-`Name` parents redefined by multiple mods
/// resolve differently than in a real RimWorld run. Also not modeled: per-node `MayRequire` mod
/// gating on `Name`/`ParentName` registration (RimEdit registers unconditionally, matching the
/// Plan's existing decision to ignore `LoadFolders.xml` mod conditions for the same reason).
pub fn resolve_inheritance<'d>(
    document: Document<'d>,
    top_level_defs: &[Element<'d>],
) -> InheritanceResolution<'d> {
    let mut nodes: Vec<RegisteredNode<'d>> = Vec::new();
    let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
    let mut diagnostics: Vec<InheritanceDiagnostic> = Vec::new();

    for &el in top_level_defs {
        let name_attr = el.attribute_value("Name").map(|s| s.to_string());
        let parent_name = el.attribute_value("ParentName").map(|s| s.to_string());
        if name_attr.is_none() && parent_name.is_none() {
            continue;
        }
        let idx = nodes.len();
        nodes.push(RegisteredNode {
            xml: el,
            def_type: el.name().local_part().to_string(),
            def_name: def_name_of(el),
            parent_name,
            parent: None,
            children: Vec::new(),
            resolved: None,
        });
        if let Some(name) = name_attr {
            by_name.entry(name).or_default().push(idx);
        }
    }

    // ResolveParentsAndChildNodesLinks: link each ParentName-bearing node to its best-matching
    // parent by registration order (see simplification note above).
    for node in &mut nodes {
        let Some(parent_name) = node.parent_name.clone() else {
            continue;
        };
        match by_name.get(&parent_name).and_then(|c| c.first().copied()) {
            Some(parent_idx) => node.parent = Some(parent_idx),
            None => diagnostics.push(
                InheritanceDiagnostic::error(
                    "inheritance_missing_parent",
                    format!(
                        "Could not find parent node named \"{}\" for {} (def falls back to its own, unmerged values)",
                        parent_name, node.def_type
                    ),
                    node.def_type.clone(),
                    node.def_name.clone(),
                )
                .with_args(crate::diagnostics::diagnostic_args([(
                    "parentName",
                    parent_name.into(),
                )])),
            ),
        }
    }
    // Deliberately index-based (not `for node in &nodes`): each iteration writes into a
    // *different* node's `children` (`nodes[parent_idx]`) than the one it reads (`nodes[i]`),
    // which an iterator over `nodes` itself cannot express without aliasing `nodes` twice.
    #[allow(clippy::needless_range_loop)]
    for i in 0..nodes.len() {
        if let Some(parent_idx) = nodes[i].parent {
            nodes[parent_idx].children.push(i);
        }
    }

    // ResolveXmlNodes: process every node reachable from a root (parent == None) top-down, so a
    // parent is always resolved before its children merge into it. Nodes left unreached (pure
    // cycles, unreachable from any root) are flagged as cyclic afterward.
    let roots: Vec<usize> = (0..nodes.len())
        .filter(|&i| nodes[i].parent.is_none())
        .collect();
    for root in roots {
        resolve_recursively(document, root, &mut nodes, &mut diagnostics);
    }
    for node in &nodes {
        if node.resolved.is_none() {
            diagnostics.push(InheritanceDiagnostic::error(
                "inheritance_cycle",
                format!(
                    "Cyclic inheritance hierarchy detected for {} (def falls back to its own, unmerged values)",
                    node.def_type
                ),
                node.def_type.clone(),
                node.def_name.clone(),
            )
            .with_args(crate::diagnostics::diagnostic_args([(
                "defType",
                node.def_type.as_str().into(),
            )])));
        }
    }

    // `Element`'s `Hash`/`Eq` are identity-based (the underlying arena pointer), not
    // content-based, so mutating a node's attributes/children after insertion never changes
    // where it hashes -- safe despite `sxd_document::Storage`'s interior mutability underneath.
    #[allow(clippy::mutable_key_type)]
    let mut resolved = HashMap::new();
    for node in &nodes {
        if node.parent_name.is_some() {
            if let Some(resolved_el) = node.resolved {
                resolved.insert(node.xml, resolved_el);
            }
            // Left unmapped on a cycle: `InheritanceResolution::resolve` falls back to the
            // node's own (already-patched) XML, matching `GetResolvedNodeFor`'s behavior on the
            // "resolvedXmlNode == null" defensive path.
        }
    }

    InheritanceResolution {
        resolved,
        diagnostics,
    }
}

fn resolve_recursively<'d>(
    document: Document<'d>,
    idx: usize,
    nodes: &mut [RegisteredNode<'d>],
    diagnostics: &mut Vec<InheritanceDiagnostic>,
) {
    if nodes[idx].resolved.is_some() {
        diagnostics.push(
            InheritanceDiagnostic::error(
                "inheritance_cycle",
                format!(
                    "Cyclic inheritance hierarchy detected for {}",
                    nodes[idx].def_type
                ),
                nodes[idx].def_type.clone(),
                nodes[idx].def_name.clone(),
            )
            .with_args(crate::diagnostics::diagnostic_args([(
                "defType",
                nodes[idx].def_type.as_str().into(),
            )])),
        );
        return;
    }

    let parent_idx = nodes[idx].parent;
    let xml = nodes[idx].xml;
    let resolved = match parent_idx {
        None => xml,
        Some(pidx) => match nodes[pidx].resolved {
            Some(parent_resolved) => {
                let cloned = clone_element(document, parent_resolved);
                merge_child_into(document, xml, cloned);
                cloned
            }
            // Defensive only: traversal order guarantees the parent resolves first.
            None => xml,
        },
    };
    nodes[idx].resolved = Some(resolved);

    let children = nodes[idx].children.clone();
    for child_idx in children {
        resolve_recursively(document, child_idx, nodes, diagnostics);
    }
}

/// Port of `XmlInheritance.RecursiveNodeCopyOverwriteElements`. `current` starts as a deep clone
/// of the (already-resolved) parent; this merges `child`'s own data on top of it, in place.
fn merge_child_into<'d>(document: Document<'d>, child: Element<'d>, current: Element<'d>) {
    if let Some(inherit) = child.attribute_value("Inherit") {
        if inherit.eq_ignore_ascii_case("false") {
            current.clear_children();
            let cloned_children: Vec<ChildOfElement<'d>> = child
                .children()
                .into_iter()
                .map(|c| clone_child_of_element(document, c))
                .collect();
            current.append_children(cloned_children);
            // Unlike the non-`Inherit="False"` path below, RimWorld does not clear `current`'s
            // existing (inherited) attributes first here -- only `child`'s own attributes are
            // applied on top, so a parent attribute `child` doesn't redeclare survives.
            for attr in child.attributes() {
                if !attr.name().local_part().eq_ignore_ascii_case("Inherit") {
                    current.set_attribute_value(attr.name(), attr.value());
                }
            }
            return;
        }
    }

    // Attributes are replaced wholesale by `child`'s own (RimWorld does not merge parent+child
    // attribute sets at a given recursion level -- see `docs/patches-editor/07-preview-engine.md`).
    for attr in current.attributes() {
        current.remove_attribute(attr.name());
    }
    for attr in child.attributes() {
        current.set_attribute_value(attr.name(), attr.value());
    }

    let mut has_element_child = false;
    let mut last_text: Option<sxd_document::dom::Text<'d>> = None;
    for c in child.children() {
        match c {
            ChildOfElement::Text(t) => last_text = Some(t),
            ChildOfElement::Element(_) => has_element_child = true,
            _ => {}
        }
    }

    if let Some(text) = last_text {
        current.clear_children();
        current.append_child(document.create_text(text.text()));
        return;
    }

    if !has_element_child {
        let current_has_element_child = current
            .children()
            .into_iter()
            .any(|c| matches!(c, ChildOfElement::Element(_)));
        if current_has_element_child {
            // `child` re-declared this field empty, but the inherited value has real content --
            // RimWorld keeps the inherited value untouched in this case (a frequent source of
            // modder confusion; faithfully reproduced here, not a RimEdit bug).
            return;
        }
        current.clear_children();
        return;
    }

    for c in child.children() {
        let ChildOfElement::Element(child_el) = c else {
            continue;
        };
        if is_list_element(child_el) {
            current.append_child(clone_element(document, child_el));
            continue;
        }
        match first_child_element_named(current, child_el.name().local_part()) {
            Some(existing) => merge_child_into(document, child_el, existing),
            None => current.append_child(clone_element(document, child_el)),
        }
    }
}

fn is_list_element(el: Element<'_>) -> bool {
    if el.name().local_part() == "li" {
        return true;
    }
    match el.parent() {
        Some(ParentOfChild::Element(parent)) => {
            ALLOW_DUPLICATE_NODES_FIELD_NAMES.contains(&parent.name().local_part())
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patches::dom::{child_elements_named, element_text, parse_fragment};
    use sxd_document::Package;

    // Parses `xml` (the full contents of a `<Defs>...</Defs>` block, without the wrapper) as a
    // combined document's top-level Def elements.
    fn parse_top_level<'d>(document: Document<'d>, xml: &str) -> Vec<Element<'d>> {
        let result = parse_fragment(document, xml);
        assert!(!result.had_fatal_error, "{:?}", result.diagnostics);
        result
            .nodes
            .into_iter()
            .filter_map(|n| match n {
                ChildOfElement::Element(el) => Some(el),
                _ => None,
            })
            .collect()
    }

    fn find_by_def_name<'d>(defs: &[Element<'d>], def_name: &str) -> Element<'d> {
        *defs
            .iter()
            .find(|&&el| def_name_of(el).as_deref() == Some(def_name))
            .unwrap_or_else(|| panic!("no def named {}", def_name))
    }

    #[test]
    fn single_parent_scalar_override_and_inherited_field() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = parse_top_level(
            doc,
            r#"
            <ThingDef Name="BaseThing" Abstract="True">
                <defName>BaseThing</defName>
                <statBases><MoveSpeed>1</MoveSpeed></statBases>
            </ThingDef>
            <ThingDef ParentName="BaseThing">
                <defName>Wall</defName>
                <statBases><MoveSpeed>0</MoveSpeed></statBases>
            </ThingDef>
            "#,
        );
        let resolution = resolve_inheritance(doc, &defs);
        assert!(
            resolution.diagnostics.is_empty(),
            "{:?}",
            resolution.diagnostics
        );

        let wall = find_by_def_name(&defs, "Wall");
        let resolved = resolution.resolve(wall);
        let stat_bases = first_child_element_named(resolved, "statBases").unwrap();
        let move_speed = first_child_element_named(stat_bases, "MoveSpeed").unwrap();
        assert_eq!(element_text(move_speed), Some("0".to_string()));
    }

    #[test]
    fn multi_level_parent_chain_merges_transitively() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = parse_top_level(
            doc,
            r#"
            <ThingDef Name="Grandparent">
                <defName>Grandparent</defName>
                <a>1</a>
                <b>2</b>
            </ThingDef>
            <ThingDef Name="Parent" ParentName="Grandparent">
                <defName>Parent</defName>
                <b>20</b>
            </ThingDef>
            <ThingDef ParentName="Parent">
                <defName>Child</defName>
                <c>300</c>
            </ThingDef>
            "#,
        );
        let resolution = resolve_inheritance(doc, &defs);
        assert!(
            resolution.diagnostics.is_empty(),
            "{:?}",
            resolution.diagnostics
        );

        let child = find_by_def_name(&defs, "Child");
        let resolved = resolution.resolve(child);
        assert_eq!(
            element_text(first_child_element_named(resolved, "a").unwrap()),
            Some("1".to_string())
        );
        assert_eq!(
            element_text(first_child_element_named(resolved, "b").unwrap()),
            Some("20".to_string())
        );
        assert_eq!(
            element_text(first_child_element_named(resolved, "c").unwrap()),
            Some("300".to_string())
        );
    }

    #[test]
    fn abstract_parent_is_usable_but_not_selected_directly() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = parse_top_level(
            doc,
            r#"
            <ThingDef Name="BaseThing" Abstract="True">
                <label>base</label>
            </ThingDef>
            <ThingDef ParentName="BaseThing">
                <defName>Wall</defName>
            </ThingDef>
            "#,
        );
        let resolution = resolve_inheritance(doc, &defs);
        let wall = find_by_def_name(&defs, "Wall");
        let resolved = resolution.resolve(wall);
        assert_eq!(
            element_text(first_child_element_named(resolved, "label").unwrap()),
            Some("base".to_string())
        );
    }

    #[test]
    fn missing_parent_reports_diagnostic_and_falls_back_to_self() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = parse_top_level(
            doc,
            r#"<ThingDef ParentName="DoesNotExist"><defName>Wall</defName></ThingDef>"#,
        );
        let resolution = resolve_inheritance(doc, &defs);
        assert_eq!(resolution.diagnostics.len(), 1);
        assert_eq!(resolution.diagnostics[0].code, "inheritance_missing_parent");
        assert_eq!(
            resolution.diagnostics[0].args["parentName"],
            crate::diagnostics::DiagnosticArgValue::Text("DoesNotExist".to_string())
        );

        let wall = find_by_def_name(&defs, "Wall");
        let resolved = resolution.resolve(wall);
        assert_eq!(resolved, wall);
    }

    #[test]
    fn inheritance_diagnostic_wire_shape_omits_empty_args() {
        let diag =
            InheritanceDiagnostic::error("inheritance_cycle", "Cyclic hierarchy", "Wall", None);
        let json = serde_json::to_value(&diag).unwrap();
        assert_eq!(json["code"], "inheritance_cycle");
        assert!(json.get("args").is_none());
    }

    #[test]
    fn parent_cycle_reports_diagnostics_for_both_nodes() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = parse_top_level(
            doc,
            r#"
            <ThingDef Name="A" ParentName="B"><defName>A</defName></ThingDef>
            <ThingDef Name="B" ParentName="A"><defName>B</defName></ThingDef>
            "#,
        );
        let resolution = resolve_inheritance(doc, &defs);
        let cycle_diags: Vec<_> = resolution
            .diagnostics
            .iter()
            .filter(|d| d.code == "inheritance_cycle")
            .collect();
        assert_eq!(cycle_diags.len(), 2, "{:?}", resolution.diagnostics);
    }

    #[test]
    fn scalar_field_override_replaces_parent_value() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = parse_top_level(
            doc,
            r#"
            <ThingDef Name="Base"><defName>Base</defName><value>1</value></ThingDef>
            <ThingDef ParentName="Base"><defName>Child</defName><value>2</value></ThingDef>
            "#,
        );
        let resolution = resolve_inheritance(doc, &defs);
        let child = find_by_def_name(&defs, "Child");
        let resolved = resolution.resolve(child);
        assert_eq!(
            element_text(first_child_element_named(resolved, "value").unwrap()),
            Some("2".to_string())
        );
    }

    #[test]
    fn list_items_merge_by_appending_not_matching() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = parse_top_level(
            doc,
            r#"
            <ThingDef Name="Base">
                <defName>Base</defName>
                <comps><li>CompA</li></comps>
            </ThingDef>
            <ThingDef ParentName="Base">
                <defName>Child</defName>
                <comps><li>CompB</li></comps>
            </ThingDef>
            "#,
        );
        let resolution = resolve_inheritance(doc, &defs);
        let child = find_by_def_name(&defs, "Child");
        let resolved = resolution.resolve(child);
        let comps = first_child_element_named(resolved, "comps").unwrap();
        let items = child_elements_named(comps, "li");
        assert_eq!(items.len(), 2);
        assert_eq!(element_text(items[0]), Some("CompA".to_string()));
        assert_eq!(element_text(items[1]), Some("CompB".to_string()));
    }

    #[test]
    fn inherit_false_replaces_list_wholesale() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = parse_top_level(
            doc,
            r#"
            <ThingDef Name="Base">
                <defName>Base</defName>
                <comps><li>CompA</li><li>CompB</li></comps>
            </ThingDef>
            <ThingDef ParentName="Base">
                <defName>Child</defName>
                <comps Inherit="False"><li>CompC</li></comps>
            </ThingDef>
            "#,
        );
        let resolution = resolve_inheritance(doc, &defs);
        let child = find_by_def_name(&defs, "Child");
        let resolved = resolution.resolve(child);
        let comps = first_child_element_named(resolved, "comps").unwrap();
        let items = child_elements_named(comps, "li");
        assert_eq!(items.len(), 1);
        assert_eq!(element_text(items[0]), Some("CompC".to_string()));
    }

    #[test]
    fn patch_modifying_abstract_parent_before_inheritance_is_visible_in_child() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = parse_top_level(
            doc,
            r#"
            <ThingDef Name="Base"><defName>Base</defName><value>1</value></ThingDef>
            <ThingDef ParentName="Base"><defName>Child</defName></ThingDef>
            "#,
        );
        // Simulate a patch operation editing the abstract parent's field *before* inheritance
        // resolution runs (patches always apply to the pre-inheritance document).
        let base = find_by_def_name(&defs, "Base");
        let value_el = first_child_element_named(base, "value").unwrap();
        value_el.set_text("99");

        let resolution = resolve_inheritance(doc, &defs);
        let child = find_by_def_name(&defs, "Child");
        let resolved = resolution.resolve(child);
        assert_eq!(
            element_text(first_child_element_named(resolved, "value").unwrap()),
            Some("99".to_string())
        );
    }

    #[test]
    fn empty_child_field_does_not_clear_inherited_object_value() {
        let package = Package::new();
        let doc = package.as_document();
        let defs = parse_top_level(
            doc,
            r#"
            <ThingDef Name="Base">
                <defName>Base</defName>
                <comps><li>CompA</li></comps>
            </ThingDef>
            <ThingDef ParentName="Base">
                <defName>Child</defName>
                <comps></comps>
            </ThingDef>
            "#,
        );
        let resolution = resolve_inheritance(doc, &defs);
        let child = find_by_def_name(&defs, "Child");
        let resolved = resolution.resolve(child);
        let comps = first_child_element_named(resolved, "comps").unwrap();
        assert_eq!(child_elements_named(comps, "li").len(), 1);
    }
}
