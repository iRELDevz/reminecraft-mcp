import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const bin = process.platform === "win32"
  ? path.join(root, "reminecraft-mcp", "target", "debug", "reminecraft-mcp.exe")
  : path.join(root, "reminecraft-mcp", "target", "debug", "reminecraft-mcp");

const child = spawn(bin, ["--transport", "stdio"], { stdio: ["pipe", "pipe", "inherit"] });

let buf = "";
const pending = new Map();
let id = 0;
child.stdout.setEncoding("utf8");
child.stdout.on("data", (d) => {
  buf += d;
  let i;
  while ((i = buf.indexOf("\n")) >= 0) {
    const line = buf.slice(0, i).trim();
    buf = buf.slice(i + 1);
    if (!line) continue;
    const msg = JSON.parse(line);
    if (msg.id !== undefined && pending.has(msg.id)) {
      pending.get(msg.id)(msg);
      pending.delete(msg.id);
    }
  }
});

function send(method, params) {
  return new Promise((resolve) => {
    const myId = ++id;
    pending.set(myId, resolve);
    child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id: myId, method, params }) + "\n");
  });
}

function notify(method) {
  child.stdin.write(JSON.stringify({ jsonrpc: "2.0", method }) + "\n");
}

const expected = ["server_logs", "server_start", "server_stop", "server_status", "plugin_list", "rcon_command", "bot_join", "bot_leave", "bot_chat", "bot_goto", "bot_stop", "bot_mine", "bot_status", "bedrock_ping", "screenshot", "launcher_launch", "windows_agent_ping"];

const init = await send("initialize", { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "smoke", version: "1" } });
if (!init.result) throw new Error("initialize gagal: " + JSON.stringify(init));
notify("notifications/initialized");
const list = await send("tools/list", {});
const names = (list.result?.tools ?? []).map((t) => t.name);
const missing = expected.filter((n) => !names.includes(n));
if (missing.length > 0) throw new Error("tools hilang: " + missing.join(","));
const status = await send("tools/call", { name: "bot_status", arguments: {} });
const text = status.result?.content?.[0]?.text ?? "";
if (!text.includes("joined")) throw new Error("bot_status respon aneh: " + text);
console.log("PASS tools=" + names.length + " bot_status=" + text);
child.kill();
process.exit(0);
