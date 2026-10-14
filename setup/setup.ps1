param(
  [string]$ServerDir = "C:\Reminecraft-Server",
  [string]$ServerKind = "paper",
  [string]$McVersion = "26.1.2",
  [string]$RconPassword = "remine-test",
  [string]$BedrockUrl = ""
)
$ErrorActionPreference = "Stop"
$ROOT = Split-Path -Parent $PSScriptRoot
$UA = "remincraft-mcp-setup/0.1.0 (local install)"

function Need($cmd, $hint) {
  if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) {
    throw "butuh $cmd. $hint"
  }
}
Need "cargo" "pasang dari https://rustup.rs"
Need "dotnet" "pasang .NET 10 SDK"
Need "node" "pasang Node 18+"

$exe = "$ROOT\reminecraft-mcp\target\debug\reminecraft-mcp.exe"
if (-not (Test-Path $exe)) {
  cargo build --manifest-path "$ROOT\reminecraft-mcp\Cargo.toml"
} else {
  Write-Output "rust ok: $exe"
}
dotnet build "$ROOT\windows-agent\windows-agent.csproj" -v minimal | Select-Object -Last 2

$isBedrock = $ServerKind -eq "bedrock"
$isBukkit = ($ServerKind -eq "paper") -or ($ServerKind -eq "purpur")

$javaBin = "$ServerDir\jdk-25\bin\java.exe"
if (-not $isBedrock) {
if (-not (Test-Path $javaBin)) {
  New-Item -ItemType Directory -Force $ServerDir | Out-Null
  $zip = "$env:TEMP\temurin25-jdk.zip"
  if (-not (Test-Path $zip)) {
    curl.exe -sL -m 900 "https://api.adoptium.net/v3/binary/latest/25/ga/windows/x64/jdk/hotspot/normal/eclipse" -o $zip
  }
  $tmpJdk = "$env:TEMP\remine-jdk"
  Remove-Item -Recurse -Force $tmpJdk -ErrorAction SilentlyContinue
  Expand-Archive -Path $zip -DestinationPath $tmpJdk -Force
  $src = Get-ChildItem $tmpJdk -Directory | Where-Object { $_.Name -like "jdk-25*" } | Select-Object -First 1
  Copy-Item -Recurse -Force $src.FullName "$ServerDir\jdk-25"
} else {
  Write-Output "jdk ok: $javaBin"
}
}

if ($isBedrock) {
  if ([string]::IsNullOrWhiteSpace($BedrockUrl)) {
    throw "kind bedrock butuh -BedrockUrl (link zip BDS Windows dari minecraft.net/download/server/bedrock)"
  }
  if (-not (Test-Path "$ServerDir\bedrock_server.exe")) {
    $zip = "$env:TEMP\bds.zip"
    curl.exe -sL -m 600 $BedrockUrl -o $zip
    Expand-Archive -Path $zip -DestinationPath $ServerDir -Force
    Write-Output "bedrock ok"
  } else {
    Write-Output "bedrock ok"
  }
} elseif ($ServerKind -eq "vanilla") {
  if (-not (Test-Path "$ServerDir\server.jar")) {
    $manifest = curl.exe -s -m 30 "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json" | ConvertFrom-Json
    $ver = $manifest.versions | Where-Object { $_.id -eq $McVersion } | Select-Object -First 1
    if (-not $ver) { throw "vanilla $McVersion tidak ada di manifest Mojang" }
    $detail = curl.exe -s -m 30 $ver.url | ConvertFrom-Json
    curl.exe -sL -m 600 $detail.downloads.server.url -o "$ServerDir\server.jar"
    Write-Output "vanilla $McVersion ok"
  } else {
    Write-Output "vanilla ok"
  }
} elseif ($ServerKind -eq "purpur") {
  if (-not (Test-Path "$ServerDir\purpur.jar")) {
    $url = "https://api.purpurmc.org/v2/purpur/$McVersion/latest/download"
    curl.exe -sL -m 600 -H "User-Agent: $UA" $url -o "$ServerDir\purpur.jar"
    Write-Output "purpur $McVersion ok"
  } else {
    Write-Output "purpur ok"
  }
} else {
  New-Item -ItemType Directory -Force "$ServerDir\plugins" | Out-Null
  if (-not (Test-Path "$ServerDir\paper.jar")) {
    $builds = curl.exe -s -m 30 -H "User-Agent: $UA" "https://fill.papermc.io/v3/projects/paper/versions/$McVersion/builds" | ConvertFrom-Json
    $stable = $builds | Where-Object { $_.channel -eq "STABLE" } | Sort-Object id -Descending | Select-Object -First 1
    if (-not $stable) { throw "tidak ada build STABLE paper $McVersion" }
    $url = $stable.downloads."server:default".url
    Write-Output "paper $McVersion build $($stable.id)"
    curl.exe -sL -m 600 -H "User-Agent: $UA" $url -o "$ServerDir\paper.jar"
  } else {
    Write-Output "paper ok"
  }
}

