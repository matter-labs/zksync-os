#!/bin/bash

# URLs of the tarballs
DEVELOP_TAR_URL="https://github.com/ethereum/execution-spec-tests/releases/download/v4.5.0/fixtures_develop.tar.gz"
STABLE_TAR_URL="https://github.com/ethereum/execution-spec-tests/releases/download/v4.5.0/fixtures_stable.tar.gz"
STATIC_TAR_URL="https://github.com/ethereum/execution-spec-tests/releases/download/v4.5.0/fixtures_static.tar.gz"

# Target directory
TARGET_DIR="ethereum-fixtures"

# Create the target directory
mkdir -p "$TARGET_DIR"

DEVELOP_TARGET_DIR="develop"
STABLE_TARGET_DIR="stable"
STATIC_TARGET_DIR="static"

# Create the directories
mkdir -p "$TARGET_DIR/$DEVELOP_TARGET_DIR"
mkdir -p "$TARGET_DIR/$STABLE_TARGET_DIR"
mkdir -p "$TARGET_DIR/$STATIC_TARGET_DIR"

# Download and extract, stripping the top-level "fixtures" directory
curl -L "$DEVELOP_TAR_URL" | tar -xz --strip-components=1 -C "$TARGET_DIR/$DEVELOP_TARGET_DIR"
curl -L "$STABLE_TAR_URL" | tar -xz --strip-components=1 -C "$TARGET_DIR/$STABLE_TARGET_DIR"
curl -L "$STATIC_TAR_URL" | tar -xz --strip-components=1 -C "$TARGET_DIR/$STATIC_TARGET_DIR"

echo "Download and extraction complete into '$TARGET_DIR'."
