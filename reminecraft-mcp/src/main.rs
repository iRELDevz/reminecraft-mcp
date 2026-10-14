use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use azalea::BlockPos;
use azalea::pathfinder::PathfinderClientExt;
use azalea::pathfinder::goals::BlockPosGoal;
use azalea::prelude::*;
use clap::{Parser, ValueEnum};
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars,
    tool, tool_handler, tool_router,
    transport::{
        stdio,
        streamable_http_server::{
            StreamableHttpServerConfig, StreamableHttpService,
            session::local::LocalSessionManager,
        },
    },
    ServiceExt,
};
use serde_json::json;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TransportKind {
    Stdio,
    Http,
}

#[derive(Parser, Debug)]
#[command(name = "reminecraft-mcp", about = "Reminecraft MCP - dual transport (stdio + streamable http)")]
struct Args {
    #[arg(long, value_enum, default_value = "stdio")]
    transport: TransportKind,
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value = "8000")]
    port: u16,
    #[arg(long, default_value = "http://127.0.0.1:5123")]
    windows_agent: String,
    #[arg(long, default_value = "http://127.0.0.1:5124")]
    shim: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ServerLogsArgs {
    #[serde(default = "default_log_path")]
    pub path: String,
    #[serde(default = "default_log_lines")]
    pub lines: usize,
}

