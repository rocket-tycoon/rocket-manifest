#!/bin/bash
# Usage: ./scripts/bump-version.sh 0.2.0
# Then: git tag v0.2.0 && git push && git push --tags

set -e

if [ -z "$1" ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.2.0"
    exit 1
fi

VERSION=$1

# Update Cargo.toml
sed -i.bak "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
rm -f Cargo.toml.bak

# Update Cargo.lock
cargo check

echo "Version bumped to $VERSION"
echo ""
echo "Next steps:"
echo "  git add Cargo.toml Cargo.lock"
echo "  git commit -m \"Bump version to $VERSION\""
echo "  git tag v$VERSION"
echo "  git push && git push --tags"
