import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const bin = process.platform === "win32"
  ? path.join(root, "reminecraft-mcp", "target", "debug", "reminecraft-mcp.exe")
  : path.join(root, "reminecraft-mcp", "target", "debug", "reminecraft-mcp");

const child = spawn(bin, ["--transport", "stdio"], { stdio: ["pipe", "pipe", "inherit"], cwd: root });

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

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const results = [];
async function call(name, args, expect) {
  const r = await send("tools/call", { name, arguments: args });
  const out = JSON.stringify(r.result ?? r.error);
  const ok = expect.test(out);
  results.push([ok ? "ok" : "FAIL", name, out.slice(0, 220)]);
}
async function callField(name, args, field, value) {
  const r = await send("tools/call", { name, arguments: args });
  let ok = false;
  let shown = JSON.stringify(r.result ?? r.error).slice(0, 220);
  try {
    const text = r.result.content.find((c) => c.type === "text")?.text ?? "";
    ok = JSON.parse(text)[field] === value;
  } catch {}
  results.push([ok ? "ok" : "FAIL", name, shown]);
}

await send("initialize", { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "flow", version: "1" } });
notify("notifications/initialized");

await call("server_start", { workdir: "./server", jar: "paper.jar", java: "java", memory: "1G" }, /started/);
await call("server_status", {}, /running/);
await call("plugin_list", {}, /plugins/);
await call("bot_join", { address: "127.0.0.1:25565", username: "RemineBot" }, /joining/);
await sleep(8000);
await callField("bot_status", {}, "joined", true);
await call("bot_chat", { message: "hello dari test" }, /sent|bot_not_spawned_yet/);
await call("bot_goto", { x: 0, y: 64, z: 0 }, /going|bot_not_spawned_yet/);
await callField("bot_leave", {}, "left", true);
await callField("bot_status", {}, "joined", false);
await call("server_stop", {}, /stopped|server_not_running/);
await call("bedrock_ping", { host: "127.0.0.1" }, /bedrock_ping_failed/);
await call("screenshot", {}, /image/);
await call("windows_agent_ping", {}, /windows-agent OK/);
await call("launcher_launch", { instance: "" }, /launcher_failed|windows_agent_unreachable|instance/);
await call("server_logs", { path: "./tidak-ada.log" }, /log_read_failed/);
await call("rcon_command", { password: "x", command: "list" }, /rcon_failed/);

for (const [s, n, o] of results) console.log(s, n, o);
const failed = results.filter(([s]) => s === "FAIL");
console.log(failed.length === 0 ? "FLOW PASS" : "FLOW FAIL " + failed.length);
child.kill();
process.exit(failed.length === 0 ? 0 : 1);
