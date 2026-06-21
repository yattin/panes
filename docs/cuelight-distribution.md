# CueLight Distribution

CueLight is shipped as a Tauri flavor of this workspace while the main Panes distribution remains intact.

## Build Commands

```bash
pnpm tauri:dev:cuelight
pnpm tauri:build:cuelight
```

The CueLight flavor uses `src-tauri/tauri.cuelight.conf.json` as an overlay on top of the main Tauri config.

## Current Flavor Settings

| Setting | Value |
|---|---|
| Product name | `CueLight` |
| Bundle identifier | `com.panes.cuelight` |
| Window title | `CueLight` |
| Updater feed | `https://wygoralves.github.io/panes/cuelight/latest.json` |
| Updater artifacts | Enabled |

The main `src-tauri/tauri.conf.json` still builds Panes. Do not rename the main distribution to CueLight until C8 in `claurst-vendor-integration.md` is resolved.

## Release Copy

Short description:

> CueLight is a desktop creative cockpit for film and video projects, combining local workspaces, project-aware AI assistance, structured approvals, and direct CueLight project tools.

Long description:

> CueLight brings the CueLight project model into a native desktop workspace. It helps film teams bind a CueLight project, inspect scripts and production assets, coordinate storyboards, characters, scenes, props, and generated media, and work with a built-in CueLight Agent that can call project tools while preserving local file access and explicit action approval.

Release note baseline:

> This build introduces the CueLight Agent as the default native chat engine, integrates CueLight project binding into onboarding and workspace creation, and ships a dedicated CueLight desktop flavor with its own bundle identifier and updater feed.

## Remaining Brand Assets

The flavor uses its own CueLight icon set generated from `src-tauri/icons-cuelight/source.svg`. The Tauri overlay points at these assets:

- `src-tauri/icons-cuelight/32x32.png`
- `src-tauri/icons-cuelight/64x64.png`
- `src-tauri/icons-cuelight/128x128.png`
- `src-tauri/icons-cuelight/128x128@2x.png`
- `src-tauri/icons-cuelight/icon.png`
- `src-tauri/icons-cuelight/icon.icns`
- `src-tauri/icons-cuelight/icon.ico`
- Windows Store logo variants under `src-tauri/icons-cuelight/`

Regenerate them with:

```bash
pnpm tauri icon src-tauri/icons-cuelight/source.svg -o src-tauri/icons-cuelight
```

Keep future icon revisions separate from runtime work so generated binary assets do not obscure claurst-native implementation diffs.
