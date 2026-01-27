#!/bin/bash
set -euo pipefail

# Build script for Fiberlamp macOS screensaver
# Creates a .saver bundle (universal if cross-compilation targets available)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="$PROJECT_ROOT/target/macos-screensaver"
BUNDLE_DIR="$BUILD_DIR/Fiberlamp.saver"

echo "=== Building Fiberlamp macOS Screensaver ==="
echo "Project root: $PROJECT_ROOT"
echo "Build dir: $BUILD_DIR"

# Clean previous build
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"

# Detect native architecture
NATIVE_ARCH=$(uname -m)
echo "Native architecture: $NATIVE_ARCH"

# Check if we can do cross-compilation (rustup with both targets)
CAN_CROSS_COMPILE=false
if command -v rustup &> /dev/null; then
    INSTALLED_TARGETS=$(rustup target list --installed 2>/dev/null || echo "")
    if echo "$INSTALLED_TARGETS" | grep -q "x86_64-apple-darwin" && \
       echo "$INSTALLED_TARGETS" | grep -q "aarch64-apple-darwin"; then
        CAN_CROSS_COMPILE=true
    fi
fi

if [ "$CAN_CROSS_COMPILE" = true ]; then
    echo "Cross-compilation available, building universal binary"

    # Build for both architectures
    echo ""
    echo "=== Building Rust staticlib (x86_64) ==="
    cargo build --release --lib --features macos-screensaver --target x86_64-apple-darwin

    echo ""
    echo "=== Building Rust staticlib (arm64) ==="
    cargo build --release --lib --features macos-screensaver --target aarch64-apple-darwin

    # Create universal binary with lipo
    echo ""
    echo "=== Creating universal Rust library ==="
    RUST_LIB_X86="$PROJECT_ROOT/target/x86_64-apple-darwin/release/libfiberlamp.a"
    RUST_LIB_ARM="$PROJECT_ROOT/target/aarch64-apple-darwin/release/libfiberlamp.a"
    RUST_LIB_UNIVERSAL="$BUILD_DIR/libfiberlamp.a"

    lipo -create "$RUST_LIB_X86" "$RUST_LIB_ARM" -output "$RUST_LIB_UNIVERSAL"

    CLANG_ARCH_FLAGS="-arch x86_64 -arch arm64"
else
    echo "Cross-compilation not available, building for native architecture only"

    # Build for native architecture
    echo ""
    echo "=== Building Rust staticlib (native) ==="
    cargo build --release --lib --features macos-screensaver

    RUST_LIB_UNIVERSAL="$PROJECT_ROOT/target/release/libfiberlamp.a"

    if [ "$NATIVE_ARCH" = "arm64" ]; then
        CLANG_ARCH_FLAGS="-arch arm64"
    else
        CLANG_ARCH_FLAGS="-arch x86_64"
    fi
fi

echo "Rust library: $RUST_LIB_UNIVERSAL"

# Compile Objective-C shim
echo ""
echo "=== Compiling Objective-C shim ==="
OBJ_FILE="$BUILD_DIR/FiberlampView.o"

clang -c \
    $CLANG_ARCH_FLAGS \
    -fobjc-arc \
    -fmodules \
    -mmacosx-version-min=11.0 \
    -isysroot "$(xcrun --show-sdk-path)" \
    -I"$SCRIPT_DIR" \
    "$SCRIPT_DIR/FiberlampView.m" \
    -o "$OBJ_FILE"

echo "Created: $OBJ_FILE"

# Link into bundle executable
echo ""
echo "=== Linking screensaver bundle ==="
EXECUTABLE="$BUILD_DIR/Fiberlamp"

clang \
    $CLANG_ARCH_FLAGS \
    -bundle \
    -fobjc-arc \
    -mmacosx-version-min=11.0 \
    -isysroot "$(xcrun --show-sdk-path)" \
    -framework ScreenSaver \
    -framework Cocoa \
    -framework Metal \
    -framework QuartzCore \
    -framework IOKit \
    -framework IOSurface \
    -framework CoreGraphics \
    "$OBJ_FILE" \
    "$RUST_LIB_UNIVERSAL" \
    -o "$EXECUTABLE"

echo "Created: $EXECUTABLE"

# Assemble .saver bundle structure
echo ""
echo "=== Assembling bundle structure ==="
mkdir -p "$BUNDLE_DIR/Contents/MacOS"
mkdir -p "$BUNDLE_DIR/Contents/Resources"

cp "$EXECUTABLE" "$BUNDLE_DIR/Contents/MacOS/Fiberlamp"
cp "$SCRIPT_DIR/Info.plist" "$BUNDLE_DIR/Contents/"

# Create PkgInfo
echo -n "BNDL????" > "$BUNDLE_DIR/Contents/PkgInfo"

echo "Bundle created: $BUNDLE_DIR"

# Ad-hoc codesign
echo ""
echo "=== Code signing ==="
codesign --force --deep --sign - "$BUNDLE_DIR"
echo "Signed: $BUNDLE_DIR"

# Verify
echo ""
echo "=== Verification ==="
codesign --verify --verbose "$BUNDLE_DIR"

# Show architecture info
echo ""
echo "=== Architecture info ==="
lipo -info "$BUNDLE_DIR/Contents/MacOS/Fiberlamp"

echo ""
echo "=== Build complete ==="
echo "Output: $BUNDLE_DIR"
echo ""
echo "To install:"
echo "  cp -R \"$BUNDLE_DIR\" ~/Library/Screen\\ Savers/"
echo ""
echo "Then open System Settings > Screen Saver to select Fiberlamp"
