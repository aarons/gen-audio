# Changelog

## [1.0.0] - 2025-12-01

### Changed
- Renamed crate from `bookname` to `bookworm`
- Per-program default model preset support via llm-client 0.3.0

## [0.1.0] - 2025-11-30

### Added
- Initial release (as `bookname`) of epub filename cleaner
- AI-powered filename cleaning using configurable LLM providers
- Support for current directory, `--dir` path, and `--recursive` modes
- Automatic conflict resolution with numeric suffixes
- `--debug` flag for verbose output
- `--model` flag to override default LLM preset
- Config subcommands for managing LLM configuration:
  - `config list` - Show available presets
  - `config show` - Display full configuration
  - `config set-default <preset>` - Change default preset
  - `config add-preset <name> -p <provider> -M <model>` - Add new preset
