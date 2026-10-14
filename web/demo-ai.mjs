import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const bin = path.join(root, "reminecraft-mcp", "target", "debug", "reminecraft-mcp.exe");
const serverDir = process.env.REMINE_SERVER_DIR ?? path.join(root, "server");
const javaBin = process.env.REMINE_JAVA_BIN ?? path.join(root, ".jdk", "jdk-25", "bin", "java.exe");

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
async function call(name, args) {
  return send("tools/call", { name, arguments: args });
}
function textOf(r) {
  return (r.result?.content ?? []).filter((c) => c.type === "text").map((c) => c.text).join("\n");
}

await send("initialize", { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "demo", version: "1" } });
child.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) + "\n");

await call("server_start", { workdir: serverDir + "/", jar: "paper.jar", java: javaBin, memory: "2G" });
for (let i = 0; i < 30; i++) {
  await sleep(10000);
  const t = textOf(await call("server_logs", { path: serverDir + "/logs/latest.log", lines: 10 }));
  if (t.includes("Done (")) break;
}
await call("bot_join", { address: "127.0.0.1:25565", username: "RemineBot" });
for (let i = 0; i < 18; i++) {
  await sleep(10000);
  try {
    if (JSON.parse(textOf(await call("bot_status", {}))).connected) break;
  } catch {}
}
await call("bot_chat", { message: "/remine MCP" });
await sleep(5000);
await call("bot_goto", { x: 8, y: 70, z: 8 });
await sleep(8000);

const status = await call("bot_status", {});
console.log("AI MELIHAT STATUS BOT:");
console.log(textOf(status));

const shot = await call("screenshot", { save_path: serverDir + "/demo-shot.png" });
console.log("AI MELIHAT SCREENSHOT:");
console.log(textOf(shot).slice(0, 120) + "...[gambar PNG utuh masuk ke AI]");

await call("bot_leave", {});
await call("server_stop", {});
child.kill();
console.log("DEMO SELESAI");
