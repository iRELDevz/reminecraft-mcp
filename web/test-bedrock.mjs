import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const bin = path.join(root, "reminecraft-mcp", "target", "debug", "reminecraft-mcp.exe");
const serverDir = "C:/Bedrock-Server";

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

await send("initialize", { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "bedrocktest", version: "1" } });
child.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) + "\n");

let r = await call("server_start", { kind: "bedrock", workdir: serverDir + "/", executable: serverDir + "/bedrock_server.exe" });
check("bds_start", textOf(r).includes("started"), textOf(r));

let ready = false;
for (let i = 0; i < 30; i++) {
  await sleep(10000);
  const t = textOf(await call("server_logs", { path: serverDir + "/logs/remine-console.log", lines: 200 }));
  if (t.includes("Server started")) { ready = true; break; }
}
check("bds_boot", ready, ready ? "started" : "timeout");

r = await call("server_console", { command: "list" });
await sleep(4000);
r = await call("server_logs", { path: serverDir + "/logs/remine-console.log", lines: 60 });
check("bds_console", /There are|players online|No players/.test(textOf(r)), textOf(r).slice(-120));

r = await call("bedrock_ping", { host: "127.0.0.1" });
check("bds_ping", !r.error, !r.error ? textOf(r).slice(0, 160) : JSON.stringify(r.error).slice(0, 160));

await call("server_stop", {});

const failed = results.filter(([s]) => s === "FAIL");
console.log(failed.length === 0 ? "BEDROCK TEST PASS" : "BEDROCK TEST FAIL " + failed.length);
child.kill();
process.exit(failed.length === 0 ? 0 : 1);
