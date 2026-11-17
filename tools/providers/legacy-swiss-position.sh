#!/usr/bin/env sh
# Development-only adapter wrapper; no Swiss code distributed here.
exec node "$(dirname "$0")/legacy-swiss-position.cjs"
