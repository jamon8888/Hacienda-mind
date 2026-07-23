#!/bin/bash
# Quick performance check script for hacienda-mcp

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

check_build_performance() {
    echo "=== hacienda-mcp Build Performance Check ==="
    echo

    # Check current configuration
    echo "📊 Current Build Configuration:"
    local jobs=$(cargo metadata --quiet 2>/dev/null | grep -o '"jobs": [0-9]*' | cut -d' ' -f2 || echo "Auto (0)")
    echo "  Jobs: $jobs"
    echo "  Incremental: $(grep -A5 '\[profile.dev\]' Cargo.toml | grep 'incremental' | cut -d'=' -f2 | tr -d ' \"')"
    echo "  Release opt-level: $(grep -A5 '\[profile.release\]' Cargo.toml | grep 'opt-level' | cut -d'=' -f2 | tr -d ' \"')"
    echo

    # Check system resources
    echo "🖥️ System Resources:"
    echo "  CPU cores: $(nproc)"
    if [[ -f "/proc/meminfo" ]]; then
        local mem_total=$(grep MemTotal /proc/meminfo | awk '{print $2}')
        local mem_gb=$((mem_total / 1024 / 1024))
        echo "  RAM: ${mem_gb}GB"
    fi
    echo

    # Check existing builds
    echo "📦 Existing Builds:"
    local build_dirs=(
        "$ROOT_DIR/target/release/hacienda-mcp"
        "$ROOT_DIR/target-fast/release/hacienda-mcp"
        "$HOME/.cargo/target/release/hacienda-mcp"
    )

    for dir in "${build_dirs[@]}"; do
        if [[ -f "$dir" ]]; then
            local size=$(du -h "$dir" 2>/dev/null | cut -f1 || echo "Unknown")
            echo "  ✅ $dir ($size)"
        else
            echo "  ❌ $dir (not found)"
        fi
    done

    echo

    # Check Cargo cache
    if [[ -d "$HOME/.cargo/registry/cache" ]]; then
        local cache_size=$(du -sh "$HOME/.cargo/registry/cache" 2>/dev/null | cut -f1)
        echo "🗃️ Cargo Registry Cache: $cache_size"
    fi

    echo

    # Performance recommendations
    echo "💡 Performance Recommendations:"
    echo "  1. Run './scripts/fast-build.sh' for optimized builds"
    echo "  2. Use './scripts/fast-build.sh clean' to reset cache"
    echo "  3. Set FEATURES=\"documents\" for lighter builds"
    echo "  4. Set FEATURES=\"code-intel\" for code intelligence only"
    echo "  5. Add current configs to .cargo/config.toml for all builds"
    echo

    # Generate optimization score
    local score=100

    # Deduct points for suboptimal configurations
    if ! grep -q "sparse" "$ROOT_DIR/.cargo/config.toml" 2>/dev/null; then
        echo "  ⚠️  Add 'protocol = \"sparse\"' to .cargo/config.toml"
        score=$((score - 20))
    fi

    if [[ ! -f "$ROOT_DIR/target/release/hacienda-mcp" ]] && \
       [[ ! -f "$ROOT_DIR/target-fast/release/hacienda-mcp" ]]; then
        echo "  ⚠️  No existing builds found"
        score=$((score - 10))
    fi

    echo "📈 Build Performance Score: $score/100"

    if [[ $score -ge 80 ]]; then
        echo "  🎉 Build configuration is well optimized!"
    elif [[ $score -ge 60 ]]; then
        echo "  ✅ Build configuration is mostly optimized"
    else
        echo "  ⚠️  Build configuration needs attention"
    fi
}

check_build_performance