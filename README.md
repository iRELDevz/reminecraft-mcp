# Reminecraft-MCP

MCP untuk develop server Minecraft: AI membaca log, menjalankan server, mengirim console command via RCON, mengelola plugin, join sebagai bot player (jalan, chat, mining), ping Bedrock, screenshot layar, dan menjalankan launcher. Cara kerja mirip MCP Roblox Studio: satu binary yang dimuat AI agent sebagai MCP server.

Komposisi: Rust inti MCP + bot, C# sidecar Windows (launcher/screenshot), C++ helper RakNet, TypeScript smoke test.

## Struktur

```text
reminecraft-mcp/  Rust — MCP server dual transport (rmcp + azalea), 22 tools
windows-agent/    C# — sidecar localhost:5123 (launch/screenshot/proses)
native/           C++ — ping RakNet Bedrock (lib + CLI)
web/              TypeScript — smoke test, flow test, plugin test
plugin/           Java — plugin contoh RemineTest (/remine)
plugin-shim/      Java — RemineShim, HTTP API localhost:5124 untuk AI
```

## Syarat

- Rust nightly-2026-03-30 via `rust-toolchain.toml` (sesuai rilis azalea 0.16, toolchain dipin otomatis)
- .NET 10 SDK untuk windows-agent
- g++ atau CMake untuk native (opsional)
- Node 18+ untuk smoke test (opsional)
- Server Java untuk bot: azalea 0.16 berbicara protokol Minecraft 26.1. Server beda versi ditolak saat handshake.
- RCON: `enable-rcon=true` dan `rcon.password` di `server.properties` untuk `rcon_command`.

## Transport

Satu binary, dua mode. Pilih sesuai AI agent masuk lewat mana.

| Agent | Command |
|---|---|
| Lokal (OpenCode / Claude Desktop) | `reminecraft-mcp --transport stdio` |
| Remote / multi-client | `reminecraft-mcp --transport http --host 127.0.0.1 --port 8000`, endpoint `/mcp` |

STDIO config (`command` = path exe + `args: ["--transport", "stdio"]`). HTTP config (`url` = `http://host:8000/mcp`, header `Accept: application/json, text/event-stream`). Jangan expose HTTP ke publik tanpa auth.

## Pasang lewat AI (tanpa ribet)

Ucapkan ini ke AI kamu di folder proyek ini:

```text
pasang Reminecraft-MCP: jalankan setup/setup.ps1
```

AI mengerjakan sisanya: build Rust + C#, download JDK 25 dan Paper stabil, compile 2 plugin, tulis config server, dan daftarkan MCP ke Claude Desktop + OpenCode. Syarat di mesin: cargo (rustup), .NET 10 SDK, Node 18+. Toolchain Rust dipin otomatis via `rust-toolchain.toml`. Ulangi perintah yang sama kapan saja, aman (idempoten, config lama di-backup `.bak`).

## Build dan run

```powershell
cargo +nightly build --manifest-path reminecraft-mcp/Cargo.toml
.\reminecraft-mcp\target\debug\reminecraft-mcp.exe --transport stdio

dotnet run --project windows-agent --urls http://127.0.0.1:5123

g++ -O2 -std=c++17 -DREMINE_BEDROCK_PING_CLI native/bedrock_ping.cpp -o native/bedrock_ping.exe -lws2_32

node web/test-stdio.mjs
```

## Tools

Server (semua jenis, bedanya cuma java vs bedrock): `server_start` (kind java: paper/vanilla/spigot/purpur/fabric, kind bedrock: bedrock_server.exe), `server_stop`, `server_status`, `server_console` (stdin, pengganti RCON — satu-satunya console Bedrock), `server_logs`, `plugin_list` (plugins/ maupun mods/), `datapack_list`, `rcon_command` (Java saja).
Bot Java: `bot_join`, `bot_leave`, `bot_chat`, `bot_goto`, `bot_stop`, `bot_mine`, `bot_status`.
Shim (butuh RemineShim di folder plugins): `shim_players`, `shim_entities`, `shim_entity`, `shim_entity_set` (tp/remove/name/glow/velocity), `shim_exec`.
Bedrock: `bedrock_ping` (Rust) dan `native/bedrock_ping.exe` (C++).
Windows: `screenshot` (gambar PNG + simpan opsional), `launcher_launch` (Prism `--launch instance` atau executable custom), `windows_agent_ping`.

Alur develop plugin: `server_start` -> edit file di `server/plugins` -> `rcon_command` reload -> `bot_join` -> `bot_chat` command test -> `server_logs` cek hasil -> `screenshot` lihat layar.
