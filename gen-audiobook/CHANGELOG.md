# Changelog

## [0.2.0] - 2024-12-18

### Added

- **Self-bootstrapping**: Automatically downloads and manages all dependencies on first run
  - Portable Python 3.11 from python-build-standalone
  - Static FFmpeg/FFprobe binaries
  - Chatterbox TTS with PyTorch
- `gen-audio uninstall` command to cleanly remove all dependencies
  - `--include-models` flag to also remove HuggingFace model cache
- `gen-audio setup --clean` to remove and re-download dependencies
- Progress bars during dependency downloads
- Platform detection for macOS (arm64, x86_64) and Linux (x86_64, arm64)

### Changed

- No longer requires system Python or FFmpeg installation
- All gen-audio-managed dependencies now stored in `~/.local/share/gen-audio/`
- Improved first-run user experience with confirmation prompt

### Fixed

- FFmpeg/FFprobe now use bootstrapped versions instead of system versions

## [0.1.0] - 2024-12-14

### Added

- Initial release
- EPUB parsing and text extraction
- Chatterbox TTS backend with voice cloning
- M4B audiobook output with chapter markers
- GPU acceleration (MPS, CUDA, CPU)
- Resumable sessions
- Configuration management
- TTS parameter controls (exaggeration, cfg, temperature)
