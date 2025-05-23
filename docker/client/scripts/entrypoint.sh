#!/bin/bash
# Remove set -e to prevent exit on error

# Print system info
echo "μDCN Client Container"
echo "====================="
echo "System: $(uname -a)"
echo "Date: $(date)"
echo "Network interfaces:"
ip addr

# Execute the command but don't fail if it returns non-zero
"$@" || true

# Keep the container running
echo "Keeping container alive for debugging and metrics collection..."
exec sleep infinity
