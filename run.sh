#!/usr/bin/env bash
# steM. — Application Launcher Script
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV_DIR="$SCRIPT_DIR/.venv"

if [ -f "$VENV_DIR/bin/python3" ]; then
    PYTHON_BIN="$VENV_DIR/bin/python3"
else
    PYTHON_BIN="$(which python3)"
fi

export PYTHONPATH="$SCRIPT_DIR:$PYTHONPATH"
export GSETTINGS_SCHEMA_DIR="$SCRIPT_DIR/data:$GSETTINGS_SCHEMA_DIR"

exec "$PYTHON_BIN" -m stem.app "$@"
