#!/bin/bash
# snapshot-db.sh - Copy production database to staging
#
# Usage: ./scripts/snapshot-db.sh
#
# Prerequisites:
#   - flyctl installed and authenticated
#   - Access to both manifest (prod) and manifest-staging apps

set -euo pipefail

PROD_APP="manifest"
STAGING_APP="manifest-staging"
DB_PATH="/data/manifest.db"
TEMP_FILE="/tmp/manifest-snapshot.db"

echo "=== Manifest Database Snapshot ==="
echo "Source: $PROD_APP"
echo "Target: $STAGING_APP"
echo ""

# Step 1: Download from production
echo "[1/4] Downloading database from production..."
flyctl ssh sftp get "$DB_PATH" "$TEMP_FILE" --app "$PROD_APP"

# Step 2: Stop staging to prevent writes
echo "[2/4] Stopping staging app..."
flyctl scale count 0 --app "$STAGING_APP" --yes

# Wait for app to stop
sleep 5

# Step 3: Upload to staging
echo "[3/4] Uploading database to staging..."
flyctl ssh sftp shell --app "$STAGING_APP" <<EOF
put $TEMP_FILE $DB_PATH
EOF

# Step 4: Restart staging
echo "[4/4] Restarting staging app..."
flyctl scale count 1 --app "$STAGING_APP" --yes

# Cleanup
rm -f "$TEMP_FILE"

echo ""
echo "=== Snapshot complete ==="
echo "Staging now has a copy of production data."
