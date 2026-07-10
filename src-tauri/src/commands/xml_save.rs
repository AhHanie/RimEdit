use crate::def_index::{settings_fingerprint, DefIndexBuildOptions, DefIndexState};
use crate::project_model::AppError;
use crate::project_save::{
    compute_validation_token, hash_file_fingerprints, preview_xml_save_with_index,
    save_project_xml_with_index, try_save_with_fast_token, SavePreview, SaveResult,
    SaveValidationSecret,
};
use crate::services::app_paths;
use crate::services::def_index_cache;
use crate::services::indexing::{self, IndexJobReason};
use crate::settings_store::load_settings;
use tauri::{AppHandle, Manager};

fn save_tags(trace_id: Option<&str>, relative_path: &str) -> Vec<(String, String)> {
    let mut tags = vec![("relativePath".to_string(), relative_path.to_string())];
    if let Some(id) = trace_id {
        tags.push(("traceId".to_string(), id.to_string()));
    }
    tags
}

#[tauri::command]
pub fn preview_project_xml_save(
    app: AppHandle,
    project_id: String,
    relative_path: String,
    proposed_xml: String,
    trace_id: Option<String>,
    full_diff: Option<bool>,
) -> Result<SavePreview, AppError> {
    let xml_bytes = proposed_xml.len();
    let tid = trace_id.as_deref();
    let _span = crate::instrumentation::span_with_tags(&app, "commands.previewProjectXmlSave", {
        let mut t = save_tags(tid, &relative_path);
        t.push(("xmlBytes".to_string(), xml_bytes.to_string()));
        t
    });

    let settings = {
        let _s = crate::instrumentation::span_with_tags(
            &app,
            "commands.previewProjectXmlSave.loadSettings",
            save_tags(tid, &relative_path),
        );
        load_settings(&app)?
    };

    let base_index = {
        let _s = crate::instrumentation::span_with_tags(
            &app,
            "commands.previewProjectXmlSave.loadIndex",
            save_tags(tid, &relative_path),
        );
        def_index_cache::load_for_project(&app, &settings, &project_id, false)?
    };

    let mut preview = {
        let _s = crate::instrumentation::span_with_tags(
            &app,
            "commands.previewProjectXmlSave.validateAndDiff",
            save_tags(tid, &relative_path),
        );
        preview_xml_save_with_index(
            &settings,
            &base_index,
            &project_id,
            &relative_path,
            &proposed_xml,
            !full_diff.unwrap_or(false),
        )
        .map_err(AppError::from)?
    };

    // Compute index_fp from the in-memory cache fingerprints populated by load_for_project.
    // If unavailable (shouldn't happen), fall back to an empty string: the save fast path
    // also falls back gracefully when fingerprints are missing.
    let options = DefIndexBuildOptions {
        project_id: Some(&project_id),
        include_sources: true,
        replacement: None,
        force_rebuild: false,
    };
    let settings_fp = {
        let _s = crate::instrumentation::span_with_tags(
            &app,
            "commands.previewProjectXmlSave.computeSettingsFingerprint",
            save_tags(tid, &relative_path),
        );
        settings_fingerprint(&settings, &options)
    };
    let index_fp = {
        let _s = crate::instrumentation::span_with_tags(
            &app,
            "commands.previewProjectXmlSave.getIndexFingerprint",
            save_tags(tid, &relative_path),
        );
        app.state::<DefIndexState>()
            .get_file_fingerprints_if_settings_match(&settings_fp)
            .as_deref()
            .map(hash_file_fingerprints)
            .unwrap_or_default()
    };

    {
        let _s = crate::instrumentation::span_with_tags(
            &app,
            "commands.previewProjectXmlSave.computeValidationToken",
            save_tags(tid, &relative_path),
        );
        let secret = app.state::<SaveValidationSecret>();
        preview.validation_token = compute_validation_token(
            secret.as_bytes(),
            &project_id,
            &relative_path,
            &preview.current_hash,
            &preview.proposed_hash,
            &index_fp,
        );
    }
    Ok(preview)
}

