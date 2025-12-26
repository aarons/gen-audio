.PHONY: all build docker run-worker run-worker-gpu stop-worker clean test help

# Default: build coordinator and docker image
all: build docker

# Build the Rust coordinator binary
build:
	cargo build --release -p gen-audiobook
	cp target/release/gen-audio .
	@echo ""
	@echo "Built: ./gen-audio"

# Build the Docker worker image (CPU version for local testing)
docker:
	docker build -t gen-audio-worker -f gen-audio-worker/Dockerfile.cpu gen-audio-worker/
	@echo ""
	@echo "Built: gen-audio-worker:latest"

# Build GPU Docker image
docker-gpu:
	docker build -t gen-audio-worker:gpu gen-audio-worker/
	@echo ""
	@echo "Built: gen-audio-worker:gpu"

# Find SSH public key (prefer ed25519, then rsa)
SSH_PUBKEY := $(shell if [ -f ~/.ssh/id_ed25519.pub ]; then echo ~/.ssh/id_ed25519.pub; elif [ -f ~/.ssh/id_rsa.pub ]; then echo ~/.ssh/id_rsa.pub; fi)

# Start local worker (CPU mode)
run-worker:
	@if [ -z "$(SSH_PUBKEY)" ]; then \
		echo "Error: No SSH public key found"; \
		echo "Generate one with: ssh-keygen -t ed25519"; \
		exit 1; \
	fi
	docker run -d --name gen-audio-worker \
		-p 2222:22 \
		-v $(SSH_PUBKEY):/root/.ssh/authorized_keys:ro \
		-v gen-audio-models:/root/.cache \
		gen-audio-worker
	@echo ""
	@echo "Worker started (CPU mode)."
	@echo ""
	@echo "Add it to gen-audio:"
	@echo "  ./gen-audio workers add local localhost -p 2222"

# Start local worker (GPU mode)
run-worker-gpu:
	@if [ -z "$(SSH_PUBKEY)" ]; then \
		echo "Error: No SSH public key found"; \
		echo "Generate one with: ssh-keygen -t ed25519"; \
		exit 1; \
	fi
	docker run -d --name gen-audio-worker \
		-p 2222:22 \
		-v $(SSH_PUBKEY):/root/.ssh/authorized_keys:ro \
		-v gen-audio-models:/root/.cache \
		--gpus all \
		gen-audio-worker:gpu
	@echo ""
	@echo "Worker started (GPU mode)."
	@echo ""
	@echo "Add it to gen-audio:"
	@echo "  ./gen-audio workers add local localhost -p 2222"

# Stop and remove the worker container
stop-worker:
	docker stop gen-audio-worker 2>/dev/null || true
	docker rm gen-audio-worker 2>/dev/null || true
	@echo "Worker stopped."

# View worker logs
logs:
	docker logs -f gen-audio-worker

# Run tests
test:
	cargo test --workspace

# Clean build artifacts
clean:
	cargo clean
	rm -f gen-audio

# Help
help:
	@echo "gen-audio build system"
	@echo ""
	@echo "Build targets:"
	@echo "  make              Build coordinator and Docker image"
	@echo "  make build        Build the coordinator (./gen-audio)"
	@echo "  make docker       Build CPU Docker image"
	@echo "  make docker-gpu   Build GPU Docker image"
	@echo ""
	@echo "Worker targets:"
	@echo "  make run-worker      Start local worker (CPU)"
	@echo "  make run-worker-gpu  Start local worker (GPU)"
	@echo "  make stop-worker     Stop the worker"
	@echo "  make logs            View worker logs"
	@echo ""
	@echo "Other:"
	@echo "  make test         Run tests"
	@echo "  make clean        Clean build artifacts"
	@echo "  make help         Show this help"
