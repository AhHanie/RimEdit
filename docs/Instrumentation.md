# Debug Instrumentation

A lightweight, dev-only timing service for measuring named code paths without any external dependencies or production overhead.

## Overview

Instrumentation is active only when two conditions are met simultaneously:

- **Build gate** - frontend requires `import.meta.env.DEV`; backend requires `cfg(debug_assertions)`.
- **Runtime gate** - frontend requires `VITE_RIMEDIT_INSTRUMENTATION=1`; backend requires `RIMEDIT_INSTRUMENTATION=1`.

A production build is always a no-op regardless of environment variables.

---

## Frontend

### Enabling

Set the environment variable before starting the dev server:

```powershell
$env:VITE_RIMEDIT_INSTRUMENTATION = "1"
pnpm dev
```

Output appears in the browser's developer console as structured objects prefixed with `[rimedit:timing]`.

### Runtime override

You can toggle instrumentation at runtime from the browser console without restarting the dev server:

```js
// Enable
localStorage.setItem("rimedit.instrumentation.enabled", "true");

// Disable
localStorage.setItem("rimedit.instrumentation.enabled", "false");

// Revert to env-var default
localStorage.removeItem("rimedit.instrumentation.enabled");
```

Changes take effect on the next call - no page reload needed.

### API

Import from `src/instrumentation`:

```ts
import {
  isInstrumentationEnabled,
  setInstrumentationEnabled,
  measure,
  measureAsync,
  markTiming,
} from "../instrumentation";
```

#### `measureAsync` - wrap an async operation

```ts
export function loadDocument(projectId: string, relativePath: string) {
  return measureAsync(
    "xmlEditor.loadProjectDocument",
    () =>
      invoke("read_project_xml_editor_document", { projectId, relativePath }),
    { sourceKind: "project", relativePath },
  );
}
```

#### `measure` - wrap a synchronous operation

```ts
const sorted = measure("defIndex.sortResults", () => results.sort(comparator), {
  resultCount: results.length,
});
```

#### `markTiming` - record a duration you measured yourself

```ts
const start = performance.now();
// ... work ...
markTiming("xmlEditor.parseBuffer", performance.now() - start, {
  relativePath,
});
```

### Operation naming

Use stable dotted names. Do not embed IDs, paths, or cardinality in the name - put those in tags.

```
xmlEditor.loadProjectDocument
xmlEditor.loadLocationDocument
xmlEditor.parseBuffer
xmlEditor.applyEdit
xmlEditor.applyEdits
xmlEditor.savePreview
xmlEditor.saveFile
projectExplorer.scanFiles
defIndex.rebuild
defIndex.search
graphicPreview.resolveAssets
```

### Tags

Keep tags low-cardinality and free of sensitive content.

```ts
// Good
{ relativePath, sourceKind: "project" }
{ queryLength: query.length }
{ batchSize: edits.length }
{ operation: "form" }

// Avoid
{ rawXml, query, fieldValue }   // raw content
{ absolutePath }                 // absolute filesystem paths
```

---

## Backend

### Enabling

Set the environment variable before starting Tauri in dev mode:

```powershell
$env:RIMEDIT_INSTRUMENTATION = "1"
pnpm tauri dev
```

Output appears in the Tauri dev terminal as one-line strings:

```
[rimedit:timing] source=backend name=commands.parseXmlEditorBuffer durationMs=12.42 relativePath=Defs/ThingDefs.xml
```

### Runtime toggle

Two Tauri commands are registered for toggling instrumentation without restarting the process. Call them from the frontend during a dev session:

```ts
import { invoke } from "@tauri-apps/api/core";

// Check current state
const config = await invoke("get_instrumentation_config");
// { available: true, enabled: true, sink: "console" }

// Enable
await invoke("set_instrumentation_enabled", { enabled: true });

// Disable
await invoke("set_instrumentation_enabled", { enabled: false });
```

In a release build these commands return `{ available: false, enabled: false }` and `set_instrumentation_enabled` is a no-op.

### API

Import from `crate::instrumentation`:

```rust
use crate::instrumentation;
```

#### `span` - time a scope

```rust
#[tauri::command]
pub fn scan_project_files(app: AppHandle, project_id: String) -> Result<ProjectFileScan, AppError> {
    let _span = instrumentation::span(&app, "commands.scanProjectFiles");
    // ... work ...
}
```

#### `span_with_tags` - time a scope with metadata

```rust
#[tauri::command]
pub fn parse_xml_editor_buffer(
    app: AppHandle,
    project_id: String,
    relative_path: String,
    raw_xml: String,
) -> Result<XmlEditorDocumentLoadResult, AppError> {
    let _span = instrumentation::span_with_tags(
        &app,
        "commands.parseXmlEditorBuffer",
        [("relativePath".to_string(), relative_path.clone())],
    );
    xml_editor_service::parse_editor_buffer(&app, project_id, relative_path, raw_xml)
}
```

The span records `Instant::now()` on creation and emits in `Drop`, so the timing covers the full scope including early returns and `?` propagation. The guard must be assigned to `_span` (not `_`) to avoid immediate drop.

#### `is_enabled` / `set_enabled`

```rust
if instrumentation::is_enabled(&app) {
    // conditional work only needed when recording
}

instrumentation::set_enabled(&app, true);
```

### Operation naming

```
commands.readProjectXmlEditorDocument
commands.readLocationXmlEditorDocument
commands.parseXmlEditorBuffer
commands.applyXmlEditorEdit
commands.applyXmlEditorEdits
commands.previewProjectXmlSave
commands.saveProjectXmlFile
commands.scanProjectFiles
commands.rebuildDefIndex
commands.searchDefs
commands.resolveGraphicPreviewAssets
indexing.executeFullRebuild
indexing.executeFileJobs
validation.validateDocForProject
```

### Tags

```rust
// Good
[("relativePath".to_string(), relative_path.clone())]
[("queryLength".to_string(), query.len().to_string())]
[("batchSize".to_string(), batch.len().to_string())]
[("projectPresent".to_string(), project_id.is_some().to_string())]

// Avoid
[("rawXml".to_string(), raw_xml.clone())]      // file content
[("absolutePath".to_string(), path.display())] // absolute paths
```

---

## What not to record

- Raw XML or file contents.
- Absolute filesystem paths.
- User-entered search queries or field values.
- Validation diagnostic details.

Use counts, sizes, boolean flags, and relative paths instead.

---

## Release safety

- `isInstrumentationEnabled()` always returns `false` when `import.meta.env.DEV` is `false`.
- All backend measurement code is inside `#[cfg(debug_assertions)]` blocks and compiles away entirely in release builds.
- `span_with_tags` in release builds discards all arguments immediately and returns a zero-size no-op guard.
- The `set_instrumentation_enabled` command accepts calls in release builds but has no effect.