#[tauri::command]
pub fn save_project_xml_file(
    app: AppHandle,
    project_id: String,
    relative_path: String,
    proposed_xml: String,
    validation_token: Option<String>,
    trace_id: Option<String>,
) -> Result<SaveResult, AppError> {
    let xml_bytes = proposed_xml.len();
    let tid = trace_id.as_deref();
    let mut span = crate::instrumentation::span_with_tags(&app, "commands.saveProjectXmlFile", {
        let mut t = save_tags(tid, &relative_path);
        t.push(("xmlBytes".to_string(), xml_bytes.to_string()));
        t.push((
            "tokenProvided".to_string(),
            validation_token.is_some().to_string(),
        ));
        t
    });

    let settings = {
        let _s = crate::instrumentation::span_with_tags(
            &app,
            "commands.saveProjectXmlFile.loadSettings",
            save_tags(tid, &relative_path),
        );
        load_settings(&app)?
    };
    let storage_dir = {
        let _s = crate::instrumentation::span_with_tags(
            &app,
            "commands.saveProjectXmlFile.appStorageDir",
            save_tags(tid, &relative_path),
        );
        app_paths::app_storage_dir(&app, "save_backup_failed")?
    };

    // Fast path: if a token is provided and the in-memory cache has fingerprints for the current
    // settings, verify the token without loading the full def index (no project-wide file scan).
    // Track why we fall through to the slow path so the terminal span tag explains it.
    let mut fallback_reason = "missingToken";
    if let Some(ref token) = validation_token {
        let options = DefIndexBuildOptions {
            project_id: Some(&project_id),
            include_sources: true,
            replacement: None,
            force_rebuild: false,
        };
        let settings_fp = {
            let _s = crate::instrumentation::span_with_tags(
                &app,
                "commands.saveProjectXmlFile.fastPath.settingsFingerprint",
                save_tags(tid, &relative_path),
            );
            settings_fingerprint(&settings, &options)
        };
        let file_fps = {
            let _s = crate::instrumentation::span_with_tags(
                &app,
                "commands.saveProjectXmlFile.fastPath.getFingerprints",
                save_tags(tid, &relative_path),
            );
            app.state::<DefIndexState>()
                .get_file_fingerprints_if_settings_match(&settings_fp)
        };
        if let Some(ref file_fps) = file_fps {
            let index_fp = {
                let _s = crate::instrumentation::span_with_tags(
                    &app,
                    "commands.saveProjectXmlFile.fastPath.hashFingerprints",
                    save_tags(tid, &relative_path),
                );
                hash_file_fingerprints(file_fps)
            };
            let secret = app.state::<SaveValidationSecret>();
            let fast_result = {
                let _s = crate::instrumentation::span_with_tags(
                    &app,
                    "commands.saveProjectXmlFile.fastPath.trySaveWithToken",
                    save_tags(tid, &relative_path),
                );
                try_save_with_fast_token(
                    &settings,
                    &storage_dir,
                    &project_id,
                    &relative_path,
                    &proposed_xml,
                    token,
                    secret.as_bytes(),
                    &index_fp,
                )
                .map_err(AppError::from)?
            };
            if let Some(result) = fast_result {
                {
                    let _s = crate::instrumentation::span_with_tags(
                        &app,
                        "commands.saveProjectXmlFile.fastPath.enqueueIndex",
                        save_tags(tid, &relative_path),
                    );
                    indexing::enqueue_file_change(
                        &app,
                        project_id,
                        relative_path,
                        IndexJobReason::SavedProjectFile,
                    );
                }
                span.set_tag("fastPath", "true");
                return Ok(result);
            }
            // fast_result is None - token mismatch, fall through to slow path
            fallback_reason = "tokenMismatch";
        } else {
            // No in-memory fingerprints for the current settings, fall through to slow path
            fallback_reason = "missingFingerprints";
        }
    }

    // Slow path: load the full index (may rescan files), run full semantic validation, write.
    span.set_tag("fastPath", "false");
    span.set_tag("fallbackReason", fallback_reason);
    let base_index = {
        let _s = crate::instrumentation::span_with_tags(
            &app,
            "commands.saveProjectXmlFile.slowPath.loadIndex",
            save_tags(tid, &relative_path),
        );
        def_index_cache::load_for_project(&app, &settings, &project_id, false)?
    };
    let result = {
        let _s = crate::instrumentation::span_with_tags(
            &app,
            "commands.saveProjectXmlFile.slowPath.validateAndWrite",
            save_tags(tid, &relative_path),
        );
        save_project_xml_with_index(
            &settings,
            &storage_dir,
            &base_index,
            &project_id,
            &relative_path,
            &proposed_xml,
        )
        .map_err(AppError::from)?
    };
    {
        let _s = crate::instrumentation::span_with_tags(
            &app,
            "commands.saveProjectXmlFile.slowPath.enqueueIndex",
            save_tags(tid, &relative_path),
        );
        indexing::enqueue_file_change(
            &app,
            project_id,
            relative_path,
            IndexJobReason::SavedProjectFile,
        );
    }
    Ok(result)
}
