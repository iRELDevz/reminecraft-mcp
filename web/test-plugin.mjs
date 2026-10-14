import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const bin = process.platform === "win32"
  ? path.join(root, "reminecraft-mcp", "target", "debug", "reminecraft-mcp.exe")
  : path.join(root, "reminecraft-mcp", "target", "debug", "reminecraft-mcp");
const serverDir = process.env.REMINE_SERVER_DIR ?? path.join(root, "server");
const javaBin = process.env.REMINE_JAVA_BIN ?? (() => {
  const jdkDir = fs.readdirSync(path.join(root, ".jdk")).find((d) => d.startsWith("jdk-25"));
  return path.join(root, ".jdk", jdkDir, "bin", process.platform === "win32" ? "java.exe" : "java");
})();

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
function check(name, ok, shown) {
  results.push([ok ? "ok" : "FAIL", name, String(shown).slice(0, 200)]);
  console.log((ok ? "ok " : "FAIL ") + name + " " + String(shown).slice(0, 200));
}
async function call(name, args) {
  const r = await send("tools/call", { name, arguments: args });
  return r;
}
function textOf(r) {
  return (r.result?.content ?? []).filter((c) => c.type === "text").map((c) => c.text).join("\n");
}

await send("initialize", { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "plugintest", version: "1" } });
notify("notifications/initialized");

let r = await call("server_start", { workdir: serverDir + "/", jar: "paper.jar", java: javaBin, memory: "2G" });
check("server_start", textOf(r).includes("started"), textOf(r));

let ready = false;
for (let i = 0; i < 36; i++) {
  await sleep(10000);
  r = await call("server_logs", { path: serverDir + "/logs/latest.log", lines: 30 });
  const t = textOf(r);
  if (t.includes("Done (") || t.includes("Running on")) { ready = true; break; }
}
check("server_boot", ready, ready ? "done" : "timeout");

r = await call("plugin_list", { dir: serverDir + "/plugins" });
check("plugin_list", textOf(r).includes("RemineTest.jar"), textOf(r));

r = await call("server_logs", { path: serverDir + "/logs/latest.log", lines: 400 });
check("plugin_enable", textOf(r).includes("REMINE_ENABLE"), "REMINE_ENABLE found");

r = await call("rcon_command", { password: "remine-test", command: "list" });
check("rcon_list", !r.error, textOf(r) || JSON.stringify(r.error));

r = await call("bot_join", { address: "127.0.0.1:25565", username: "RemineBot" });
check("bot_join", textOf(r).includes("joining"), textOf(r));

let joined = false;
for (let i = 0; i < 18; i++) {
  await sleep(10000);
  r = await call("bot_status", {});
  try {
    const s = JSON.parse(textOf(r));
    if (s.connected) { joined = true; break; }
  } catch {}
}
check("bot_connected", joined, joined ? "connected" : "timeout");

r = await call("shim_players", {});
let botUuid = "";
try {
  const p = JSON.parse(textOf(r));
  const me = (p.players ?? []).find((x) => x.name === "RemineBot");
  if (me) botUuid = me.uuid;
} catch {}
check("shim_players", botUuid !== "", botUuid || "bot tidak ada");

r = await call("shim_entities", { limit: 50 });
let entityCount = -1;
try { entityCount = JSON.parse(textOf(r)).count ?? -1; } catch {}
check("shim_entities", entityCount > 0, "count=" + entityCount);

if (botUuid) {
  r = await call("shim_entity", { uuid: botUuid });
  check("shim_entity", textOf(r).includes("PLAYER") || textOf(r).includes("RemineBot"), textOf(r).slice(0, 200));
  r = await call("shim_entity_set", { uuid: botUuid, op: "glow", value: "true" });
  check("shim_glow", textOf(r).includes("true"), textOf(r));
  r = await call("shim_entity_set", { uuid: botUuid, op: "glow", value: "false" });
  check("shim_unglow", textOf(r).includes("true"), textOf(r));
}
r = await call("shim_exec", { command: "list" });
check("shim_exec", textOf(r).includes("ok"), textOf(r).slice(0, 200));

r = await call("bot_chat", { message: "/remine MCP" });
check("bot_remine_cmd", textOf(r).includes("sent"), textOf(r));
await sleep(5000);

r = await call("bot_status", {});
let greeted = false;
try {
  const s = JSON.parse(textOf(r));
  greeted = (s.recent_chat ?? []).join("\n").includes("Halo MCP");
} catch {}
check("bot_greeted", greeted, greeted ? "greeting seen" : textOf(r).slice(0, 200));

r = await call("server_logs", { path: serverDir + "/logs/latest.log", lines: 400 });
const logs = textOf(r);
check("log_join", logs.includes("REMINE_JOIN"), "REMINE_JOIN found");
check("log_cmd", logs.includes("REMINE_CMD"), "REMINE_CMD found");

r = await call("bot_leave", {});
check("bot_leave", textOf(r).includes("left"), textOf(r));
r = await call("server_stop", {});
check("server_stop", textOf(r).includes("stopped"), textOf(r));

const failed = results.filter(([s]) => s === "FAIL");
console.log(failed.length === 0 ? "PLUGIN TEST PASS" : "PLUGIN TEST FAIL " + failed.length);
child.kill();
process.exit(failed.length === 0 ? 0 : 1);
