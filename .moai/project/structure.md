# TubeExtract (yt-downloder) - Project Structure Document

## Directory Tree

```text
yt-downloder/
├── .claude/                           # MoAI-ADK Claude configuration
│   ├── skills/                        # Custom skill definitions
│   ├── agents/                        # Custom agent definitions
│   ├── commands/                      # Custom command definitions
│   ├── rules/                         # Project rules
│   └── hooks/                         # Event hooks
├── .moai/                             # MoAI-ADK primary configuration
│   ├── config/                        # Configuration files
│   ├── specs/                         # SPEC documents
│   ├── docs/                          # Generated documents
│   ├── project/                       # Project metadata
│   └── cache/                         # Cache data
├── .github/
│   └── workflows/                     # GitHub Actions automation
│       ├── tests.yml                  # Test pipeline
│       └── release.yml                # Release automation
├── public/                            # Static assets
├── src/                               # React frontend source
│   ├── main.tsx                       # App entry point
│   ├── App.tsx                        # Top-level component
│   ├── index.css                      # Global styles
│   ├── App.css                        # App styles
│   ├── tauri.d.ts                     # Tauri TypeScript definitions
│   ├── assets/                        # Images, fonts, etc.
│   ├── renderer/
│   │   ├── domains/                   # Domain modules
│   │   │   ├── setup/                 # URL analysis domain
│   │   │   ├── queue/                 # Download queue domain
│   │   │   └── settings/              # Settings domain
│   │   ├── components/                # Shared UI components
│   │   ├── hooks/                     # Shared custom hooks
│   │   ├── lib/                       # Utility library
│   │   ├── queries/                   # React Query definitions
│   │   ├── store/                     # Zustand stores
│   │   └── i18n/                      # Internationalization files
│   └── test/                          # Test setup and mocks
├── src-tauri/                         # Tauri Rust backend
│   ├── src/
│   │   ├── lib.rs                     # Main Tauri runtime and commands
│   │   └── main.rs                    # Tauri app entry point
│   ├── Cargo.toml                     # Rust dependency management
│   ├── tauri.conf.json                # Tauri configuration
│   └── target/                        # Rust build artifacts
├── node_modules/                      # npm packages (gitignored)
├── dist/                              # Frontend build artifacts
├── release/                           # Release build artifacts
├── CLAUDE.md                          # MoAI-ADK project directives
├── package.json                       # npm main configuration
├── package-lock.json                  # npm dependency lockfile
├── tsconfig.json                      # TypeScript configuration
├── vite.config.ts                     # Vite configuration
├── vitest.config.ts                   # Vitest configuration
├── eslint.config.js                   # ESLint rules
├── tailwind.config.ts                 # Tailwind configuration
├── index.html                         # HTML entry point
├── README.md                          # Project README
└── .gitignore                         # Git ignore rules
```

## Purpose of Core Directories

### `src/renderer/domains/`
**Purpose**: DDD-oriented separation of features by domain.

Each domain (`setup`, `queue`, `settings`) contains:
- `components/`: domain-specific UI
- `hooks/`: domain logic hooks
- `store/`: Zustand state layer
- `[Domain]Page.tsx`: domain page component

This structure improves cohesion and reduces merge conflicts in team workflows.

### `src/renderer/components/`
**Purpose**: Shared UI components reusable across domains.

The `ui/` folder contains Radix UI-based primitives (Button, Modal, Input, etc.).

### `src/renderer/lib/`
**Purpose**: Cross-domain utility functions and shared library code.

- `desktopClient.ts`: IPC bridge between frontend and Rust backend
- `types.ts`: shared TypeScript types
- `constants.ts`: global app constants

### `src/renderer/queries/`
**Purpose**: React Query configuration and centralized data-fetching policies.

### `src/renderer/store/`
**Purpose**: Global client-side state management with Zustand.

### `src/renderer/i18n/`
**Purpose**: i18n configuration and locale resources.

- `ko.json`: Korean translations
- `en.json`: English translations

### `src-tauri/src/`
**Purpose**: Tauri Rust backend implementation.

`lib.rs` implements 18 Tauri commands for URL analysis, queue control, filesystem operations, settings, diagnostics, and update checks.

### `.github/workflows/`
**Purpose**: CI/CD automation.

- `tests.yml`: automated tests on PR/main pushes
- `release.yml`: automated build and release on version tags

### `.moai/`
**Purpose**: MoAI-ADK framework configuration.

- `config/`: project settings and quality criteria
- `specs/`: SPEC docs and requirements
- `docs/`: generated documentation
- `project/`: project metadata

## Module Composition

### Frontend Modules (`src/renderer/`)

```text
Renderer App
├── Setup Domain      -> URL analysis and metadata extraction
├── Queue Domain      -> Queue management and progress tracking
└── Settings Domain   -> Preferences and environment management

Shared Layer
├── State (Zustand + React Query)
├── UI components (Radix UI + Tailwind)
├── Custom hooks
└── Utilities (desktopClient IPC bridge)

Support Layer
├── i18n
├── Test infrastructure
└── Type definitions
```

### Backend Modules (`src-tauri/src/`)

```text
Tauri Runtime (lib.rs)
├── URL Analysis Module
│   ├── analyze_url
│   └── check_duplicate
├── Download Queue Module
│   ├── enqueue_job
│   ├── pause_job
│   ├── resume_job
│   ├── cancel_job
│   └── clear_terminal_jobs
├── Filesystem Module
│   ├── delete_file
│   ├── open_folder
│   └── open_external_url
├── State Query Module
│   ├── get_queue_snapshot
│   ├── get_settings
│   └── get_storage_stats
├── Settings Module
│   ├── pick_download_dir
│   └── set_settings
└── Diagnostics Module
    ├── run_diagnostics
    ├── check_update
    └── get_dependency_bootstrap_status
```

## Key File Locations

| Capability | Path |
|------|------|
| Type definitions | `src/renderer/lib/types.ts` |
| IPC bridge | `src/renderer/lib/desktopClient.ts` |
| Tauri commands | `src-tauri/src/lib.rs` |
| Setup page | `src/renderer/domains/setup/SetupPage.tsx` |
| Queue page | `src/renderer/domains/queue/QueuePage.tsx` |
| Settings page | `src/renderer/domains/settings/SettingsPage.tsx` |
| Tests | `src/**/*.test.tsx` |
| Build config | `vite.config.ts`, `tsconfig.json` |
| Test config | `vitest.config.ts` |
| Styling | `tailwind.config.ts`, `src/**/*.css` |
| i18n resources | `src/renderer/i18n/locales/*.json` |
