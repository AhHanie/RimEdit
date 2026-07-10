use crate::def_index::{DefIndex, DefIndexError};
use crate::project_files::scan_xml_files;
use crate::project_model::{AppError, ProjectSettings};
use crate::schema_pack::SchemaCatalog;
use crate::xml_document::{parse_to_document, validate_document, ValidationContext};
use crate::xml_document::{ParseDiagnostic, ValidationDiagnostic};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectValidationResult {
    pub project_id: String,
    pub parse_diagnostics: Vec<ParseDiagnostic>,
    pub validation_diagnostics: Vec<ValidationDiagnostic>,
    pub index_errors: Vec<DefIndexError>,
}

pub fn validate_project(
    settings: &ProjectSettings,
    project_id: &str,
    catalog: &SchemaCatalog,
    def_index: &DefIndex,
) -> Result<ProjectValidationResult, AppError> {
    let scan = scan_xml_files(settings, project_id).map_err(AppError::from)?;
    let root = PathBuf::from(&scan.project_root);
    let context = ValidationContext { catalog, def_index };
    let mut parse_diagnostics = Vec::new();
    let mut validation_diagnostics = Vec::new();

    for file in scan.files {
        let path = root.join(Path::new(&file.relative_path));
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        let doc = parse_to_document(&file.relative_path, &source);
        parse_diagnostics.extend(doc.parse_diagnostics.clone());
        if !doc.had_fatal_parse_error {
            validation_diagnostics.extend(validate_document(&doc, &context));
        }
    }

    Ok(ProjectValidationResult {
        project_id: project_id.to_string(),
        parse_diagnostics,
        validation_diagnostics,
        index_errors: def_index
            .errors
            .iter()
            .filter(|e| e.location_id == project_id)
            .cloned()
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::def_index::{build_def_index, DefIndexBuildOptions};
    use crate::project_model::{LocationKind, RegisteredLocation, SourceType};
    use crate::schema_pack::build_schema_catalog;
    use std::fs;
    use time::OffsetDateTime;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rimedit_project_validation_{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn location(
        id: &str,
        display_name: &str,
        root: &Path,
        kind: LocationKind,
        source_type: SourceType,
        read_only: bool,
    ) -> RegisteredLocation {
        RegisteredLocation {
            id: id.to_string(),
            display_name: display_name.to_string(),
            root_path: root.to_string_lossy().to_string(),
            kind,
            source_type,
            read_only,
            mod_id: None,
            game_version: None,
            expansion_name: None,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        }
    }

    #[test]
    fn validates_recipe_maker_references_from_core_base_game_defs() {
        let project_dir = temp_dir();
        let data_dir = temp_dir();
        fs::create_dir(project_dir.join("Defs")).unwrap();
        let core_items = data_dir.join("Core").join("Defs").join("ThingDefs_Items");
        let biotech_items = data_dir
            .join("Biotech")
            .join("Defs")
            .join("ThingDefs_Items");
        let core_buildings = data_dir
            .join("Core")
            .join("Defs")
            .join("ThingDefs_Buildings");
        let anomaly_buildings = data_dir
            .join("Anomaly")
            .join("Defs")
            .join("ThingDefs_Buildings");
        fs::create_dir_all(&core_items).unwrap();
        fs::create_dir_all(&biotech_items).unwrap();
        fs::create_dir_all(&core_buildings).unwrap();
        fs::create_dir_all(&anomaly_buildings).unwrap();
        fs::write(
            core_items.join("Items_Unfinished.xml"),
            "<Defs><ThingDef><defName>UnfinishedTechArmor</defName></ThingDef></Defs>",
        )
        .unwrap();
        fs::write(
            biotech_items.join("Items_Unfinished.xml"),
            "<Defs><ThingDef><defName>BiotechUnfinished</defName></ThingDef></Defs>",
        )
        .unwrap();
        fs::write(
            core_buildings.join("Buildings_Production.xml"),
            "<Defs><ThingDef><defName>FabricationBench</defName></ThingDef></Defs>",
        )
        .unwrap();
        fs::write(
            anomaly_buildings.join("Buildings_Production.xml"),
            "<Defs><ThingDef><defName>AnomalyProduction</defName></ThingDef></Defs>",
        )
        .unwrap();
        fs::write(
            project_dir.join("Defs").join("Recipes.xml"),
            r#"<Defs>
  <ThingDef>
    <defName>TestArmor</defName>
    <recipeMaker>
      <unfinishedThingDef>UnfinishedTechArmor</unfinishedThingDef>
      <recipeUsers>
        <li>FabricationBench</li>
      </recipeUsers>
    </recipeMaker>
  </ThingDef>
</Defs>"#,
        )
        .unwrap();

        let settings = ProjectSettings {
            schema_version: 2,
            game_version: "1.6".to_string(),
            locations: vec![
                location(
                    "project",
                    "Project",
                    &project_dir,
                    LocationKind::Project,
                    SourceType::Folder,
                    false,
                ),
                location(
                    "base",
                    "RimWorld Data",
                    &data_dir,
                    LocationKind::Source,
                    SourceType::BaseGame,
                    true,
                ),
            ],
            active_project_id: Some("project".to_string()),
        };
        let catalog_result = build_schema_catalog(&[], Some("1.6"));
        let index = build_def_index(&settings, DefIndexBuildOptions::for_project("project"));

        let result =
            validate_project(&settings, "project", &catalog_result.catalog, &index).unwrap();

        assert!(
            !result.validation_diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "validation_unresolved_reference"
                    && diagnostic.message.contains("UnfinishedTechArmor")
            }),
            "unexpected diagnostics: {:?}",
            result.validation_diagnostics
        );
        assert!(
            !result.validation_diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "validation_unresolved_reference"
                    && diagnostic.message.contains("FabricationBench")
            }),
            "unexpected diagnostics: {:?}",
            result.validation_diagnostics
        );
        fs::remove_dir_all(&project_dir).ok();
        fs::remove_dir_all(&data_dir).ok();
    }
}
