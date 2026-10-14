import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const bin = path.join(root, "reminecraft-mcp", "target", "debug", "reminecraft-mcp.exe");
const serverDir = "C:/Vanilla-Server";
const javaBin = "C:/Reminecraft-Server/jdk-25/bin/java.exe";
const rconPass = "remine-test";

const child = spawn(bin, ["--transport", "stdio"], { stdio: ["pipe", "pipe", "ignore"], cwd: root });

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

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const results = [];
function check(name, ok, shown) {
  results.push([ok ? "ok" : "FAIL", name]);
  console.log((ok ? "ok " : "FAIL ") + name + " " + String(shown).slice(0, 160));
}
async function call(name, args) {
  return send("tools/call", { name, arguments: args });
}
function textOf(r) {
  return (r.result?.content ?? []).filter((c) => c.type === "text").map((c) => c.text).join("\n");
}

await send("initialize", { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "vanillatest", version: "1" } });
child.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) + "\n");

if (!fs.existsSync(path.join(serverDir, "eula.txt"))) {
  fs.writeFileSync(path.join(serverDir, "eula.txt"), "eula=true\n");
}

let r = await call("server_start", { kind: "java", workdir: serverDir + "/", jar: "server.jar", java: javaBin, memory: "2G" });
check("server_start", textOf(r).includes("started"), textOf(r));

let ready = false;
for (let i = 0; i < 36; i++) {
  await sleep(10000);
  const t = textOf(await call("server_logs", { path: serverDir + "/logs/remine-console.log", lines: 200 }));
  if (t.includes("Done (")) { ready = true; break; }
}
check("vanilla_boot", ready, ready ? "done" : "timeout");
await call("server_stop", {});

fs.writeFileSync(path.join(serverDir, "server.properties"), [
  "server-port=25566", "online-mode=false", "enable-rcon=true",
  "rcon.password=" + rconPass, "rcon.port=25576", "motd=Remine Vanilla",
  "max-players=5", "gamemode=creative", "difficulty=peaceful",
  "view-distance=4", "simulation-distance=4", "spawn-protection=0",
].join("\n") + "\n");

const dpBase = path.join(serverDir, "world", "datapacks", "remine_pack", "data", "remine", "function");
fs.mkdirSync(dpBase, { recursive: true });
fs.writeFileSync(path.join(serverDir, "world", "datapacks", "remine_pack", "pack.mcmeta"), JSON.stringify({ pack: { description: "remine test", pack_format: 80 } }));
fs.writeFileSync(path.join(dpBase, "hello.mcfunction"), "say hello dari datapack remine\n");

r = await call("server_start", { kind: "java", workdir: serverDir + "/", jar: "server.jar", java: javaBin, memory: "2G" });
check("server_restart", textOf(r).includes("started"), textOf(r));
ready = false;
for (let i = 0; i < 36; i++) {
  await sleep(10000);
  const t = textOf(await call("server_logs", { path: serverDir + "/logs/remine-console.log", lines: 200 }));
  if (t.includes("Done (")) { ready = true; break; }
}
check("vanilla_boot2", ready, ready ? "done" : "timeout");

r = await call("server_console", { command: "list" });
await sleep(3000);
r = await call("server_logs", { path: serverDir + "/logs/remine-console.log", lines: 30 });
check("console_list", textOf(r).includes("players online"), "list via stdin");

r = await call("datapack_list", { world: serverDir + "/world" });
check("datapack_list", textOf(r).includes("remine_pack"), textOf(r));

r = await call("bot_join", { address: "127.0.0.1:25566", username: "RemineBot" });
check("bot_join", textOf(r).includes("joining"), textOf(r));
let joined = false;
for (let i = 0; i < 18; i++) {
  await sleep(10000);
  try {
    if (JSON.parse(textOf(await call("bot_status", {}))).connected) { joined = true; break; }
  } catch {}
}
check("bot_connected_vanilla", joined, joined ? "connected" : "timeout");

r = await call("rcon_command", { port: 25576, password: rconPass, command: "list" });
check("rcon_list", !r.error, textOf(r) || JSON.stringify(r.error));

await call("bot_leave", {});
await call("server_stop", {});

const failed = results.filter(([s]) => s === "FAIL");
console.log(failed.length === 0 ? "VANILLA TEST PASS" : "VANILLA TEST FAIL " + failed.length);
child.kill();
process.exit(failed.length === 0 ? 0 : 1);
