# TubeExtract (yt-downloder) - Technical Document

## Technology Stack Overview

### Frontend Stack
| Technology | Version | Purpose |
|------|------|------|
| React | 19.2.0 | UI framework |
| TypeScript | 5.9.3 | Static typing |
| Vite | 7.3.1 | Bundler and dev server |
| React Router DOM | 7.13.0 | Routing |
| React Query | 5.90.21 | Server/async state management |
| Zustand | 5.0.11 | Client state management |
| React Hook Form | 7.71.2 | Form state management |
| Radix UI | latest | Accessible UI primitives |
| Tailwind CSS | 4.2.0 | Utility-first CSS |
| Lucide React | 0.575.0 | Icon set |
| i18next | 25.8.13 | Internationalization |

### Desktop Runtime
| Technology | Version | Purpose |
|------|------|------|
| Tauri | 2.0 | Desktop runtime |
| Rust | 1.70+ | Backend implementation |

### Development Tooling
| Technology | Version | Purpose |
|------|------|------|
| Vitest | 3.2.4 | Unit testing |
| @testing-library/react | 16.3.2 | Component testing |
| ESLint | 9.39.1 | Linting |
| TypeScript ESLint | 8.56.0 | TS linting rules |
| Babel React Compiler | 1.0.0 | React compile-time optimization |

### External Dependencies
| Tool | Purpose |
|------|------|
| yt-dlp | YouTube metadata extraction and download |
| FFmpeg | Media conversion and processing |

## Framework Selection Rationale

### React 19 + TypeScript
Reasons:
- Modern React features and performance profile
- Strong type safety that reduces runtime errors
- Mature ecosystem and tooling
- Productive development workflow

Benefits:
- Declarative UI composition
- Reusable component architecture
- High-quality IDE support and refactoring safety

### Vite
Reasons:
- Fast startup and rebuild cycles via esbuild pipeline
- Great DX with hot module replacement
- Smooth integration with the Tauri frontend workflow

Benefits:
- Faster local feedback loops
- Optimized production bundles with tree shaking

### React Query
Reasons:
- Standardized async state handling
- Built-in caching/retries/background refetch

Benefits:
- Less boilerplate for API workflows
- Predictable synchronization behavior

### Zustand
Reasons:
- Lightweight and straightforward state model
- Lower overhead than heavier alternatives

Benefits:
- Efficient selective subscriptions
- Simple extension via middleware patterns

### Tauri
Reasons:
- Smaller bundles and lower memory footprint than Electron
- Rust-based safety and performance model
- Cross-platform runtime support

Benefits:
- Native-level performance characteristics
- Direct integration with OS/filesystem capabilities

### Tailwind CSS + Radix UI
Reasons:
- Fast and consistent UI implementation
- Accessibility-first primitives with flexible styling

Benefits:
- Reduced custom CSS overhead
- Better baseline accessibility compliance

### i18next
Reasons:
- Robust i18n workflows and namespace support
- Runtime language switching

Benefits:
- Scalable translation management
- Smooth integration with React hooks

## Architecture

### Layered Model

```text
UI Layer (React + Radix + Tailwind)
    ↓
State Layer (Zustand + React Query + React Hook Form)
    ↓
Desktop Bridge (desktopClient IPC)
    ↓
Tauri Runtime (Rust commands)
    ↓
External Services (yt-dlp, FFmpeg)
```

### Domain-Oriented Separation

- **Setup**: URL analysis and metadata/format preparation
- **Queue**: job scheduling, lifecycle, and progress tracking
- **Settings**: user configuration, diagnostics, and update checks

## Development Environment Requirements

### Node.js and npm
- Node.js >= 18.0.0 (recommended: 20 LTS or 22 current)
- npm >= 9.0.0
- Alternatives: Bun 1.x or pnpm 8.x

### Rust and Tauri
- Rust >= 1.70
- Rustup for toolchain management
- Tauri CLI installed via npm

### Optional Tooling
- Git for version control
- VS Code with Rust Analyzer and ESLint
- Docker for cross-platform builds

### OS-Level Build Dependencies
| OS | Requirements |
|----|----|
| Windows | Visual Studio Build Tools 2019+ or gcc |
| macOS | Xcode Command Line Tools (10.15+) |
| Linux | gcc, gtk3-dev, libssl-dev, etc. |

## Build and Release Configuration

### Local Development Setup

1. Install dependencies
```bash
npm install
# or
npm ci
```

2. Start development
```bash
npm run dev         # Vite dev server (port 1420)
npm run tauri:dev   # Tauri + Rust dev mode
```

3. Build artifacts
```bash
npm run build       # frontend build -> dist/
npm run tauri:build # desktop bundle build
```

### Build Pipeline

#### Frontend (Vite)
```text
TypeScript compile -> lint -> transform -> bundle -> minify -> dist/
```

#### Backend (Tauri/Rust)
```text
cargo dependency resolve -> Rust compile -> platform binary -> bundle artifacts
```

### CI/CD

#### Test Pipeline (`.github/workflows/tests.yml`)
- Trigger: PR or push to `main`
- Steps: Node setup -> install -> typecheck -> lint -> unit tests

#### Release Pipeline (`.github/workflows/release.yml`)
- Trigger: semantic version tag push (for example `v0.1.0`)
- Steps: Rust build -> frontend build -> bundle generation -> release publish

## Performance Optimization

### Frontend
- Route-level code splitting
- Tree shaking and dead-style elimination
- React compiler optimizations where applicable

### State Management
- Selective subscriptions in Zustand
- Memoization (`useMemo`, `useCallback`) where justified
- React Query cache policy tuning

### Runtime
- Async/non-blocking backend tasks
- Streaming for large downloads
- Minimized unnecessary system calls

## Security Considerations

### Frontend
- XSS mitigation through React escaping model
- Schema-based input validation in forms

### Backend (Tauri/Rust)
- Command-level permission constraints
- File path validation before filesystem actions
- Controlled subprocess execution (`yt-dlp`, `FFmpeg`)

### Data Safety
- Secure handling of temporary files
- Safe local settings storage practices

## Dependency Lifecycle Management

### Update Strategy
- Monthly security review (`npm audit` and Cargo advisories)
- Quarterly minor/patch upgrade review
- Planned major upgrades with explicit test windows

### Bundle Monitoring Targets
- Frontend bundle target: < 500 KB gzip
- Desktop bundle target: < 150 MB (macOS DMG baseline)

---

Document Date: 2026-03-01  
Author: MoAI Documentation System  
Version: 1.0.0