if ($isBukkit) {

$lib = "$ROOT\plugin\lib"
New-Item -ItemType Directory -Force $lib | Out-Null
$deps = @(
  @("https://repo.papermc.io/repository/maven-public/io/papermc/paper/paper-api/26.1.2.build.74-stable/paper-api-26.1.2.build.74-stable.jar", "paper-api.jar"),
  @("https://repo.papermc.io/repository/maven-public/net/kyori/adventure-api/4.26.1/adventure-api-4.26.1.jar", "adventure-api.jar"),
  @("https://repo.papermc.io/repository/maven-public/net/kyori/adventure-key/4.26.1/adventure-key-4.26.1.jar", "adventure-key.jar"),
  @("https://repo.papermc.io/repository/maven-public/net/md-5/bungeecord-chat/1.21-R0.2-deprecated+build.21/bungeecord-chat-1.21-R0.2-deprecated+build.21.jar", "bungee-chat.jar"),
  @("https://repo1.maven.org/maven2/com/google/code/gson/gson/2.13.2/gson-2.13.2.jar", "gson.jar")
)
foreach ($d in $deps) {
  if (-not (Test-Path "$lib\$($d[1])")) {
    curl.exe -sL -m 300 -H "User-Agent: $UA" $d[0] -o "$lib\$($d[1])"
  }
}

$javac = "$ServerDir\jdk-25\bin\javac.exe"
$jar = "$ServerDir\jdk-25\bin\jar.exe"
$cp = "$lib\*"
foreach ($p in @(@("plugin", "com\remine\test", "RemineTest"), @("plugin-shim", "com\remine\shim", "RemineShim"))) {
  $srcFile = "$ROOT\$($p[0])\src\$($p[1])\$($p[2]).java"
  $clsDir = "$ROOT\$($p[0])\classes"
  Remove-Item -Recurse -Force $clsDir -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Force $clsDir | Out-Null
  & $javac -cp $cp -d $clsDir $srcFile
  Copy-Item "$ROOT\$($p[0])\src\*.yml" $clsDir -Force
  & $jar --create --file "$ServerDir\plugins\$($p[2]).jar" -C $clsDir .
  Write-Output "plugin ok: $($p[2]).jar"
}
}

if (-not (Test-Path "$ServerDir\eula.txt")) {
  Set-Content "$ServerDir\eula.txt" "eula=true`n"
}
if (-not (Test-Path "$ServerDir\server.properties")) {
  if ($isBedrock) {
    Set-Content "$ServerDir\server.properties" @"
server-name=Remine Bedrock
gamemode=creative
force-gamemode=false
difficulty=peaceful
allow-cheats=true
max-players=5
online-mode=false
allow-list=false
server-port=19132
server-portv6=19133
enable-lan-visibility=true
"@
  } else {
    Set-Content "$ServerDir\server.properties" @"
server-port=25565
online-mode=false
enable-rcon=true
rcon.password=$RconPassword
rcon.port=25575
broadcast-rcon-to-ops=false
motd=Remine Test Server
max-players=5
gamemode=creative
difficulty=peaceful
allow-nether=false
view-distance=4
simulation-distance=4
spawn-protection=0
white-list=false
enforce-whitelist=false
"@
  }
}

function Merge-JsonFile($file, $script) {
  $obj = @{}
  if (Test-Path $file) {
    try { $obj = Get-Content $file -Raw | ConvertFrom-Json -AsHashtable } catch { $obj = @{} }
    Copy-Item $file "$file.bak" -Force
  } else {
    New-Item -ItemType Directory -Force (Split-Path -Parent $file) | Out-Null
  }
  $obj = & $script $obj
  $obj | ConvertTo-Json -Depth 10 | Set-Content $file
  Write-Output "config ok: $file"
}

Merge-JsonFile "$env:APPDATA\Claude\claude_desktop_config.json" {
  param($o)
  if (-not $o["mcpServers"]) { $o["mcpServers"] = @{} }
  $o["mcpServers"]["reminecraft"] = @{ command = $exe; args = @("--transport", "stdio") }
  $o
}

Merge-JsonFile "$env:USERPROFILE\.config\opencode\opencode.json" {
  param($o)
  if (-not $o["mcp"]) { $o["mcp"] = @{} }
  $o["mcp"]["reminecraft"] = @{ type = "local"; command = @($exe, "--transport", "stdio"); enabled = $true }
  $o
}

Write-Output "SELESAI. server=$ServerDir rcon=$RconPassword"
Write-Output "jalan agent: dotnet run --project $ROOT\windows-agent --urls http://127.0.0.1:5123"
