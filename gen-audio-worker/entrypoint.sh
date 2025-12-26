#!/bin/bash
set -e

# Start SSH daemon
/usr/sbin/sshd

# Print status
echo "========================================"
echo "  gen-audio-worker ready"
echo "========================================"
echo ""
echo "SSH listening on port 22"
echo ""
echo "Add this worker to your coordinator:"
echo "  ./gen-audio workers add local <host> -p <port>"
echo ""
echo "Worker status:"
gen-audio-worker status
echo ""

# Keep container running
exec tail -f /dev/null
