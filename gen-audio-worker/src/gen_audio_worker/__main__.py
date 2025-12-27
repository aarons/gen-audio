"""CLI entry point for gen-audio-worker."""

import argparse
import json
import os
import shutil
import sys
import time

# Worker directories
WORKER_DIR = os.path.expanduser("~/.gen-audio/worker")
VOICES_DIR = os.path.join(WORKER_DIR, "voices")
OUTPUT_DIR = os.path.join(WORKER_DIR, "output")
RESULTS_DIR = os.path.join(WORKER_DIR, "results")


def get_available_disk_mb() -> int:
    """Get available disk space in MB."""
    try:
        stat = shutil.disk_usage(WORKER_DIR if os.path.exists(WORKER_DIR) else os.path.expanduser("~"))
        return stat.free // (1024 * 1024)
    except OSError:
        return 0


def cmd_status(args):
    """Print worker status as JSON (matching Rust WorkerStatus protocol)."""
    import torch

    from .models import WorkerStatus, list_models

    # Detect device
    if torch.cuda.is_available():
        device = "cuda"
    elif hasattr(torch.backends, "mps") and torch.backends.mps.is_available():
        device = "mps"
    else:
        device = "cpu"

    # Check if chatterbox is available
    chatterbox_installed = "chatterbox" in list_models()

    status = WorkerStatus(
        ready=True,
        device=device,
        gen_audio_version="0.1.0",
        chatterbox_installed=chatterbox_installed,
        jobs_in_progress=0,
        available_disk_mb=get_available_disk_mb(),
    )

    print(json.dumps(status.to_dict()))


def _write_result(result, job_id: str):
    """Write result to file and print the path to stdout."""
    os.makedirs(RESULTS_DIR, exist_ok=True)
    result_path = os.path.join(RESULTS_DIR, f"{job_id}.json")
    with open(result_path, "w") as f:
        json.dump(result.to_dict(), f)
    print(result_path)


def cmd_run(args):
    """Run a single job from stdin (SSH/stdio mode).

    Reads TtsJob JSON from stdin, writes audio to file, writes TtsResult to file.
    Prints only the result file path to stdout (avoids library output pollution).
    """
    from .models import TtsJob, TtsResult, get_model

    # Read job from stdin
    try:
        job_data = json.load(sys.stdin)
    except json.JSONDecodeError as e:
        result = TtsResult.failure("unknown", f"Invalid JSON: {e}")
        _write_result(result, "unknown")
        sys.exit(1)

    # Parse job
    try:
        job = TtsJob.from_dict(job_data)
    except (KeyError, ValueError) as e:
        job_id = job_data.get("job_id", "unknown")
        result = TtsResult.failure(job_id, f"Invalid job: {e}")
        _write_result(result, job_id)
        sys.exit(1)

    # Ensure output directory exists
    os.makedirs(OUTPUT_DIR, exist_ok=True)

    # Run synthesis
    start_time = time.time()
    try:
        model = get_model("chatterbox")  # Currently only chatterbox is supported
        if not model.is_loaded:
            model.load()

        # Build synthesis options
        synth_options = {
            "exaggeration": job.options.exaggeration,
            "cfg": job.options.cfg,
            "temperature": job.options.temperature,
        }

        # Handle voice reference
        if job.options.voice_ref_hash:
            voice_path = os.path.join(VOICES_DIR, f"{job.options.voice_ref_hash}.wav")
            if os.path.exists(voice_path):
                synth_options["voice_ref"] = voice_path

        # Synthesize
        audio_data = model.synthesize(job.text, **synth_options)

        # Write audio to file
        audio_path = os.path.join(OUTPUT_DIR, f"{job.job_id}.wav")
        with open(audio_path, "wb") as f:
            f.write(audio_data)

        duration_ms = int((time.time() - start_time) * 1000)
        audio_size_bytes = len(audio_data)

        result = TtsResult.success(
            job_id=job.job_id,
            duration_ms=duration_ms,
            audio_size_bytes=audio_size_bytes,
            audio_path=audio_path,
        )

    except Exception as e:
        result = TtsResult.failure(job.job_id, str(e))

    _write_result(result, job.job_id)


def main():
    parser = argparse.ArgumentParser(
        prog="gen-audio-worker",
        description="TTS worker for gen-audio (SSH/SFTP mode)",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    # status command
    status_parser = subparsers.add_parser("status", help="Print worker status as JSON")
    status_parser.set_defaults(func=cmd_status)

    # run command (stdio mode)
    run_parser = subparsers.add_parser("run", help="Run single job from stdin")
    run_parser.set_defaults(func=cmd_run)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
