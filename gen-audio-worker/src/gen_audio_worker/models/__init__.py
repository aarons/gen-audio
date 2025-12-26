"""TTS model implementations."""

from .base import (
    PROTOCOL_VERSION,
    TTSModel,
    TtsJob,
    TtsJobOptions,
    TtsResult,
    WorkerStatus,
    # Legacy aliases
    TTSRequest,
    TTSResponse,
)
from .registry import get_model, list_models, ModelRegistry

__all__ = [
    "PROTOCOL_VERSION",
    "TTSModel",
    "TtsJob",
    "TtsJobOptions",
    "TtsResult",
    "WorkerStatus",
    "TTSRequest",
    "TTSResponse",
    "get_model",
    "list_models",
    "ModelRegistry",
]
