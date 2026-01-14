#!/bin/bash
# Release script - bumps version in all files, commits, tags, and pushes
#
# Usage: ./scripts/release.sh <version>
# Example: ./scripts/release.sh 0.1.14

set -e

VERSION="$1"

if [ -z "$VERSION" ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.1.14"
    exit 1
fi

# Validate version format (semver)
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
    echo "Error: Invalid version format. Expected semver (e.g., 0.1.14 or 0.1.14-beta.1)"
    exit 1
fi

# Check for uncommitted changes
if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "Error: Working directory has uncommitted changes. Please commit or stash them first."
    exit 1
fi

# Check we're on main branch
BRANCH=$(git branch --show-current)
if [ "$BRANCH" != "main" ]; then
    echo "Warning: Not on main branch (currently on $BRANCH)"
    read -p "Continue anyway? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Get current version from Cargo.toml
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
echo "Current version: $CURRENT_VERSION"
echo "New version: $VERSION"
echo

# Confirm
read -p "Release v$VERSION? [y/N] " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Update Cargo.toml
echo "Updating Cargo.toml..."
sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" "$PROJECT_ROOT/Cargo.toml"

# Update plugin.json
echo "Updating plugin.json..."
PLUGIN_JSON="$PROJECT_ROOT/plugins/claude-code/.claude-plugin/plugin.json"
sed -i '' "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" "$PLUGIN_JSON"

# Update rmf-wrapper.sh
echo "Updating rmf-wrapper.sh..."
WRAPPER_SH="$PROJECT_ROOT/plugins/claude-code/bin/rmf-wrapper.sh"
sed -i '' "s/^VERSION=\".*\"/VERSION=\"$VERSION\"/" "$WRAPPER_SH"

# Verify changes
echo
echo "Changes made:"
git diff --stat

# Commit
echo
echo "Committing..."
git add "$PROJECT_ROOT/Cargo.toml" "$PLUGIN_JSON" "$WRAPPER_SH"
git commit -m "Bump version to $VERSION"

# Tag
echo "Creating tag v$VERSION..."
git tag "v$VERSION"

# Push
echo "Pushing to origin..."
git push origin main
git push origin "v$VERSION"

echo
echo "Release v$VERSION initiated!"
echo "Monitor the release workflow at:"
echo "  gh run list --workflow=release.yml --limit=1"
echo
echo "Or watch it:"
echo "  gh run watch"
