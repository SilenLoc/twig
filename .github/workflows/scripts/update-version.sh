#!/usr/bin/env bash
# Update Cargo.toml version for semantic-release
set -e

if [ -z "$1" ]; then
  echo "Usage: $0 <version>"
  exit 1
fi

VERSION=$1

# Update Cargo.toml
sed -i "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml

# Update Cargo.lock to reflect the new version (only update fig package)
cargo update -p fig

echo "Updated Cargo.toml and Cargo.lock to version $VERSION"
