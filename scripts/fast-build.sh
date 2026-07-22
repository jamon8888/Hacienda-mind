#!/bin/bash
# Fast build script for hacienda-mcp - optimized for local development

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

echo "=== Fast Build for hacienda-mcp ==="

# Apply critical performance optimizations
export CARGO_INCREMENTAL=2
export CARGO_DEPS_DEBUGINFO=0
export CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse
export CARGO_TARGET_DIR="$ROOT_DIR/target-fast"

# Determine optimal job count
export BUILD_JOBS=$(nproc)
echo "Using $BUILD_JOBS parallel jobs"

# Determine features from environment or default
FEATURES="${FEATURES:-full}"
echo "Building with features: $FEATURES"

# Function to monitor build performance
monitor_build() {
    local start_time=$(date +%s)
    local cmd="$1"
    local output_file="$2"
    
    echo "Building... ($start_time)"
    
    if $cmd > "$output_file" 2>&1; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        local speed=$(echo "scale=2; $BUILD_JOBS * 1000 / $duration" | bc 2>/dev/null || echo "N/A")
        
        echo "Build completed in ${duration}s ($speed job/sec)"
        echo "Output: $output_file"
        return 0
    else
        local exit_code=$?
        echo "Build failed with exit code: $exit_code"
        cat "$output_file" | tail -20
        return $exit_code
    fi
}

# Clean previous builds if requested
if [[ "${1:-}" == "clean" ]]; then
    echo "Cleaning previous builds..."
    rm -rf "$ROOT_DIR/target" "$ROOT_DIR/target-fast"
fi

# Create separate build directory
mkdir -p "$ROOT_DIR/target-fast"

case "${FEATURES}" in
    "docs")
        # For docs: faster, lighter build
        monitor_build \
            "cargo build --quiet --bin hacienda-mcp --features documents --jobs $BUILD_JOBS --target-dir \"$ROOT_DIR/target-fast\"" \
            "$ROOT_DIR/build-docs.log"
        ;;
    
    "code")
        # For code-intel: optimized build
        monitor_build \
            "cargo build --release --quiet --bin hacienda-mcp --features code-intel --jobs $BUILD_JOBS --target-dir \"$ROOT_DIR/target-fast\"" \
            "$ROOT_DIR/build-code.log"
        ;;
    
    "full")
        # For full feature set - this is the main optimization target
        echo "Building with 'full' features - this may take 10-15 minutes initially..."
        echo "First build will be slower (compiling all dependencies), subsequent builds much faster."
        monitor_build \
            "cargo build --release --quiet --bin hacienda-mcp --features full --jobs $BUILD_JOBS --target-dir \"$ROOT_DIR/target-fast\"" \
            "$ROOT_DIR/build-full.log"
        ;;
    
    "test")
        # For testing: balanced build
        monitor_build \
            "cargo test --quiet --features full --jobs $BUILD_JOBS --target-dir \"$ROOT_DIR/target-fast\"" \
            "$ROOT_DIR/build-test.log"
        ;;
    
    *)
        # Default build with optimizations
        echo "Building with default features - optimized for speed..."
        monitor_build \
            "cargo build --release --quiet --bin hacienda-mcp --features \"intelligence,code-intel\" --jobs $BUILD_JOBS --target-dir \"$ROOT_DIR/target-fast\"" \
            "$ROOT_DIR/build-default.log"
        ;;
esac

echo "=== Build Complete ==="