#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="${PROFILE:-release}"
OUT_DIR="${ROOT_DIR}/target/wasm_text_smoke"
PORT="${PORT:-8787}"
URL="http://127.0.0.1:${PORT}/index.html"

cd "${ROOT_DIR}"

if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "error: wasm-bindgen CLI is required."
  echo "install with: cargo install wasm-bindgen-cli"
  exit 1
fi

if ! rustup target list --installed | rg -q '^wasm32-unknown-unknown$'; then
  echo "Installing wasm target wasm32-unknown-unknown..."
  rustup target add wasm32-unknown-unknown
fi

if [[ "${PROFILE}" != "debug" && "${PROFILE}" != "release" ]]; then
  echo "error: PROFILE must be 'debug' or 'release' (got '${PROFILE}')"
  exit 1
fi

echo "Building wasm_text_smoke example (${PROFILE})..."
if [[ "${PROFILE}" == "release" ]]; then
  cargo build --release --example wasm_text_smoke --target wasm32-unknown-unknown --features wasm
  WASM_INPUT="${ROOT_DIR}/target/wasm32-unknown-unknown/release/examples/wasm_text_smoke.wasm"
else
  cargo build --example wasm_text_smoke --target wasm32-unknown-unknown --features wasm
  WASM_INPUT="${ROOT_DIR}/target/wasm32-unknown-unknown/debug/examples/wasm_text_smoke.wasm"
fi

mkdir -p "${OUT_DIR}"
wasm-bindgen \
  --target web \
  --out-dir "${OUT_DIR}" \
  "${WASM_INPUT}"

WASM_BYTES="$(wc -c < "${OUT_DIR}/wasm_text_smoke_bg.wasm" | tr -d ' ')"
echo "wasm payload size: ${WASM_BYTES} bytes"

cat > "${OUT_DIR}/index.html" <<'HTML'
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Plutonium WASM Text Smoke</title>
    <style>
      body {
        margin: 0;
        background: #0f1219;
      }
      canvas {
        display: block;
        width: 960px;
        height: 540px;
        border: 1px solid #3f4a5f;
      }
      #pluto-debug {
        color: #d3def4;
        font: 12px/1.4 "SF Mono", "Menlo", monospace;
        padding: 8px 10px;
      }
    </style>
  </head>
  <body>
    <canvas id="pluto-canvas" width="960" height="540"></canvas>
    <div id="pluto-debug">booting...</div>
    <script type="module">
      import init, { run_smoke } from "./wasm_text_smoke.js";
      const debugEl = document.getElementById("pluto-debug");
      const setDebug = (msg) => {
        if (debugEl) debugEl.textContent = String(msg);
        document.title = String(msg);
      };

      // Keep browser diagnostics usable while debugging wasm input handling.
      // Capture before app listeners so right-click/devtools remain available.
      const canvas = document.getElementById("pluto-canvas");
      if (canvas) {
        canvas.addEventListener("contextmenu", (e) => e.stopImmediatePropagation(), true);
      }
      window.addEventListener("contextmenu", (e) => e.stopImmediatePropagation(), true);

      window.addEventListener("error", (e) => {
        setDebug(`js error: ${e.message || "unknown"}`);
      });
      window.addEventListener("unhandledrejection", (e) => {
        const reason = e.reason && (e.reason.message || e.reason.toString()) || "unknown";
        setDebug(`promise rejection: ${reason}`);
      });

      (async () => {
        try {
          setDebug("instantiating wasm (streaming)...");
          // Use wasm-bindgen's default streaming path to avoid long main-thread stalls.
          await init();
          setDebug("wasm module loaded; starting smoke...");
          if (typeof run_smoke !== "function") {
            throw new Error("run_smoke export missing");
          }
          run_smoke();
          setDebug("run_smoke invoked");
        } catch (err) {
          setDebug(`wasm init failed: ${err && (err.message || err.toString())}`);
        }
      })();
    </script>
  </body>
</html>
HTML

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "${SERVER_PID}" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM

echo "Serving ${OUT_DIR} at ${URL}"
python3 -m http.server "${PORT}" --directory "${OUT_DIR}" >/tmp/wasm_text_smoke_server.log 2>&1 &
SERVER_PID=$!

sleep 1
if command -v open >/dev/null 2>&1; then
  open "${URL}"
elif command -v xdg-open >/dev/null 2>&1; then
  xdg-open "${URL}"
else
  echo "Open this URL in your browser: ${URL}"
fi

echo "Smoke test running. Press Ctrl+C to stop the server."
wait "${SERVER_PID}"