fn default_log_path() -> String {
    "./server/logs/latest.log".to_string()
}
fn default_log_lines() -> usize {
    100
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ServerStartArgs {
    #[serde(default = "default_workdir")]
    pub workdir: String,
    #[serde(default = "default_server_kind")]
    pub kind: String,
    #[serde(default = "default_jar")]
    pub jar: String,
    #[serde(default = "default_java")]
    pub java: String,
    #[serde(default)]
    pub executable: Option<String>,
    #[serde(default = "default_memory")]
    pub memory: String,
    #[serde(default = "default_nogui")]
    pub nogui: bool,
}

fn default_server_kind() -> String {
    "java".to_string()
}

fn default_workdir() -> String {
    "./server".to_string()
}
fn default_jar() -> String {
    "paper.jar".to_string()
}
fn default_java() -> String {
    "java".to_string()
}
fn default_memory() -> String {
    "2G".to_string()
}
fn default_nogui() -> bool {
    true
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct PluginListArgs {
    #[serde(default = "default_plugin_dir")]
    pub dir: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct DatapackListArgs {
    #[serde(default = "default_world_dir")]
    pub world: String,
}

fn default_world_dir() -> String {
    "./server/world".to_string()
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ServerConsoleArgs {
    pub command: String,
}

fn default_plugin_dir() -> String {
    "./server/plugins".to_string()
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct RconArgs {
    #[serde(default = "default_rcon_host")]
    pub host: String,
    #[serde(default = "default_rcon_port")]
    pub port: u16,
    pub password: String,
    pub command: String,
}

fn default_rcon_host() -> String {
    "127.0.0.1".to_string()
}
fn default_rcon_port() -> u16 {
    25575
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct BotJoinArgs {
    pub address: String,
    #[serde(default = "default_bot_name")]
    pub username: String,
}

fn default_bot_name() -> String {
    "RemineBot".to_string()
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct BotChatArgs {
    pub message: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct BotGotoArgs {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct BotMineArgs {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct BedrockPingArgs {
    pub host: String,
    #[serde(default = "default_bedrock_port")]
    pub port: u16,
}

fn default_bedrock_port() -> u16 {
    19132
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ShimEntitiesArgs {
    #[serde(default)]
    pub world: Option<String>,
    #[serde(default = "default_shim_limit")]
    pub limit: usize,
}

fn default_shim_limit() -> usize {
    100
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ShimEntityArgs {
    pub uuid: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ShimEntitySetArgs {
    pub uuid: String,
    pub op: String,
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
    #[serde(default)]
    pub z: Option<f64>,
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ShimExecArgs {
    pub command: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ScreenshotArgs {
    #[serde(default)]
    pub save_path: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct LauncherArgs {
    #[serde(default)]
    pub instance: Option<String>,
    #[serde(default = "default_launcher_mode")]
    pub mode: String,
    #[serde(default)]
    pub executable: Option<String>,
    #[serde(default)]
    pub args: Option<String>,
}

fn default_launcher_mode() -> String {
    "java".to_string()
}

#[derive(Clone, Default)]
struct Snapshot {
    connected: bool,
    username: String,
    has_position: bool,
    x: f64,
    y: f64,
    z: f64,
    ticks: u64,
    executing_path: bool,
    calculating_path: bool,
    chat: VecDeque<String>,
    client: Option<Client>,
}

#[derive(Clone, Component, Default)]
struct BotShared {
    inner: Arc<Mutex<Snapshot>>,
}

struct BotSession {
    shared: BotShared,
    task: tokio::task::JoinHandle<AppExit>,
    server: String,
}

struct JoinReq {
    username: String,
    address: String,
    shared: BotShared,
    reply: tokio::sync::oneshot::Sender<tokio::task::JoinHandle<AppExit>>,
}

struct BotRuntime {
    tx: tokio::sync::mpsc::UnboundedSender<JoinReq>,
}

fn spawn_bot_runtime() -> BotRuntime {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<JoinReq>();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("bot runtime");
        let local = tokio::task::LocalSet::new();
        local.block_on(&rt, async move {
            while let Some(req) = rx.recv().await {
                let handle = tokio::task::spawn_local(async move {
                    ClientBuilder::new()
                        .set_handler(handle_bot)
                        .set_state(req.shared.clone())
                        .start(
                            Account::offline(req.username.as_str()),
                            req.address.as_str(),
                        )
                        .await
                });
                let _ = req.reply.send(handle);
            }
        });
    });
    BotRuntime { tx }
}

struct ServerProc {
    child: tokio::process::Child,
    stdin: Option<tokio::process::ChildStdin>,
    pid: u32,
    kind: String,
    command: String,
}

#[derive(Clone)]
struct Reminecraft {
    windows_agent_url: String,
    shim_url: String,
    tool_router: ToolRouter<Reminecraft>,
    bot: Arc<Mutex<Option<BotSession>>>,
    bot_rt: Arc<Mutex<Option<BotRuntime>>>,
    server: Arc<Mutex<Option<ServerProc>>>,
}

async fn handle_bot(bot: Client, event: Event, state: BotShared) -> eyre::Result<()> {
    match event {
        Event::Init => {
            lock(&state.inner).client = Some(bot.clone());
        }
        Event::Login => {
            let mut s = lock(&state.inner);
            s.connected = true;
            s.username = bot.username();
        }
        Event::Tick => {
            let mut s = lock(&state.inner);
            if s.connected {
                s.ticks = bot.ticks_connected() as u64;
                s.executing_path = bot.is_executing_path();
                s.calculating_path = bot.is_calculating_path();
                let p = bot.position();
                s.has_position = s.ticks > 0;
                s.x = p.x;
                s.y = p.y;
                s.z = p.z;
            }
        }
        Event::Chat(m) => {
            let (sender, content) = m.split_sender_and_content();
            let line = match sender {
                Some(name) => format!("{name}: {content}"),
                None => content,
            };
            let mut s = lock(&state.inner);
            if s.chat.len() >= 20 {
                s.chat.pop_front();
            }
            s.chat.push_back(line);
        }
        _ => {}
    }
    Ok(())
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn take_bot_client(bot: &Arc<Mutex<Option<BotSession>>>) -> Result<Client, McpError> {
    let guard = lock(bot);
    let sess = guard
        .as_ref()
        .ok_or_else(|| McpError::internal_error("bot_not_joined", None))?;
    if sess.task.is_finished() {
        return Err(McpError::internal_error("bot_task_ended", None));
    }
    let snap = lock(&sess.shared.inner);
    snap.client
        .clone()
        .ok_or_else(|| McpError::internal_error("bot_not_spawned_yet", None))
}

#[tool_router]
impl Reminecraft {
    fn new(windows_agent_url: String, shim_url: String) -> Self {
        Self {
            windows_agent_url,
            shim_url,
            bot: Arc::new(Mutex::new(None)),
            bot_rt: Arc::new(Mutex::new(None)),
            server: Arc::new(Mutex::new(None)),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Baca N baris terakhir dari log server Minecraft (Paper/BDS).")]
    async fn server_logs(
        &self,
        Parameters(args): Parameters<ServerLogsArgs>,
    ) -> Result<CallToolResult, McpError> {
        let text = tokio::fs::read_to_string(&args.path).await.map_err(|e| {
            McpError::internal_error(
                "log_read_failed",
                Some(json!({ "path": args.path, "error": e.to_string() })),
            )
        })?;
        let lines: Vec<&str> = text.lines().collect();
        if lines.is_empty() {
            return Ok(CallToolResult::success(vec![ContentBlock::text(
                "(log kosong)".to_string(),
            )]));
        }
        let n = args.lines.clamp(1, lines.len());
        let tail = lines[lines.len() - n..].join("\n");
        Ok(CallToolResult::success(vec![ContentBlock::text(tail)]))
    }

    #[tool(description = "Jalankan server Minecraft sebagai proses background. kind: java (paper/vanilla/spigot/purpur/fabric) atau bedrock (bedrock_server.exe).")]
    async fn server_start(
        &self,
        Parameters(args): Parameters<ServerStartArgs>,
    ) -> Result<CallToolResult, McpError> {
        {
            let guard = lock(&self.server);
            if guard.is_some() {
                return Err(McpError::internal_error("server_already_running", None));
            }
        }
        let mut cmd = if args.kind == "bedrock" {
            let exe = args.executable.clone().ok_or_else(|| {
                McpError::internal_error("bedrock_needs_executable", None)
            })?;
            tokio::process::Command::new(exe)
        } else {
            let bin = args.executable.clone().unwrap_or(args.java.clone());
            let mut c = tokio::process::Command::new(bin);
            c.arg(format!("-Xmx{}", args.memory))
                .arg(format!("-Xms{}", args.memory))
                .arg("-jar")
                .arg(&args.jar);
            if args.nogui {
                c.arg("nogui");
            }
            c
        };
        cmd.current_dir(&args.workdir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|e| {
            McpError::internal_error(
                "server_spawn_failed",
                Some(json!({ "error": e.to_string() })),
            )
        })?;
        let pid = child.id().unwrap_or(0);
        let stdin = child.stdin.take();
        let logpath = std::path::Path::new(&args.workdir)
            .join("logs")
            .join("remine-console.log");
        if let Some(parent) = logpath.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        if let Ok(file) = tokio::fs::File::create(&logpath).await {
            if let Some(out) = child.stdout.take() {
                tokio::spawn(async move {
                    let mut r = tokio::io::BufReader::new(out);
                    let mut f = file;
                    let _ = tokio::io::copy(&mut r, &mut f).await;
                });
            }
        }
        let command = if args.kind == "bedrock" {
            args.executable.clone().unwrap_or_default()
        } else {
            format!(
                "{} -Xmx{} -Xms{} -jar {}{}",
                args.executable.clone().unwrap_or(args.java.clone()),
                args.memory,
                args.memory,
                args.jar,
                if args.nogui { " nogui" } else { "" }
            )
        };
        {
            let mut guard = lock(&self.server);
            *guard = Some(ServerProc {
                child,
                stdin,
                pid,
                kind: args.kind.clone(),
                command: command.clone(),
            });
        }
        Ok(CallToolResult::success(vec![ContentBlock::text(
            json!({ "started": true, "pid": pid, "kind": args.kind, "workdir": args.workdir, "command": command, "console_log": logpath.to_string_lossy() })
                .to_string(),
        )]))
    }

    #[tool(description = "Hentikan server yang dijalankan via server_start.")]
    async fn server_stop(&self) -> Result<CallToolResult, McpError> {
        let mut proc = {
            let mut guard = lock(&self.server);
            guard
                .take()
                .ok_or_else(|| McpError::internal_error("server_not_running", None))?
        };
        let pid = proc.pid;
        if let Err(e) = proc.child.kill().await {
            *lock(&self.server) = Some(proc);
            return Err(McpError::internal_error(
                "server_kill_failed",
                Some(json!({ "error": e.to_string() })),
            ));
        }
        let status = proc.child.wait().await.map_err(|e| {
            McpError::internal_error("server_wait_failed", Some(json!({ "error": e.to_string() })))
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(
            json!({ "stopped": true, "pid": pid, "exit_code": status.code() }).to_string(),
        )]))
    }

    #[tool(description = "Status proses server yang dijalankan via server_start.")]
    async fn server_status(&self) -> Result<CallToolResult, McpError> {
        let mut guard = lock(&self.server);
        match guard.as_mut() {
            None => Ok(CallToolResult::success(vec![ContentBlock::text(
                json!({ "running": false }).to_string(),
            )])),
            Some(proc) => match proc.child.try_wait().map_err(|e| {
                McpError::internal_error("server_poll_failed", Some(json!({ "error": e.to_string() })))
            })? {
                Some(status) => {
                    let pid = proc.pid;
                    *guard = None;
                    Ok(CallToolResult::success(vec![ContentBlock::text(
                        json!({ "running": false, "pid": pid, "exit_code": status.code() })
                            .to_string(),
                    )]))
                }
                None => Ok(CallToolResult::success(vec![ContentBlock::text(
                    json!({ "running": true, "pid": proc.pid, "kind": proc.kind, "command": proc.command })
                        .to_string(),
                )])),
            },
        }
    }

    #[tool(description = "Tulis command ke stdin console server. Universal: jalan di Bedrock dan Java (pengganti RCON).")]
    async fn server_console(
        &self,
        Parameters(args): Parameters<ServerConsoleArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut proc = {
            let mut guard = lock(&self.server);
            guard
                .take()
                .ok_or_else(|| McpError::internal_error("server_not_running", None))?
        };
        let line = format!("{}\n", args.command);
        let r = async {
            let stdin = proc
                .stdin
                .as_mut()
                .ok_or_else(|| McpError::internal_error("server_no_stdin", None))?;
            use tokio::io::AsyncWriteExt;
            stdin.write_all(line.as_bytes()).await.map_err(|e| {
                McpError::internal_error("console_write_failed", Some(json!({ "error": e.to_string() })))
            })?;
            stdin.flush().await.map_err(|e| {
                McpError::internal_error("console_write_failed", Some(json!({ "error": e.to_string() })))
            })?;
            Ok::<(), McpError>(())
        }
        .await;
        {
            let mut guard = lock(&self.server);
            *guard = Some(proc);
        }
        r?;
        Ok(CallToolResult::success(vec![ContentBlock::text(
            json!({ "sent": true, "command": args.command }).to_string(),
        )]))
    }

    #[tool(description = "Daftar file plugin/mods (.jar) di folder plugins atau mods server.")]
    async fn plugin_list(
        &self,
        Parameters(args): Parameters<PluginListArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut dir = tokio::fs::read_dir(&args.dir).await.map_err(|e| {
            McpError::internal_error(
                "plugin_dir_unreadable",
                Some(json!({ "dir": args.dir, "error": e.to_string() })),
            )
        })?;
        let mut items = Vec::new();
        while let Some(entry) = dir.next_entry().await.map_err(|e| {
            McpError::internal_error("plugin_dir_unreadable", Some(json!({ "error": e.to_string() })))
        })? {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".jar") && !name.ends_with(".jar.disabled") {
                continue;
            }
            let size = entry
                .metadata()
                .await
                .map(|m| m.len())
                .unwrap_or(0);
            items.push(json!({
                "file": name,
                "bytes": size,
                "disabled": name.ends_with(".disabled"),
            }));
        }
        items.sort_by(|a, b| a["file"].as_str().cmp(&b["file"].as_str()));
        Ok(CallToolResult::success(vec![ContentBlock::text(
            json!({ "dir": args.dir, "count": items.len(), "plugins": items }).to_string(),
        )]))
    }

    #[tool(description = "Daftar datapack (folder atau .zip) di folder datapacks sebuah world.")]
    async fn datapack_list(
        &self,
        Parameters(args): Parameters<DatapackListArgs>,
    ) -> Result<CallToolResult, McpError> {
        let dir = std::path::Path::new(&args.world).join("datapacks");
        let mut rd = tokio::fs::read_dir(&dir).await.map_err(|e| {
            McpError::internal_error(
                "datapack_dir_unreadable",
                Some(json!({ "dir": dir.to_string_lossy(), "error": e.to_string() })),
            )
        })?;
        let mut items = Vec::new();
        while let Some(entry) = rd.next_entry().await.map_err(|e| {
            McpError::internal_error("datapack_dir_unreadable", Some(json!({ "error": e.to_string() })))
        })? {
            let name = entry.file_name().to_string_lossy().to_string();
            let ft = entry.file_type().await.map_err(|e| {
                McpError::internal_error("datapack_dir_unreadable", Some(json!({ "error": e.to_string() })))
            })?;
            if !ft.is_dir() && !name.ends_with(".zip") {
                continue;
            }
            items.push(json!({
                "pack": name,
                "folder": ft.is_dir(),
                "disabled": name.starts_with(".") || name.ends_with(".disabled"),
            }));
        }
        items.sort_by(|a, b| a["pack"].as_str().cmp(&b["pack"].as_str()));
        Ok(CallToolResult::success(vec![ContentBlock::text(
            json!({ "dir": dir.to_string_lossy(), "count": items.len(), "datapacks": items }).to_string(),
        )]))
    }

    #[tool(description = "Kirim console command ke server via RCON.")]
    async fn rcon_command(
        &self,
        Parameters(args): Parameters<RconArgs>,
    ) -> Result<CallToolResult, McpError> {
        let out = rcon_send(&args.host, args.port, &args.password, &args.command)
            .await
            .map_err(|e| {
                McpError::internal_error("rcon_failed", Some(json!({ "error": e.to_string() })))
            })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(out)]))
    }

    #[tool(description = "Join bot player (azalea, offline auth) ke server Java Edition.")]
    async fn bot_join(
        &self,
        Parameters(args): Parameters<BotJoinArgs>,
    ) -> Result<CallToolResult, McpError> {
        {
            let guard = lock(&self.bot);
            if guard.is_some() {
                return Err(McpError::internal_error("bot_already_joined", None));
            }
        }
        let tx = {
            let mut rt = lock(&self.bot_rt);
            if rt.is_none() {
                *rt = Some(spawn_bot_runtime());
            }
            rt.as_ref().unwrap().tx.clone()
        };
        let shared = BotShared::default();
        let (rep_tx, rep_rx) = tokio::sync::oneshot::channel();
        tx.send(JoinReq {
            username: args.username.clone(),
            address: args.address.clone(),
            shared: shared.clone(),
            reply: rep_tx,
        })
        .map_err(|_| McpError::internal_error("bot_runtime_gone", None))?;
        let handle = rep_rx
            .await
            .map_err(|_| McpError::internal_error("bot_spawn_failed", None))?;
        {
            let mut guard = lock(&self.bot);
            *guard = Some(BotSession {
                shared,
                task: handle,
                server: args.address.clone(),
            });
        }
        Ok(CallToolResult::success(vec![ContentBlock::text(
            json!({ "joining": true, "server": args.address, "username": args.username }).to_string(),
        )]))
    }

    #[tool(description = "Keluarkan bot dari server.")]
    async fn bot_leave(&self) -> Result<CallToolResult, McpError> {
        let mut guard = lock(&self.bot);
        let sess = guard
            .take()
            .ok_or_else(|| McpError::internal_error("bot_not_joined", None))?;
        sess.task.abort();
        Ok(CallToolResult::success(vec![ContentBlock::text(
            json!({ "left": true, "server": sess.server }).to_string(),
        )]))
    }

    #[tool(description = "Kirim pesan chat / command player lewat bot. Awali dengan / untuk command.")]
    async fn bot_chat(
        &self,
        Parameters(args): Parameters<BotChatArgs>,
    ) -> Result<CallToolResult, McpError> {
        let client = take_bot_client(&self.bot)?;
        client.chat(args.message.clone());
        Ok(CallToolResult::success(vec![ContentBlock::text(
            json!({ "sent": true, "message": args.message }).to_string(),
        )]))
    }

    #[tool(description = "Jalankan bot ke koordinat blok x y z dengan pathfinder.")]
    async fn bot_goto(
        &self,
        Parameters(args): Parameters<BotGotoArgs>,
    ) -> Result<CallToolResult, McpError> {
        let client = take_bot_client(&self.bot)?;
        client.start_goto(BlockPosGoal(BlockPos::new(args.x, args.y, args.z)));
        Ok(CallToolResult::success(vec![ContentBlock::text(
            json!({ "going": true, "x": args.x, "y": args.y, "z": args.z }).to_string(),
        )]))
    }

    #[tool(description = "Hentikan pathfinder bot seketika.")]
    async fn bot_stop(&self) -> Result<CallToolResult, McpError> {
        let client = take_bot_client(&self.bot)?;
        client.force_stop_pathfinding();
        Ok(CallToolResult::success(vec![ContentBlock::text(
            json!({ "stopped": true }).to_string(),
        )]))
    }

    #[tool(description = "Suruh bot menghancurkan blok di koordinat x y z.")]
    async fn bot_mine(
        &self,
        Parameters(args): Parameters<BotMineArgs>,
    ) -> Result<CallToolResult, McpError> {
        let client = take_bot_client(&self.bot)?;
        client.mine(BlockPos::new(args.x, args.y, args.z)).await;
        Ok(CallToolResult::success(vec![ContentBlock::text(
            json!({ "mined": true, "x": args.x, "y": args.y, "z": args.z }).to_string(),
        )]))
    }

    #[tool(description = "Status bot: koneksi, posisi, pathfinder, chat terakhir.")]
    async fn bot_status(&self) -> Result<CallToolResult, McpError> {
        let guard = lock(&self.bot);
        match guard.as_ref() {
            None => Ok(CallToolResult::success(vec![ContentBlock::text(
                json!({ "joined": false }).to_string(),
            )])),
            Some(sess) => {
                let snap = lock(&sess.shared.inner).clone();
                Ok(CallToolResult::success(vec![ContentBlock::text(
                    json!({
                        "joined": true,
                        "alive": !sess.task.is_finished(),
                        "server": sess.server,
                        "username": snap.username,
                        "connected": snap.connected,
                        "position": if snap.has_position {
                            Some(json!({ "x": snap.x, "y": snap.y, "z": snap.z }))
                        } else { None },
                        "ticks": snap.ticks,
                        "executing_path": snap.executing_path,
                        "calculating_path": snap.calculating_path,
                        "recent_chat": snap.chat.iter().collect::<Vec<_>>(),
                    })
                    .to_string(),
                )]))
            }
        }
    }

    #[tool(description = "Ping server Bedrock (RakNet unconnected ping), baca MOTD dan jumlah player.")]
    async fn bedrock_ping(
        &self,
        Parameters(args): Parameters<BedrockPingArgs>,
    ) -> Result<CallToolResult, McpError> {
        let out = bedrock_ping_udp(&args.host, args.port).await.map_err(|e| {
            McpError::internal_error("bedrock_ping_failed", Some(json!({ "error": e.to_string() })))
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(out.to_string())]))
    }

    #[tool(description = "Ambil screenshot layar lewat windows-agent, kembalikan sebagai gambar.")]
    async fn screenshot(
        &self,
        Parameters(args): Parameters<ScreenshotArgs>,
    ) -> Result<CallToolResult, McpError> {
        let url = format!("{}/screenshot", self.windows_agent_url.trim_end_matches('/'));
        let res = reqwest::Client::new().get(&url).send().await.map_err(|e| {
            McpError::internal_error(
                "windows_agent_unreachable",
                Some(json!({ "error": e.to_string() })),
            )
        })?;
        if !res.status().is_success() {
            return Err(McpError::internal_error(
                "screenshot_failed",
                Some(json!({ "http": res.status().as_u16() })),
            ));
        }
        let v: serde_json::Value = res.json().await.map_err(|e| {
            McpError::internal_error("screenshot_bad_response", Some(json!({ "error": e.to_string() })))
        })?;
        let b64 = v
            .get("image_base64")
            .and_then(|x| x.as_str())
            .ok_or_else(|| McpError::internal_error("screenshot_bad_response", None))?
            .to_string();
        let width = v.get("width").and_then(|x| x.as_u64()).unwrap_or(0);
        let height = v.get("height").and_then(|x| x.as_u64()).unwrap_or(0);
        let mut blocks = vec![ContentBlock::image(b64.clone(), "image/png")];
        let mut caption = format!("{width}x{height}");
        if let Some(path) = args.save_path {
            let raw = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &b64)
                .map_err(|e| {
                    McpError::internal_error(
                        "screenshot_decode_failed",
                        Some(json!({ "error": e.to_string() })),
                    )
                })?;
            tokio::fs::write(&path, raw).await.map_err(|e| {
                McpError::internal_error(
                    "screenshot_save_failed",
                    Some(json!({ "error": e.to_string() })),
                )
            })?;
            caption = format!("{caption} -> {path}");
        }
        blocks.push(ContentBlock::text(caption));
        Ok(CallToolResult::success(blocks))
    }

    #[tool(description = "Jalankan game/launcher lewat windows-agent.")]
    async fn launcher_launch(
        &self,
        Parameters(args): Parameters<LauncherArgs>,
    ) -> Result<CallToolResult, McpError> {
        let url = format!("{}/launch", self.windows_agent_url.trim_end_matches('/'));
        let res = reqwest::Client::new()
            .post(&url)
            .json(&json!({
                "instance": args.instance,
                "mode": args.mode,
                "executable": args.executable,
                "args": args.args,
            }))
            .send()
            .await
            .map_err(|e| {
                McpError::internal_error(
                    "windows_agent_unreachable",
                    Some(json!({ "error": e.to_string() })),
                )
            })?;
        let http = res.status().as_u16();
        let body = res.text().await.unwrap_or_default();
        if http >= 400 {
            return Err(McpError::internal_error(
                "launcher_failed",
                Some(json!({ "http": http, "body": body })),
            ));
        }
        Ok(CallToolResult::success(vec![ContentBlock::text(body)]))
    }

    async fn shim_get(&self, path: &str) -> Result<String, McpError> {
        let url = format!("{}{}", self.shim_url.trim_end_matches('/'), path);
        let res = reqwest::Client::new().get(&url).send().await.map_err(|e| {
            McpError::internal_error("shim_unreachable", Some(json!({ "error": e.to_string() })))
        })?;
        let http = res.status().as_u16();
        let body = res.text().await.unwrap_or_default();
        if http >= 400 {
            return Err(McpError::internal_error(
                "shim_failed",
                Some(json!({ "http": http, "body": body })),
            ));
        }
        Ok(body)
    }

    async fn shim_post(&self, path: &str, payload: serde_json::Value) -> Result<String, McpError> {
        let url = format!("{}{}", self.shim_url.trim_end_matches('/'), path);
        let res = reqwest::Client::new()
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                McpError::internal_error("shim_unreachable", Some(json!({ "error": e.to_string() })))
            })?;
        let http = res.status().as_u16();
        let body = res.text().await.unwrap_or_default();
        if http >= 400 {
            return Err(McpError::internal_error(
                "shim_failed",
                Some(json!({ "http": http, "body": body })),
            ));
        }
        Ok(body)
    }

    #[tool(description = "Daftar player online dengan posisi, health, gamemode via RemineShim.")]
    async fn shim_players(&self) -> Result<CallToolResult, McpError> {
        let body = self.shim_get("/players").await?;
        Ok(CallToolResult::success(vec![ContentBlock::text(body)]))
    }

    #[tool(description = "Daftar entity di world via RemineShim (ganti get_objects scene).")]
    async fn shim_entities(
        &self,
        Parameters(args): Parameters<ShimEntitiesArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut path = format!("/entities?limit={}", args.limit);
        if let Some(w) = args.world {
            if !w.is_empty() {
                path = format!("{path}&world={w}");
            }
        }
        let body = self.shim_get(&path).await?;
        Ok(CallToolResult::success(vec![ContentBlock::text(body)]))
    }

    #[tool(description = "Baca data satu entity by uuid via RemineShim (ganti get_property).")]
    async fn shim_entity(
        &self,
        Parameters(args): Parameters<ShimEntityArgs>,
    ) -> Result<CallToolResult, McpError> {
        let body = self.shim_get(&format!("/entity?uuid={}", args.uuid)).await?;
        Ok(CallToolResult::success(vec![ContentBlock::text(body)]))
    }

    #[tool(description = "Ubah entity by uuid via RemineShim. op: tp, remove, name, glow, velocity.")]
    async fn shim_entity_set(
        &self,
        Parameters(args): Parameters<ShimEntitySetArgs>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .shim_post(
                "/entity",
                json!({
                    "uuid": args.uuid,
                    "op": args.op,
                    "x": args.x,
                    "y": args.y,
                    "z": args.z,
                    "value": args.value,
                }),
            )
            .await?;
        Ok(CallToolResult::success(vec![ContentBlock::text(body)]))
    }

    #[tool(description = "Jalankan console command in-process via RemineShim (tanpa RCON).")]
    async fn shim_exec(
        &self,
        Parameters(args): Parameters<ShimExecArgs>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .shim_post("/exec", json!({ "command": args.command }))
            .await?;
        Ok(CallToolResult::success(vec![ContentBlock::text(body)]))
    }

    #[tool(description = "Cek windows-agent C# sidecar hidup atau tidak.")]
    async fn windows_agent_ping(&self) -> Result<CallToolResult, McpError> {
        let url = format!("{}/health", self.windows_agent_url.trim_end_matches('/'));
        let res = reqwest::Client::new().get(&url).send().await.map_err(|e| {
            McpError::internal_error(
                "windows_agent_unreachable",
                Some(json!({ "error": e.to_string() })),
            )
        })?;
        let body = res.text().await.unwrap_or_default();
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "windows-agent OK: {body}"
        ))]))
    }
}

async fn rcon_send(host: &str, port: u16, password: &str, command: &str) -> Result<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    async fn packet(stream: &mut TcpStream, id: i32, kind: i32, body: &str) -> Result<()> {
        let mut buf = Vec::new();
        let len = (body.len() + 10) as i32;
        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend_from_slice(&id.to_le_bytes());
        buf.extend_from_slice(&kind.to_le_bytes());
        buf.extend_from_slice(body.as_bytes());
        buf.push(0);
        buf.push(0);
        stream.write_all(&buf).await?;
        Ok(())
    }

    async fn read_packet(stream: &mut TcpStream) -> Result<(i32, i32, String)> {
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await?;
        let len = i32::from_le_bytes(len_buf) as usize;
        let mut data = vec![0u8; len];
        stream.read_exact(&mut data).await?;
        let id = i32::from_le_bytes(data[0..4].try_into().unwrap());
        let kind = i32::from_le_bytes(data[4..8].try_into().unwrap());
        let body = String::from_utf8_lossy(&data[8..data.len() - 2]).to_string();
        Ok((id, kind, body))
    }

    let addr = format!("{host}:{port}");
    let mut stream = TcpStream::connect(&addr).await?;
    packet(&mut stream, 1, 3, password).await?;
    let (id, _, _) = read_packet(&mut stream).await?;
    if id == -1 {
        anyhow::bail!("RCON auth failed (id=-1), cek rcon.password di server.properties");
    }
    packet(&mut stream, 2, 2, command).await?;
    let (_, _, body) = read_packet(&mut stream).await?;
    Ok(body)
}

const RAKNET_MAGIC: [u8; 16] = [
    0x00, 0xFF, 0xFF, 0x00, 0xFE, 0xFE, 0xFE, 0xFE, 0xFD, 0xFD, 0xFD, 0xFD, 0x12, 0x34,
    0x56, 0x78,
];

async fn bedrock_ping_udp(host: &str, port: u16) -> Result<serde_json::Value> {
    use tokio::net::UdpSocket;
    use tokio::time::{Duration, timeout};

    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    sock.connect(format!("{host}:{port}")).await?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let mut pkt = vec![0x01u8];
    pkt.extend_from_slice(&now.to_be_bytes());
    pkt.extend_from_slice(&RAKNET_MAGIC);
    pkt.extend_from_slice(&0x123456789ABCDEFu64.to_be_bytes());
    sock.send(&pkt).await?;

    let mut buf = vec![0u8; 2048];
    let n = timeout(Duration::from_secs(3), sock.recv(&mut buf))
        .await
        .map_err(|_| anyhow::anyhow!("timeout, tidak ada respon RakNet dari {host}:{port}"))??;
    if n < 35 || buf[0] != 0x1C {
        anyhow::bail!("respon bukan unconnected pong");
    }
    let len = u16::from_be_bytes([buf[33], buf[34]]) as usize;
    let start = 35usize;
    let end = if len > 0 && start + len <= n {
        start + len
    } else {
        n
    };
    let motd = String::from_utf8_lossy(&buf[start..end]).to_string();
    let f: Vec<&str> = motd.split(';').collect();
    let get = |i: usize| f.get(i).unwrap_or(&"").to_string();
    Ok(json!({
        "online": true,
        "host": host,
        "port": port,
        "motd": get(1),
        "protocol": get(2).parse::<i64>().unwrap_or(0),
        "version": get(3),
        "players_online": get(4).parse::<i64>().unwrap_or(0),
        "players_max": get(5).parse::<i64>().unwrap_or(0),
        "gamemode": get(8),
        "raw": motd,
    }))
}

#[tool_handler]
impl ServerHandler for Reminecraft {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "reminecraft-mcp",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Reminecraft MCP. Java: server_logs, server_start(kind java: paper/vanilla/spigot/purpur/fabric), server_stop, server_status, server_console, plugin_list, datapack_list, rcon_command, bot_join, bot_leave, bot_chat, bot_goto, bot_stop, bot_mine, bot_status. Bedrock: server_start(kind bedrock), server_console, server_logs, bedrock_ping. Shim: shim_players, shim_entities, shim_entity, shim_entity_set, shim_exec. Windows: screenshot, launcher_launch, windows_agent_ping. Transport stdio lokal atau streamable-http /mcp remote."
                    .to_string(),
            )
    }
}

async fn serve_stdio(agent_url: String, shim_url: String) -> Result<()> {
    let service = Reminecraft::new(agent_url, shim_url).serve(stdio()).await.inspect_err(|e| {
        tracing::error!("serving error: {:?}", e);
    })?;
    service.waiting().await?;
    Ok(())
}

async fn serve_http(agent_url: String, shim_url: String, host: &str, port: u16) -> Result<()> {
    let bind = format!("{host}:{port}");
    let ct = tokio_util::sync::CancellationToken::new();
    let service = StreamableHttpService::new(
        move || Ok(Reminecraft::new(agent_url.clone(), shim_url.clone())),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default().with_cancellation_token(ct.child_token()),
    );
    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("Reminecraft MCP HTTP ready at http://{bind}/mcp");
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            ct.cancel();
        })
        .await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
    match args.transport {
        TransportKind::Stdio => {
            tracing::info!("transport=stdio");
            serve_stdio(args.windows_agent, args.shim).await
        }
        TransportKind::Http => {
            tracing::info!("transport=http {}:{}", args.host, args.port);
            serve_http(args.windows_agent, args.shim, &args.host, args.port).await
        }
    }
}
