#!/bin/bash
# Remove set -e to prevent exit on error

# Print system info
echo "μDCN Server Container"
echo "====================="
echo "System: $(uname -a)"
echo "Date: $(date)"
echo "Network interfaces:"
ip addr

# Execute the command but don't fail if it returns non-zero
"$@" || true

# Keep the container running
echo "Keeping server container alive for client connections and metrics collection..."
exec sleep infinity
