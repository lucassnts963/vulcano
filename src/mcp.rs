//! Cliente MCP (Model Context Protocol) sobre stdio.
//!
//! Lê `~/.config/vulcano/mcp.json`, sobe cada servidor como processo filho,
//! faz o handshake JSON-RPC, pergunta `tools/list` e transforma cada tool
//! anunciada num [`Tool`] cujo handler faz `tools/call` pelo mesmo processo.
//! O resultado é um [`Registry`] que os modos somam ao seu (`Registry::extend`).
//!
//! Formato do `mcp.json` (igual ao de Claude Code / opencode):
//!
//! ```json
//! {
//!   "mcpServers": {
//!     "fs": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-filesystem", "."] }
//!   }
//! }
//! ```

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

use crate::Result;
use crate::config;
use crate::tools::{Registry, Tool};

const PROTOCOL_VERSION: &str = "2024-11-05";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Deserialize)]
struct McpFile {
    #[serde(default, rename = "mcpServers")]
    servers: BTreeMap<String, ServerCfg>,
}

#[derive(Deserialize)]
struct ServerCfg {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
}

/// Lê o `mcp.json` e devolve as tools de todos os servidores que subiram.
///
/// Nunca falha o modo: arquivo ausente = registro vazio; servidor com problema
/// = aviso no stderr e segue sem ele.
pub async fn load_all() -> Registry {
    let path = config::mcp_path();
    let raw = match std::fs::read_to_string(&path) {
        Ok(r) => r,
        Err(_) => return Registry::new(),
    };

    let file: McpFile = match serde_json::from_str(&raw) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[mcp] {} inválido: {e}", path.display());
            return Registry::new();
        }
    };

    let mut registry = Registry::new();
    for (nome, cfg) in file.servers {
        match connect(&nome, &cfg).await {
            Ok(reg) => {
                eprintln!("[mcp] '{nome}': {} ferramenta(s)", reg.names().len());
                registry.extend(reg);
            }
            Err(e) => eprintln!("[mcp] '{nome}' falhou: {e}"),
        }
    }
    registry
}

async fn connect(nome: &str, cfg: &ServerCfg) -> Result<Registry> {
    let mut child = Command::new(&cfg.command)
        .args(&cfg.args)
        .envs(&cfg.env)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("não consegui iniciar '{}': {e}", cfg.command))?;

    let stdin = child.stdin.take().ok_or("processo sem stdin")?;
    let stdout = child.stdout.take().ok_or("processo sem stdout")?;

    let mut conn = Conn {
        _child: child,
        stdin,
        reader: BufReader::new(stdout),
        next_id: 0,
    };

    conn.request(
        "initialize",
        json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "vulcano", "version": env!("CARGO_PKG_VERSION") }
        }),
    )
    .await?;
    conn.notify("notifications/initialized", json!({})).await?;

    let listed = conn.request("tools/list", json!({})).await?;
    let anunciadas = listed
        .get("tools")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let shared = Arc::new(Mutex::new(conn));
    let mut registry = Registry::new();

    for t in anunciadas {
        let Some(orig) = t.get("name").and_then(Value::as_str) else {
            continue;
        };
        let descricao = t
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let schema = t
            .get("inputSchema")
            .cloned()
            .unwrap_or_else(|| json!({ "type": "object" }));

        // Prefixa com o nome do servidor pra evitar colisão entre servidores.
        let exposto = format!("{nome}_{orig}");
        let orig = orig.to_string();
        let conn = Arc::clone(&shared);

        registry.register(Tool::new(exposto, descricao, schema, move |args: Value| {
            let conn = Arc::clone(&conn);
            let orig = orig.clone();
            async move {
                let res = {
                    let mut c = conn.lock().await;
                    c.request("tools/call", json!({ "name": orig, "arguments": args })).await?
                };
                Ok(render(&res))
            }
        }));
    }

    Ok(registry)
}

/// Extrai o texto do `result` de um `tools/call`.
fn render(res: &Value) -> String {
    let texto = res
        .get("content")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|i| i.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| res.to_string());

    if res.get("isError").and_then(Value::as_bool).unwrap_or(false) {
        format!("erro: {texto}")
    } else {
        texto
    }
}

/// Conexão viva com um servidor MCP. JSON-RPC delimitado por linha.
struct Conn {
    _child: Child, // mantido vivo; morre junto (kill_on_drop)
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    next_id: i64,
}

impl Conn {
    async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        tokio::time::timeout(REQUEST_TIMEOUT, self.request_inner(method, params))
            .await
            .map_err(|_| format!("timeout em '{method}'"))?
    }

    async fn request_inner(&mut self, method: &str, params: Value) -> Result<Value> {
        self.next_id += 1;
        let id = self.next_id;
        self.write(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
            .await?;

        loop {
            let linha = self.read_line().await?;
            let Ok(msg) = serde_json::from_str::<Value>(linha.trim()) else {
                continue;
            };
            if msg.get("id").and_then(Value::as_i64) != Some(id) {
                continue; // notificação ou outra mensagem
            }
            if let Some(err) = msg.get("error") {
                return Err(format!("erro JSON-RPC: {err}").into());
            }
            return Ok(msg.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    async fn notify(&mut self, method: &str, params: Value) -> Result<()> {
        self.write(&json!({ "jsonrpc": "2.0", "method": method, "params": params })).await
    }

    async fn write(&mut self, msg: &Value) -> Result<()> {
        let mut linha = serde_json::to_string(msg)?;
        linha.push('\n');
        self.stdin.write_all(linha.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    async fn read_line(&mut self) -> Result<String> {
        let mut buf = String::new();
        if self.reader.read_line(&mut buf).await? == 0 {
            return Err("servidor fechou a saída".into());
        }
        Ok(buf)
    }
}
