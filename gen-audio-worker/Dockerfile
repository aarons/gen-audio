# Gen-audio TTS Worker
# Multi-stage build with CUDA support for GPU acceleration
# Includes SSH server for coordinator access

# Stage 1: Builder
FROM python:3.11-slim as builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    git \
    && rm -rf /var/lib/apt/lists/*

# Copy project files
COPY pyproject.toml .
COPY src/ src/

# Build wheel
RUN pip install --no-cache-dir build && \
    python -m build --wheel

# Stage 2: Runtime with CUDA
FROM nvidia/cuda:12.1.1-cudnn8-runtime-ubuntu22.04

# Install Python, audio libraries, and SSH server
RUN apt-get update && apt-get install -y --no-install-recommends \
    python3.11 \
    python3.11-venv \
    python3-pip \
    libsndfile1 \
    ffmpeg \
    openssh-server \
    && rm -rf /var/lib/apt/lists/* \
    && ln -s /usr/bin/python3.11 /usr/bin/python

# Configure SSH
RUN mkdir -p /var/run/sshd /root/.ssh \
    && chmod 700 /root/.ssh \
    && sed -i 's/#PermitRootLogin prohibit-password/PermitRootLogin prohibit-password/' /etc/ssh/sshd_config \
    && sed -i 's/#PasswordAuthentication yes/PasswordAuthentication no/' /etc/ssh/sshd_config \
    && ssh-keygen -A

WORKDIR /app

# Install PyTorch with CUDA support first (large download)
RUN pip install --no-cache-dir \
    torch==2.1.2+cu121 \
    torchaudio==2.1.2+cu121 \
    --index-url https://download.pytorch.org/whl/cu121

# Copy and install the wheel from builder
COPY --from=builder /build/dist/*.whl /tmp/
RUN pip install --no-cache-dir /tmp/*.whl[chatterbox] && rm /tmp/*.whl

# Create worker directories
RUN mkdir -p /root/.gen-audio/worker/voices /root/.gen-audio/worker/output

# Copy entrypoint script
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# Expose SSH port
EXPOSE 22

# Health check via CLI status command
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD gen-audio-worker status | python -c "import sys, json; d=json.load(sys.stdin); sys.exit(0 if d.get('ready') else 1)"

ENTRYPOINT ["/entrypoint.sh"]
