# FreeClash

FreeClash is a Windows-first Tauri app launcher for mihomo. It starts selected applications with
per-rule `HTTP_PROXY` and `HTTPS_PROXY` environment variables, routes those requests through a
rule-specific metering proxy, and then forwards traffic into a dedicated mihomo HTTP listener.

## Current v1 Scope

- Uses the existing mihomo core at `core/verge-mihomo.exe`.
- Imports one Clash/mihomo subscription URL through a mihomo `proxy-provider`.
- Manages application rules with app path, launch arguments, working directory, selected node, and
  enable state.
- Shows per-rule upload/download speed, total traffic, active connection count, and recent targets.
- Supports HTTP and HTTPS proxy traffic. HTTPS targets are shown as `host:port`; HTTPS contents are
  not decrypted.
- Does not use TUN and does not capture traffic from applications that were not launched by
  FreeClash.

## Development

```powershell
npm install
npm run build
npm run tauri:dev
```

The frontend build can run with Node.js alone. `npm run tauri:dev` requires a Rust toolchain and the
Windows Tauri prerequisites.

