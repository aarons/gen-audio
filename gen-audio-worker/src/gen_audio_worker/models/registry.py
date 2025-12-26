"""Model registry for managing TTS model instances."""

from typing import Type
from .base import TTSModel


class ModelRegistry:
    """Registry for TTS models with lazy loading and caching."""

    _models: dict[str, Type[TTSModel]] = {}
    _instances: dict[str, TTSModel] = {}

    @classmethod
    def register(cls, name: str, model_class: Type[TTSModel]) -> None:
        """Register a model class."""
        cls._models[name] = model_class

    @classmethod
    def get(cls, name: str, **kwargs) -> TTSModel:
        """Get a model instance, loading if necessary."""
        if name not in cls._instances:
            if name not in cls._models:
                raise ValueError(f"Unknown model: {name}. Available: {list(cls._models.keys())}")
            cls._instances[name] = cls._models[name](**kwargs)
        return cls._instances[name]

    @classmethod
    def list_available(cls) -> list[str]:
        """List available model names."""
        return list(cls._models.keys())

    @classmethod
    def list_loaded(cls) -> list[str]:
        """List currently loaded model names."""
        return [name for name, model in cls._instances.items() if model.is_loaded]

    @classmethod
    def unload(cls, name: str) -> None:
        """Unload a specific model."""
        if name in cls._instances:
            cls._instances[name].unload()
            del cls._instances[name]

    @classmethod
    def unload_all(cls) -> None:
        """Unload all models."""
        for model in cls._instances.values():
            model.unload()
        cls._instances.clear()


def get_model(name: str, **kwargs) -> TTSModel:
    """Get a model instance by name."""
    return ModelRegistry.get(name, **kwargs)


def list_models() -> list[str]:
    """List available model names."""
    return ModelRegistry.list_available()


# Auto-register available models
def _register_available_models():
    """Register models that are available (dependencies installed)."""
    try:
        from .chatterbox import ChatterboxModel

        ModelRegistry.register("chatterbox", ChatterboxModel)
    except ImportError:
        pass

    # Future models can be registered here
    # try:
    #     from .kokoro import KokoroModel
    #     ModelRegistry.register("kokoro", KokoroModel)
    # except ImportError:
    #     pass


_register_available_models()
