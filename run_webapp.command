#!/bin/bash

# Start passoflow's API server and web UI on macOS, then open the editor in the browser.
set -u

REPO_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$REPO_DIR" || exit 1

if [ -x "$REPO_DIR/.venv/bin/python" ]; then
  PYTHON_BIN="$REPO_DIR/.venv/bin/python"
elif command -v python3 >/dev/null 2>&1; then
  PYTHON_BIN="$(command -v python3)"
else
  echo "Python 3 is required. Install it and run this script again."
  read -r -p "Press Enter to close..."
  exit 1
fi

if ! command -v npm >/dev/null 2>&1; then
  echo "Node.js and npm are required. Install them and run this script again."
  read -r -p "Press Enter to close..."
  exit 1
fi

mkdir -p "$REPO_DIR/logs"

"$PYTHON_BIN" src/api_server.py >"$REPO_DIR/logs/api_server_mac.log" 2>&1 &
API_PID=$!

(
  cd "$REPO_DIR/web-ui" || exit 1
  npm run dev -- --host 127.0.0.1 --strictPort
) >"$REPO_DIR/logs/web_ui_mac.log" 2>&1 &
UI_PID=$!

cleanup() {
  kill "$API_PID" "$UI_PID" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

echo "Starting passoflow..."
for _ in $(seq 1 30); do
  if curl --silent --fail http://127.0.0.1:5173 >/dev/null 2>&1; then
    open http://127.0.0.1:5173
    echo "passoflow is ready at http://127.0.0.1:5173"
    echo "Press Ctrl-C to stop both servers."
    wait
    exit 0
  fi
  if ! kill -0 "$API_PID" 2>/dev/null || ! kill -0 "$UI_PID" 2>/dev/null; then
    echo "passoflow failed to start. Check logs/api_server_mac.log and logs/web_ui_mac.log."
    read -r -p "Press Enter to close..."
    exit 1
  fi
  sleep 1
done

echo "Timed out waiting for the web UI. Check logs/web_ui_mac.log."
read -r -p "Press Enter to close..."
exit 1
