use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{Result, config};

pub const DEFAULT_MODEL: &str = "deepseek-v4-flash";
pub const DEFAULT_SYSTEM_PROMPT: &str = "Você é um assistente prestativo e fala português.";

/// Uma mensagem da conversa.
///
/// `system` / `user` / `assistant` carregam `content`. O `assistant` também
/// pode pedir ferramentas via `tool_calls`. A resposta de uma ferramenta usa
/// `role = "tool"` + `tool_call_id`.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Message {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".into(), content: Some(content.into()), ..Default::default() }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: Some(content.into()), ..Default::default() }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".into(), content: Some(content.into()), ..Default::default() }
    }

    /// Resposta de uma ferramenta, amarrada ao `tool_call_id` que a pediu.
    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".into(),
            content: Some(content.into()),
            tool_call_id: Some(tool_call_id.into()),
            ..Default::default()
        }
    }

    pub fn text(&self) -> &str {
        self.content.as_deref().unwrap_or_default()
    }
}

/// Um pedido de chamada de função vindo do modelo.
#[derive(Serialize, Deserialize, Clone)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: FunctionCall,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FunctionCall {
    pub name: String,
    /// Argumentos como string JSON (formato da API).
    pub arguments: String,
}

// --- streaming (SSE) --------------------------------------------------------

#[derive(Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Deserialize)]
struct StreamChoice {
    delta: Delta,
}

#[derive(Deserialize, Default)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Deserialize)]
struct ToolCallDelta {
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<FunctionDelta>,
}

#[derive(Deserialize, Default)]
struct FunctionDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

/// Ponto único de contato com a API. Faz a requisição em modo streaming, chama
/// `on_delta` a cada fragmento de texto e devolve a mensagem do assistente
/// montada (texto e/ou `tool_calls`).
///
/// Incrementos futuros (troca de modelo/base_url, outros providers) entram aqui.
pub async fn stream_complete(
    messages: &[Message],
    tools: &[Value],
    on_delta: &mut dyn FnMut(&str),
) -> Result<Message> {
    let api_key = config::env_or_file("DEEPSEEK_API_KEY").ok_or(
        "DEEPSEEK_API_KEY não encontrada. Defina no ambiente (export DEEPSEEK_API_KEY=sk-...) \
         ou em ~/.config/vulcano/env (linha: DEEPSEEK_API_KEY=sk-...)",
    )?;

    let mut body = json!({
        "model": DEFAULT_MODEL,
        "messages": messages,
        "stream": true,
    });
    if !tools.is_empty() {
        body["tools"] = Value::from(tools.to_vec());
    }

    let resp = reqwest::Client::new()
        .post("https://api.deepseek.com/chat/completions")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let corpo = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP {status}: {corpo}").into());
    }

    let mut content = String::new();
    let mut calls: Vec<CallBuilder> = Vec::new();

    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();

    while let Some(chunk) = stream.next().await {
        buf.extend_from_slice(&chunk?);

        while let Some(nl) = buf.iter().position(|&b| b == b'\n') {
            let linha: Vec<u8> = buf.drain(..=nl).collect();
            let linha = String::from_utf8_lossy(&linha[..linha.len() - 1]);
            let Some(dados) = linha.trim_end().strip_prefix("data: ") else {
                continue;
            };
            if dados == "[DONE]" {
                return Ok(montar(content, calls));
            }
            let Ok(parsed) = serde_json::from_str::<StreamChunk>(dados) else {
                continue;
            };
            let Some(choice) = parsed.choices.into_iter().next() else {
                continue;
            };

            if let Some(txt) = choice.delta.content
                && !txt.is_empty()
            {
                on_delta(&txt);
                content.push_str(&txt);
            }
            for tc in choice.delta.tool_calls.unwrap_or_default() {
                if calls.len() <= tc.index {
                    calls.resize_with(tc.index + 1, CallBuilder::default);
                }
                let slot = &mut calls[tc.index];
                if let Some(id) = tc.id {
                    slot.id.push_str(&id);
                }
                if let Some(f) = tc.function {
                    if let Some(n) = f.name {
                        slot.name.push_str(&n);
                    }
                    if let Some(a) = f.arguments {
                        slot.arguments.push_str(&a);
                    }
                }
            }
        }
    }

    Ok(montar(content, calls))
}

#[derive(Default)]
struct CallBuilder {
    id: String,
    name: String,
    arguments: String,
}

fn montar(content: String, calls: Vec<CallBuilder>) -> Message {
    let tool_calls: Vec<ToolCall> = calls
        .into_iter()
        .filter(|c| !c.name.is_empty())
        .map(|c| ToolCall {
            id: c.id,
            kind: "function".into(),
            function: FunctionCall { name: c.name, arguments: c.arguments },
        })
        .collect();

    Message {
        role: "assistant".into(),
        content: (!content.is_empty()).then_some(content),
        tool_calls: (!tool_calls.is_empty()).then_some(tool_calls),
        tool_call_id: None,
    }
}

// --- conveniências sem streaming ------------------------------------------------

/// Manda `messages` (+ `tools`) e devolve a mensagem do assistente, sem
/// reportar fragmentos. Base do laço de ferramentas quando não se quer stream.
pub async fn complete_tools(messages: &[Message], tools: &[Value]) -> Result<Message> {
    stream_complete(messages, tools, &mut |_| {}).await
}

/// Envia o histórico e retorna só o texto.
pub async fn complete(messages: &[Message]) -> Result<String> {
    Ok(complete_tools(messages, &[]).await?.text().to_string())
}

/// Pergunta única, sem histórico. Conveniência sobre [`complete`].
pub async fn ask(message: &str) -> Result<String> {
    complete(&[Message::system(DEFAULT_SYSTEM_PROMPT), Message::user(message)]).await
}
