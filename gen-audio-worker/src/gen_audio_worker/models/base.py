"""Base TTS model interface."""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any

# Protocol version - must match Rust coordinator
PROTOCOL_VERSION = 1


@dataclass
class TtsJobOptions:
    """TTS synthesis options matching Rust TtsJobOptions."""

    exaggeration: float = 0.5
    cfg: float = 0.5
    temperature: float = 0.8
    voice_ref_hash: str | None = None

    @classmethod
    def from_dict(cls, data: dict) -> "TtsJobOptions":
        return cls(
            exaggeration=data.get("exaggeration", 0.5),
            cfg=data.get("cfg", 0.5),
            temperature=data.get("temperature", 0.8),
            voice_ref_hash=data.get("voice_ref_hash"),
        )


@dataclass
class TtsJob:
    """TTS job matching Rust TtsJob protocol."""

    version: int
    job_id: str
    session_id: str
    chapter_id: int
    chunk_id: int
    text: str
    options: TtsJobOptions
    created_at: str

    @classmethod
    def from_dict(cls, data: dict) -> "TtsJob":
        return cls(
            version=data.get("version", PROTOCOL_VERSION),
            job_id=data["job_id"],
            session_id=data.get("session_id", ""),
            chapter_id=data.get("chapter_id", 0),
            chunk_id=data.get("chunk_id", 0),
            text=data["text"],
            options=TtsJobOptions.from_dict(data.get("options", {})),
            created_at=data.get("created_at", datetime.now(timezone.utc).isoformat()),
        )


@dataclass
class TtsResult:
    """TTS result matching Rust TtsResult protocol."""

    version: int
    job_id: str
    status: str  # "completed", "failed", or "timeout"
    duration_ms: int | None = None
    audio_size_bytes: int | None = None
    audio_path: str | None = None
    error: str | None = None
    completed_at: str = field(default_factory=lambda: datetime.now(timezone.utc).isoformat())

    def to_dict(self) -> dict:
        return {
            "version": self.version,
            "job_id": self.job_id,
            "status": self.status,
            "duration_ms": self.duration_ms,
            "audio_size_bytes": self.audio_size_bytes,
            "audio_path": self.audio_path,
            "error": self.error,
            "completed_at": self.completed_at,
        }

    @classmethod
    def success(
        cls,
        job_id: str,
        duration_ms: int,
        audio_size_bytes: int,
        audio_path: str,
    ) -> "TtsResult":
        return cls(
            version=PROTOCOL_VERSION,
            job_id=job_id,
            status="completed",
            duration_ms=duration_ms,
            audio_size_bytes=audio_size_bytes,
            audio_path=audio_path,
        )

    @classmethod
    def failure(cls, job_id: str, error: str) -> "TtsResult":
        return cls(
            version=PROTOCOL_VERSION,
            job_id=job_id,
            status="failed",
            error=error,
        )


@dataclass
class WorkerStatus:
    """Worker status matching Rust WorkerStatus protocol."""

    ready: bool
    device: str
    gen_audio_version: str
    chatterbox_installed: bool
    jobs_in_progress: int
    available_disk_mb: int

    def to_dict(self) -> dict:
        return {
            "ready": self.ready,
            "device": self.device,
            "gen_audio_version": self.gen_audio_version,
            "chatterbox_installed": self.chatterbox_installed,
            "jobs_in_progress": self.jobs_in_progress,
            "available_disk_mb": self.available_disk_mb,
        }


# Legacy aliases for backward compatibility during transition
TTSRequest = TtsJob
TTSResponse = TtsResult


class TTSModel(ABC):
    """Abstract base class for TTS models."""

    @property
    @abstractmethod
    def name(self) -> str:
        """Model name identifier."""
        ...

    @property
    @abstractmethod
    def sample_rate(self) -> int:
        """Audio sample rate."""
        ...

    @property
    def device(self) -> str:
        """Device being used (cuda, mps, cpu)."""
        return getattr(self, "_device", "cpu")

    @abstractmethod
    def load(self) -> None:
        """Load the model into memory."""
        ...

    @abstractmethod
    def unload(self) -> None:
        """Unload the model from memory."""
        ...

    @property
    @abstractmethod
    def is_loaded(self) -> bool:
        """Check if model is loaded."""
        ...

    @abstractmethod
    def synthesize(self, text: str, **options) -> bytes:
        """Synthesize text to audio bytes (WAV format).

        Args:
            text: Text to synthesize
            **options: Model-specific options

        Returns:
            WAV audio data as bytes
        """
        ...

    def cleanup_memory(self) -> None:
        """Cleanup GPU memory after synthesis."""
        import gc

        gc.collect()

        try:
            import torch

            if torch.cuda.is_available():
                torch.cuda.empty_cache()
            elif hasattr(torch, "mps") and torch.backends.mps.is_available():
                if hasattr(torch.mps, "empty_cache"):
                    torch.mps.empty_cache()
        except ImportError:
            pass
