"""Chatterbox TTS model implementation."""

import io
import os
from typing import Any

import soundfile as sf

from .base import TTSModel


class ChatterboxModel(TTSModel):
    """Chatterbox TTS from Resemble AI.

    Supports voice cloning from reference audio, expressiveness control,
    and GPU acceleration (CUDA, MPS, CPU).
    """

    def __init__(self, device: str | None = None):
        """Initialize Chatterbox model.

        Args:
            device: Device to use ("cuda", "mps", "cpu", or None for auto-detect)
        """
        self._device = device or self._detect_device()
        self._model = None
        self._sample_rate = 24000

    @property
    def name(self) -> str:
        return "chatterbox"

    @property
    def sample_rate(self) -> int:
        return self._sample_rate

    @property
    def device(self) -> str:
        return self._device

    @property
    def is_loaded(self) -> bool:
        return self._model is not None

    def _detect_device(self) -> str:
        """Auto-detect the best available device."""
        try:
            import torch

            # Check MPS (Apple Silicon)
            if hasattr(torch.backends, "mps") and torch.backends.mps.is_available():
                return "mps"

            # Check CUDA
            if torch.cuda.is_available():
                return "cuda"
        except ImportError:
            pass

        return "cpu"

    def load(self) -> None:
        """Load the Chatterbox model."""
        if self._model is not None:
            return

        # Enable MPS fallback for unsupported operations
        os.environ["PYTORCH_ENABLE_MPS_FALLBACK"] = "1"

        from chatterbox.tts import ChatterboxTTS

        self._model = ChatterboxTTS.from_pretrained(device=self._device)
        self._sample_rate = self._model.sr

    def unload(self) -> None:
        """Unload the model from memory."""
        if self._model is not None:
            del self._model
            self._model = None
            self.cleanup_memory()

    def synthesize(self, text: str, **options) -> bytes:
        """Synthesize text to WAV audio bytes.

        Args:
            text: Text to synthesize
            **options: Chatterbox-specific options:
                - voice_ref: Path to voice reference audio for cloning
                - exaggeration: Expressiveness (0.25-2.0, default 0.5)
                - cfg: Pacing/CFG weight (0.0-1.0, default 0.5)
                - temperature: Randomness (0.05-5.0, default 0.8)

        Returns:
            WAV audio data as bytes
        """
        if not self.is_loaded:
            self.load()

        # Extract and clamp options
        voice_ref = options.get("voice_ref")
        exaggeration = max(0.25, min(2.0, options.get("exaggeration", 0.5)))
        cfg = max(0.0, min(1.0, options.get("cfg", 0.5)))
        temperature = max(0.05, min(5.0, options.get("temperature", 0.8)))

        # Generate audio
        gen_kwargs: dict[str, Any] = {
            "text": text,
            "exaggeration": exaggeration,
            "cfg_weight": cfg,
            "temperature": temperature,
        }

        if voice_ref:
            gen_kwargs["audio_prompt_path"] = voice_ref

        wav = self._model.generate(**gen_kwargs)

        # Convert tensor to numpy
        wav_np = wav.cpu().numpy()

        # Handle dimensions - soundfile expects (samples,) or (samples, channels)
        if wav_np.ndim == 2:
            wav_np = wav_np.T  # (channels, samples) -> (samples, channels)

        # Write to bytes buffer
        buffer = io.BytesIO()
        sf.write(buffer, wav_np, self._sample_rate, format="WAV")
        buffer.seek(0)

        # Cleanup GPU memory
        self.cleanup_memory()

        return buffer.read()
