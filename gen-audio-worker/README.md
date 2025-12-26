# gen-audio-worker

Python TTS worker for gen-audio. Communicates via SSH stdin/stdout (no HTTP server).

## Quick Start

### Installation on GPU machine (vast.ai, etc.)

```bash
# Install with Chatterbox
pip install gen-audio-worker[chatterbox]
```

### Check Status

```bash
gen-audio-worker status
```

Output:
```json
{
  "ready": true,
  "device": "cuda",
  "gen_audio_version": "0.1.0",
  "chatterbox_installed": true,
  "jobs_in_progress": 0,
  "available_disk_mb": 50000
}
```

### Run a Job

```bash
echo '{"version":1,"job_id":"test_001","session_id":"sess","chapter_id":0,"chunk_id":0,"text":"Hello world.","options":{},"created_at":"2024-01-01T00:00:00Z"}' | gen-audio-worker run
```

Output:
```json
{
  "version": 1,
  "job_id": "test_001",
  "status": "completed",
  "duration_ms": 1234,
  "audio_size_bytes": 56789,
  "audio_path": "/home/user/.gen-audio/worker/output/test_001.wav",
  "error": null,
  "completed_at": "2024-01-01T00:00:01Z"
}
```

## Protocol

### Job Input (TtsJob)

```json
{
  "version": 1,
  "job_id": "session_ch001_ck0042",
  "session_id": "abc123",
  "chapter_id": 1,
  "chunk_id": 42,
  "text": "Hello world.",
  "options": {
    "exaggeration": 0.5,
    "cfg": 0.5,
    "temperature": 0.8,
    "voice_ref_hash": "abc123..."
  },
  "created_at": "2024-01-01T00:00:00Z"
}
```

### Job Output (TtsResult)

```json
{
  "version": 1,
  "job_id": "session_ch001_ck0042",
  "status": "completed",
  "duration_ms": 1234,
  "audio_size_bytes": 56789,
  "audio_path": "~/.gen-audio/worker/output/session_ch001_ck0042.wav",
  "error": null,
  "completed_at": "2024-01-01T00:00:01Z"
}
```

## File Locations

- Voice references: `~/.gen-audio/worker/voices/{hash}.wav`
- Output audio: `~/.gen-audio/worker/output/{job_id}.wav`

## Integration with gen-audiobook

The Rust coordinator (`gen-audiobook`) communicates with workers via SSH:

1. **Status check**: `ssh worker "gen-audio-worker status"`
2. **Voice upload**: SFTP to `~/.gen-audio/worker/voices/{hash}.wav`
3. **Job execution**: `ssh worker "gen-audio-worker run" < job.json`
4. **Audio download**: SFTP from `~/.gen-audio/worker/output/{job_id}.wav`
5. **Cleanup**: SSH `rm` of output files

## Deployment on vast.ai

1. SSH to your vast.ai instance
2. Install the worker:
   ```bash
   pip install gen-audio-worker[chatterbox]
   ```
3. Configure the coordinator with your worker:
   ```bash
   gen-audio workers add vast-gpu <vast-ip> -u root -k ~/.ssh/id_rsa
   ```
4. Run conversions - the coordinator handles everything via SSH

## Supported Models

- **chatterbox**: High-quality voice cloning (Resemble AI)
- **kokoro**: Coming soon
- **xtts-v2**: Coming soon
- **bark**: Coming soon
