# Graphic Preview Fixtures

Placeholder texture files for fixture-backed preview resolver tests.

- `project_mod/` — simulates the user's project mod location
- `source_mod/` — simulates a read-only source/dependency mod location

Files are intentionally tiny placeholder bytes. The resolver only needs paths;
browser image decoding is tested at the component level with mocked URLs.

`project_mod` overrides `source_mod` for matching paths (e.g. `FixtureSingle.png`).
`SourceOnly.png` is absent from `project_mod`, proving source locations fill gaps.
