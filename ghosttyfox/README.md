# Ghosttyfox

> A real terminal in a Firefox tab, powered by [Ghostty](https://ghostty.org)'s WASM terminal engine.

Click a button → get a terminal tab running your actual shell. No remote servers. No web terminals. Your local shell, rendered with Ghostty's battle-tested VT100 parser.

## Architecture

```
┌──────────────────────────────┐      ┌──────────────────────────┐
│  Firefox Tab                 │      │  Native Host (Rust)      │
│                              │      │                          │
│  ghostty-web (WASM)          │◄────►│  PTY management          │
│  Canvas renderer             │ JSON │  Shell spawning ($SHELL) │
│  Input handling              │      │  Resize support          │
│                              │      │                          │
│  Native Messaging bridge     │      │  stdio protocol          │
└──────────────────────────────┘      └──────────────────────────┘
```

The extension has two parts:

1. **Firefox extension** — opens a full-page terminal in a new tab using [ghostty-web](https://github.com/coder/ghostty-web), Ghostty's VT100 parser compiled to WebAssembly
2. **Native host** — a small Rust binary that spawns a PTY, runs your shell, and bridges I/O over Firefox's [native messaging](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Native_messaging) protocol

## Requirements

- Firefox (any recent version)
- [Rust](https://rustup.rs/) (for building the native host)
- [Node.js](https://nodejs.org/) 18+ and npm (for bundling the extension)
- macOS (Linux support planned)

## Quick Start

```bash
# Clone and install
cd ghosttyfox
npm install

# Build everything and register with Firefox
bash scripts/install.sh

# Load in Firefox:
# 1. Open about:debugging#/runtime/this-firefox
# 2. Click "Load Temporary Add-on"
# 3. Select extension/manifest.json (in the dist/ folder after build)
```

## Development

```bash
# Build native host (debug) + bundle extension + launch Firefox with web-ext
bash scripts/dev.sh
```

## How It Works

1. You click the toolbar button
2. The extension opens `terminal.html` as a new tab
3. `terminal.js` initializes ghostty-web (loads WASM, creates Terminal + FitAddon)
4. It connects to the native host via `browser.runtime.connectNative()`
5. The native host spawns your `$SHELL` in a PTY
6. Keystrokes flow: Terminal → extension → native host → PTY
7. Output flows: PTY → native host → extension → Terminal (base64-encoded)
8. Resize events keep the PTY dimensions in sync

### Message Protocol

Extension → Host:
```json
{"type": "input", "data": "ls -la\n"}
{"type": "resize", "cols": 120, "rows": 36}
```

Host → Extension:
```json
{"type": "output", "data": "<base64-encoded bytes>"}
{"type": "exit", "code": 0}
{"type": "error", "message": "..."}
```

## Project Structure

```
ghosttyfox/
├── extension/
│   ├── manifest.json       Firefox extension manifest (MV2)
│   ├── background.js       Toolbar button → open terminal tab
│   ├── terminal.html       Full-page terminal container
│   ├── terminal.js         ghostty-web + native messaging bridge
│   └── terminal.css        Dark theme, full viewport
├── native-host/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs         Entry point
│       ├── protocol.rs     Native messaging frame I/O
│       └── pty.rs          PTY session management
├── scripts/
│   ├── build.js            esbuild bundler for extension
│   ├── install.sh          Build + register native host
│   └── dev.sh              Dev workflow with web-ext
├── package.json
└── README.md
```

## Troubleshooting

**Extension won't load WASM:**
Check that `content_security_policy` in manifest.json includes `'wasm-eval'`.

**Native host not found:**
Run `scripts/install.sh` to register the native messaging manifest. Check that the binary path in `~/Library/Application Support/Mozilla/NativeMessagingHosts/ghosttyfox.json` is correct.

**Terminal connects but shows nothing:**
Check the browser console for errors. The native host logs to stderr — run it manually to debug:
```bash
echo '{"type":"resize","cols":80,"rows":24}' | python3 -c "
import struct, sys
msg = sys.stdin.read().encode()
sys.stdout.buffer.write(struct.pack('<I', len(msg)) + msg)
" | ./native-host/target/debug/ghosttyfox-host
```

**Resize doesn't work:**
Make sure the FitAddon is loaded and `observeResize()` is called.

## License

MIT
