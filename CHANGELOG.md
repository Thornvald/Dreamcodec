# Changelog

All notable changes to this project will be documented in this file.

## [2.2.4] - 2026-02-10

### Added
- Native Tauri window drag-and-drop event handling for reliable filesystem path detection
- Full-screen drag state overlay blocks interaction and clearly indicates active file drop mode

### Improved
- Drag/drop visuals updated to monochrome black theme with white glow animation
- README updated with current features, drag/drop behavior, and GPU diagnostics notes

## [2.2.3] - 2026-02-10

### Added
- Full-screen drag-and-drop overlay with animated file-drop state

### Improved
- NVENC path now attempts CUDA hardware decode + GPU encode for lower CPU usage
- Added runtime fallback logging when CUDA decode cannot initialize

## [2.2.2] - 2026-02-10

### Added
- CPU diagnostics in logs (model and logical core count)
- Full GPU adapter logging in UI logs, including integrated adapters and primary marker
- Preferred GPU controls now surface clearer adapter-level visibility for diagnostics

### Fixed
- Preserved identical GPU model entries so dual-GPU same-model systems are represented correctly
- Stable GPU adapter IDs now map correctly to FFmpeg NVENC `-gpu` indexes

### Improved
- Top header and side panel transparency to better expose animated star background

## [2.2.1] - 2026-02-02

### Fixed
- Audio stream detection now properly detects streams with metadata like language tags and hex IDs
- Progress percentage now displays accurate real-time progress during conversion instead of jumping from 0% to 100%
- CMD window no longer appears when checking audio streams on Windows
- Fixed parameter naming issue between frontend and backend (taskId vs task_id)
- Fixed output directory creation typo (now correctly creates "Dreamcodec Output" folder)
- Fixed crash when closing application during active conversion

### Added
- Animated spinning icon during active conversions
- Cancel button for active conversions
- "Add back to queue" button for completed conversions to allow re-conversion with different settings
- Color-coded status indicators (green for completed, red for failed)
- Color-coded GPU detection status (green when detected, red when not detected)

### Improved
- Better process cleanup and cancellation handling to prevent race conditions
- More defensive locking mechanism using try_lock to avoid deadlocks on app close
- Enhanced error handling for conversion cancellation

## [2.2.0] - 2026-02-02

### Added
- Default output directory is now automatically created on app launch

## [2.1.0] - 2026-02-01

### Major New Features
- **Auto FFmpeg Download**: FFmpeg is now downloaded automatically on first run - no manual setup required!
- **All Format Support**: Convert between any video/audio formats:
  - Video: MP4, MKV, AVI, MOV, WMV, FLV, WEBM, OGV
  - Audio: MP3, WAV, AAC, FLAC, M4A, OGG
- **Adobe/After Effects Compatible**: New professional codec support:
  - Apple ProRes (422, 422 HQ, 4444, 4444 XQ)
  - Avid DNxHD/DNxHR (220, 220x, HQ, SQ, LB)
  - GoPro CineForm (YUV, RGB)
- **Parallax Star Background**: Beautiful animated starfield from Metatron project
- **Smart GPU Detection**: Actually checks FFmpeg encoders instead of just GPU names

### UI Improvements
- Renamed app to "Dreamcodec"
- Added cool glow/highlight effects to buttons
- Input and output format selection with icons
- Encoder badges show only available options for your GPU
- FFmpeg download progress indicator
- Updated footer to "made by Thornvald"

### Technical
- Added reqwest for HTTP downloads
- Added zip extraction for FFmpeg
- Real encoder detection via `ffmpeg -encoders`
- Better format handling for all conversion types

## [2.0.0] - 2026-02-01

### Major Changes
- **Complete Rewrite**: Migrated from Python/Tkinter to Tauri 2 with React + TypeScript frontend
- **Modern UI**: New dark-themed interface with improved user experience
- **Better Performance**: Native Rust backend for faster and more reliable conversions
- **Persistent Settings**: Settings are now saved automatically between sessions

### Added
- GPU auto-detection for NVIDIA, AMD, and Intel GPUs
- Support for Intel Quick Sync Video (QSV) encoding
- Batch conversion queue with individual progress tracking
- Real-time conversion logs
- Tabbed interface for Queue, Progress, and Logs
- Drag and drop file support
- Installer packages (MSI and NSIS)

### Changed
- Replaced Tkinter GUI with modern React-based UI
- Improved error handling and user feedback
- Better progress reporting with percentage and time estimates
- Updated FFmpeg command generation for better compatibility

### Removed
- Python dependencies (GPUtil, PyInstaller)
- Standalone config.json file (now using Tauri store)

### Technical
- Frontend: React 18 + TypeScript + Tailwind CSS + Vite
- Backend: Rust + Tauri 2
- Async conversion handling with Tokio

## [1.0.0] - 2024 (Original Python Version)

### Features
- Basic MKV to MP4 conversion using FFmpeg
- GPU detection for NVIDIA and AMD
- Simple queue system for batch conversion
- Progress bar and percentage display
- Configuration persistence

### Technical
- Python 3.x with Tkinter GUI
- GPUtil for GPU detection
- PyInstaller for executable generation
