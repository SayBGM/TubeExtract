# Research: Auto-Update and Release System (SPEC-UPDATE-001)

## Current Auto-Update State

The auto-update feature is a **placeholder stub with no implementation**:

**File**: `src-tauri/src/diagnostics.rs` (lines 224-232)
```rust
#[tauri::command]
pub async fn check_update() -> CommandResult<Value> {
    Ok(serde_json::json!({
      "hasUpdate": false,
      "latestVersion": Value::Null,
      "url": Value::Null
    }))
}
```

- Always returns `hasUpdate: false` regardless of actual state
- No GitHub API integration
- No version comparison logic
- Registered in `lib.rs` line 102: `diagnostics::check_update`

## Tauri Updater Configuration

**Status**: NOT configured

1. `src-tauri/Cargo.toml` line 23: `tauri = { version = "2", features = [] }` — `updater` feature NOT enabled
2. `src-tauri/tauri.conf.json`: No `updater` section, no endpoint URLs, no signing keys
3. `src-tauri/capabilities/default.json`: No updater permissions granted

## Release Infrastructure

### GitHub Actions: `.github/workflows/release.yml` (136 lines)

**Trigger**: Git tags matching `v*`

**Build Matrix**:
- macOS: `macos-latest`
- Windows: `windows-latest`
- No Linux builds

**Release Process**:
1. Version sync from git tag → `package.json` + `src-tauri/tauri.conf.json` (lines 37-51)
2. Dependency bundling: yt-dlp + ffmpeg downloaded to `src-tauri/resources/bin/` (lines 62-103)
3. `npm run tauri:build`
4. Asset renaming: `{name}-{version}.{ext}` (lines 108-124)
5. GitHub Release creation with all platform artifacts (lines 126-135)

**Manual Steps Required to Release**:
```bash
git tag v0.X.Y
git push origin v0.X.Y  # Triggers CI
```

**Test Pipeline**: `.github/workflows/test.yml` — frontend-only (no Rust tests)

## Version Management

| File | Current Version | Sync Method |
|------|-----------------|-------------|
| `package.json` | `0.0.0` | CI syncs from git tag |
| `src-tauri/tauri.conf.json` | `0.0.0` | CI syncs from git tag |
| `src-tauri/Cargo.toml` | `0.1.0` | NOT synced (only validated) |

**Issue**: Cargo.toml version (0.1.0) diverges from other files (0.0.0), creating inconsistency.

## Frontend Update UI

**Files**:
1. `src/renderer/lib/desktopClient.ts` (lines 294-301): `checkUpdate()` — calls Rust stub
2. `src/renderer/domains/settings/SettingsPage.tsx` (lines 75-97): React Query mutation
3. `src/renderer/domains/settings/components/SettingsUpdateSection.tsx`: UI component with "Check Update" button

**UI Behavior**: Shows message, opens URL in browser if update found (assumes web-based download).

**i18n keys**: `settings.update.title`, `settings.update.check`, `settings.update.latest`, `settings.update.available`

## Issues Found

1. **`check_update()` is a non-functional stub** — always returns false
2. **Tauri updater plugin not enabled** — `tauri-plugin-updater` absent from Cargo.toml
3. **Cargo.toml version not synced** by CI/CD pipeline
4. **No update metadata generation** (JSON manifest, checksums, signatures)
5. **No release command** — requires manual `git tag` + `git push`
6. **No CHANGELOG update automation** in release process
7. **Frontend assumes browser-based download** (not in-app update)

## Recommendations for SPEC

### Scope A: Fix `check_update()` (검증)
- Replace stub with GitHub Releases API query
- Compare with app's current version (`tauri::app::App::package_info()`)
- Return real `hasUpdate`, `latestVersion`, `url`

### Scope B: Release Command (릴리즈 커맨드)
Create a developer CLI/npm script for releasing:
```bash
npm run release -- --version 0.2.6
# OR
npm run release -- --patch   # 0.2.5 → 0.2.6
# OR
npm run release -- --minor   # 0.2.5 → 0.3.0
# OR
npm run release -- --major   # 0.2.5 → 1.0.0
```

Script responsibilities:
1. Validate clean git working tree
2. Determine new version (semver bump or explicit)
3. Update `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`
4. Update `CHANGELOG.md` (if exists) or create entry
5. `git commit -am "chore(release): v{version}"`
6. `git tag v{version}`
7. `git push && git push --tags` (triggers CI)

### Scope C: Version Sync Fix
- Update `release.yml` to also sync `Cargo.toml` version

## File References

| File | Lines | Purpose |
|------|-------|---------|
| `src-tauri/src/diagnostics.rs` | 224-232 | `check_update()` stub |
| `src-tauri/src/lib.rs` | 102 | Command registration |
| `src-tauri/Cargo.toml` | 23 | Tauri features (no updater) |
| `src-tauri/tauri.conf.json` | 1-41 | App config (no updater section) |
| `src-tauri/capabilities/default.json` | 1-11 | Permissions (no updater) |
| `.github/workflows/release.yml` | 1-136 | Full release pipeline |
| `.github/workflows/test.yml` | 1-26 | Test workflow |
| `src/renderer/lib/desktopClient.ts` | 294-301 | `checkUpdate()` IPC |
| `src/renderer/domains/settings/SettingsPage.tsx` | 75-97 | Update mutation |
| `src/renderer/domains/settings/components/SettingsUpdateSection.tsx` | 1-40 | Update UI |

---

Generated: 2026-03-02
Author: MoAI Research Agent
