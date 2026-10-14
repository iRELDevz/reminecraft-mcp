# Reminecraft-MCP

MCP for developing Minecraft servers: the AI reads logs, runs servers, sends console commands via RCON, manages plugins, joins as a bot player (walks, chats, mines), pings Bedrock, captures screenshots, and launches games. Works like the Roblox Studio MCP: a single binary loaded by the AI agent as an MCP server.

Composition: Rust MCP core + bot, C# Windows sidecar (launcher/screenshots), C++ RakNet helper, TypeScript smoke tests.

## Structure

```text
reminecraft-mcp/  Rust — dual-transport MCP server (rmcp + azalea), 22 tools
windows-agent/    C# — localhost:5123 sidecar (launch/screenshot/processes)
native/           C++ — Bedrock RakNet ping (lib + CLI)
web/              TypeScript — stdio smoke test, flow test, plugin test
plugin/           Java — example RemineTest plugin (/remine)
plugin-shim/      Java — RemineShim, localhost:5124 HTTP API for the AI
```

## Requirements

- Rust nightly-2026-03-30 via `rust-toolchain.toml` (matches the azalea 0.16 release, pinned automatically)
- .NET 10 SDK for windows-agent
- g++ or CMake for native (optional)
- Node 18+ for tests (optional)
- Java server for the bot: azalea 0.16 speaks the Minecraft 26.1 protocol. Servers on other versions are rejected at handshake.
- RCON: `enable-rcon=true` and `rcon.password` in `server.properties` for `rcon_command`.

## Install via AI (zero hassle)

Tell your AI in this project folder:

```text
install Reminecraft-MCP: run setup/setup.ps1
```

The AI handles the rest: Rust + C# builds, JDK 25 and stable Paper download, both plugins compiled, server config written, and MCP registration into Claude Desktop + OpenCode. Machine requirements: cargo (rustup), .NET 10 SDK, Node 18+. The Rust toolchain pins itself via `rust-toolchain.toml`. Re-run anytime, safe (idempotent, old configs backed up as `.bak`).

`setup/setup.ps1` options: `-ServerDir`, `-ServerKind paper|vanilla|purpur|bedrock`, `-McVersion`, `-RconPassword`, `-BedrockUrl`.

## Transport

One binary, two modes. Pick whichever way your AI agent connects.

| Agent | Command |
|---|---|
| Local (OpenCode / Claude Desktop) | `reminecraft-mcp --transport stdio` |
| Remote / multi-client | `reminecraft-mcp --transport http --host 127.0.0.1 --port 8000`, endpoint `/mcp` |

STDIO config (`command` = exe path + `args: ["--transport", "stdio"]`). HTTP config (`url` = `http://host:8000/mcp`, header `Accept: application/json, text/event-stream`). Never expose HTTP publicly without auth.

## Build and run

```powershell
cargo build --manifest-path reminecraft-mcp/Cargo.toml
.\reminecraft-mcp\target\debug\reminecraft-mcp.exe --transport stdio

dotnet run --project windows-agent --urls http://127.0.0.1:5123

g++ -O2 -std=c++17 -DREMINE_BEDROCK_PING_CLI native/bedrock_ping.cpp -o native/bedrock_ping.exe -lws2_32

node web/test-stdio.mjs
```

## Tools

Server (any kind, only java vs bedrock matters): `server_start` (kind java: paper/vanilla/spigot/purpur/fabric, kind bedrock: bedrock_server.exe), `server_stop`, `server_status`, `server_console` (stdin, the RCON replacement — the only console Bedrock has), `server_logs`, `plugin_list` (plugins/ or mods/), `datapack_list`, `rcon_command` (Java only).
Java bot: `bot_join`, `bot_leave`, `bot_chat`, `bot_goto`, `bot_stop`, `bot_mine`, `bot_status`.
Shim (needs RemineShim in the plugins folder): `shim_players`, `shim_entities`, `shim_entity`, `shim_entity_set` (tp/remove/name/glow/velocity), `shim_exec`.
Bedrock: `bedrock_ping` (Rust) and `native/bedrock_ping.exe` (C++).
Windows: `screenshot` (PNG image + optional save), `launcher_launch` (Prism `--launch instance` or custom executable), `windows_agent_ping`.

Plugin dev loop: `server_start` -> edit files in `server/plugins` -> `rcon_command` reload -> `bot_join` -> `bot_chat` command test -> `server_logs` check results -> `screenshot` see the screen.
