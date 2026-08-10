#!/usr/bin/env bash
# story: BUG-2026-07-04-python-interpreter-fragility
# Resolve a Python interpreter that is guaranteed to have PyYAML
# and other requirements.txt dependencies.
# Source this script, then use $PYTHON instead of bare python3.
#
# Resolution order:
#   1. .venv/bin/python3 (POSIX project virtualenv)
#   2. .venv/Scripts/python.exe (Windows project virtualenv)
#   3. python3 on PATH
#   4. python on PATH, only when it reports Python major version 3

python_env_has_pyyaml() {
  "$1" -c "import yaml" >/dev/null 2>&1
}

python_env_major_version() {
  "$1" -c "import sys; print(sys.version_info[0])" 2>/dev/null || true
}

python_env_no_python() {
  echo "python-env: ERROR — no usable Python 3 interpreter found" >&2
  echo "  Tried: $1, $2, python3 on PATH, python on PATH" >&2
  echo "  Install Python 3 or put python3 on PATH." >&2
}

resolve_python() {
  local script_dir posix_venv windows_venv path_python3 path_python python_major python_source
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  posix_venv="$script_dir/.venv/bin/python3"
  windows_venv="$script_dir/.venv/Scripts/python.exe"
  path_python3="$(command -v python3 2>/dev/null || true)"
  path_python="$(command -v python 2>/dev/null || true)"
  python_source=""

  if [ -x "$posix_venv" ]; then
    PYTHON="$posix_venv"
    python_source="project"
  elif [ -x "$windows_venv" ]; then
    PYTHON="$windows_venv"
    python_source="project"
  elif [ -n "$path_python3" ]; then
    PYTHON="$path_python3"
  elif [ -n "$path_python" ]; then
    python_major="$(python_env_major_version "$path_python")"
    if [ "$python_major" = "3" ]; then
      PYTHON="$path_python"
    else
      echo "python-env: ERROR — PATH python is not Python 3 (reported: ${python_major:-no version output})" >&2
      echo "  Install Python 3 or put python3 on PATH." >&2
      return 1
    fi
  else
    python_env_no_python "$posix_venv" "$windows_venv"
    return 1
  fi

  # Preserve the PyYAML-capable PATH python3 fallback for project virtualenvs.
  if ! python_env_has_pyyaml "$PYTHON" \
    && [ "$python_source" = "project" ] \
    && [ -n "$path_python3" ] \
    && python_env_has_pyyaml "$path_python3"; then
    PYTHON="$path_python3"
    echo "python-env: Fallback to PATH python3 (has PyYAML)" >&2
  fi

  if ! python_env_has_pyyaml "$PYTHON"; then
    echo "python-env: WARN — $PYTHON lacks PyYAML" >&2
    echo "  Run: $PYTHON -m pip install 'PyYAML>=6.0'" >&2
  fi

  export PYTHON
  echo "python-env: Using $PYTHON" >&2
}

resolve_python
