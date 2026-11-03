#!/usr/bin/env bash
# Commit generated styles alongside the dependencies that produce them.
set -euo pipefail
cd "$(dirname "$0")/../.."
if git diff --quiet HEAD -- tools/site/package.json tools/site/package-lock.json; then
  exit 0
fi
npm ci --ignore-scripts --no-audit --no-fund --prefix tools/site
npm run build:landing --prefix tools/site
