# Changelog

## [0.3.0] - 2025-11-30

### Added
- Per-program default model selection allowing gc, ask, and bookname to each use different default presets while sharing the same config file

## [0.2.0] - 2025-11-28

### Changed
- Replaced `genai` dependency with direct `reqwest` HTTP calls for lighter dependency footprint
- Consolidated `OpenRouterProvider` and `CerebrasProvider` into single `OpenAICompatibleProvider`
- Moved Claude CLI availability check into constructor (fail-fast on missing CLI)
- Changed config file location from `~/.config/gc/config.toml` to `~/.config/cli-programs/llm.toml`

### Removed
- Removed unused `temperature` and `max_tokens` fields from `ModelPreset`
- Removed `genai` crate dependency

## [0.1.0] - 2025-11-25

### Added
- Initial release of llm-client shared library
- LlmProvider trait for unified LLM interface
- Claude CLI provider (subprocess-based, default)
- Anthropic API provider
- OpenRouter provider for multi-model access
- Cerebras provider for fast Llama inference
- Configuration system with TOML file support
- Model presets for quick provider/model selection
