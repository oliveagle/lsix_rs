# justfile for lsix_rs

# Default action: Build for current platform
default: build

# Build for the current platform (macOS)
build:
    @echo "Building for current platform..."
    cargo build --release

# Build for Linux x86_64 (using musl for static linking)
build-linux:
    @echo "Building for Linux x86_64 (static)..."
    cargo build --release --target x86_64-unknown-linux-musl

# Build both platforms
build-all: build build-linux build-linux-arm64
    @echo "All builds completed."

# Build for Linux ARM64 (using musl for static linking)
build-linux-arm64:
    @echo "Building for Linux ARM64 (static)..."
    cargo build --release --target aarch64-unknown-linux-musl

# Clean build artifacts
clean:
    cargo clean

# Run the local build
run *args:
    cargo run --release -- {{args}}
