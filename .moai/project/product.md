# TubeExtract (yt-downloder) - Product Document

## Project Overview

**Project Name**: TubeExtract (yt-downloder)  
**Description**: Desktop application for YouTube URL analysis and video/audio download  
**Current Version**: 0.0.0 (in development)  
**Implementation Languages**: TypeScript + Rust  
**Runtime**: Tauri 2.0 desktop application  
**Target Users**: Individual users and content creators

## Core Features

### 1. YouTube URL Analysis
- Extract metadata from YouTube URLs
- Show video title, channel, duration, and thumbnail
- Display available format options (resolution, audio quality)
- Validate URLs and detect duplicates

### 2. Download Queue Management
- Add multiple videos to a queue for sequential processing
- Show download progress in real time
- Pause/resume/cancel controls per item
- Track download history

### 3. Format Selection
- Video download: multiple resolution options (480p, 720p, 1080p)
- Audio download: multiple audio formats (MP3, AAC, etc.)
- Save preferred formats and auto-apply defaults

### 4. File Management
- Open completed files directly
- Open the configured download folder automatically
- Delete completed files
- Customize download paths

### 5. Settings Management
- Configure download destination
- Configure retry count
- Select interface language (Korean/English)
- Manage automatic installation of yt-dlp and FFmpeg

### 6. System Diagnostics
- Environment diagnostics for required tools (yt-dlp, FFmpeg)
- Update checks for latest version
- Dependency health monitoring

## Key Value Propositions

### 1. Ease of Use
- Intuitive UI that works without technical expertise
- Fast URL analysis for immediate format selection
- Bilingual support for accessibility

### 2. Reliability
- Automatic dependency management removes setup friction
- Retry mechanisms handle transient network failures
- Format validation helps prevent corrupted output files

### 3. Efficiency
- Batch download workflow for multiple videos
- Pause/resume for flexible workload control
- Quick access to completed items

### 4. Transparency
- Real-time progress visibility
- Detailed and actionable error messages
- Built-in diagnostics for self-service troubleshooting

## Use Cases

### 1. Personal Learning and Offline Watching
- Download lecture videos for offline study
- Extract audio from podcast/music videos
- Maintain offline backups of favorite content

### 2. Content Creators
- Bulk download reference materials
- Download multiple formats for post-production
- Improve time efficiency via queue management

### 3. Educational Organizations
- Collect video assets for lecture archives
- Deliver content in offline classroom environments
- Batch-download student learning resources

### 4. Media Archiving
- Preserve important video content long-term
- Store multiple formats for compatibility
- Track assets with metadata-aware workflows

## Technical Architecture

### Layered Structure

```text
User Interface (React UI)
    ↓
State Management (Zustand + React Query)
    ↓
Desktop Bridge (desktopClient IPC)
    ↓
Tauri Runtime (Rust)
    ↓
External Services (yt-dlp, FFmpeg)
```

### Domain Structure

- **Setup Domain**: URL analysis, metadata extraction, format selection
- **Queue Domain**: queue management, progress tracking, job controls
- **Settings Domain**: user preference persistence, language/path management

## Competitive Advantages

### Compared to Web-Based Tools
- Local application avoids server-side transfers
- Better privacy: no upload of user data
- Automatic retry behavior for bulk downloads
- Offline-capable workflows

### Compared to CLI Tools
- GUI removes command-line learning requirements
- Visual progress and state visibility
- More user-friendly error messaging
- Automatic dependency bootstrap

## Roadmap

### Current Phase (v0.0.0)
- Core URL analysis
- Basic download queue management
- Korean/English language support
- Automatic dependency management

### Planned Enhancements
- Playlist batch download
- Automatic thumbnail saving
- Download scheduling
- Proxy support
- Custom metadata configuration

## Success Metrics

### Usability
- First successful download within 3 minutes for new users
- UI clarity score >= 4.5/5.0
- Error rate < 1%

### Stability
- Download success rate >= 99%
- Automatic retry on network failure
- 8+ hours of crash-free continuous operation

### Performance
- Metadata extraction within 5 seconds
- Download speed bounded by ISP bandwidth
- Memory usage under 500 MB

---

Document Date: 2026-03-01  
Author: MoAI Documentation System  
Version: 1.0.0
