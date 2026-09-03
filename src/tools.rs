//! Ferramentas: o modelo pode pedir pra chamar funções.
//!
//! [`Registry`] guarda as ferramentas disponíveis. [`resolve`] roda o laço
//! "modelo pede tool -> executamos -> devolvemos o resultado" até o modelo
//! responder com texto; [`resolve_bounded`] é a mesma coisa com teto de passos.
//!
//! [`Tool`] guarda um handler como closure boxeada e assíncrona: dá pra
//! registrar funções simples ([`Tool::sync`]), closures com estado, ou tools
//! que fazem I/O — é por aqui que MCP e ferramentas do usuário entram depois.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::{Value, json};

use crate::Result;
use crate::llm::{self, Message};
use crate::ui::Reporter;

/// O que o handler de uma ferramenta devolve: um future com o resultado textual.
pub type ToolFuture = Pin<Box<dyn Future<Output = Result<String>>>>;
/// Handler de uma ferramenta: recebe os argumentos já parseados.
pub type ToolFn = Arc<dyn Fn(Value) -> ToolFuture>;

/// Uma ferramenta que o modelo pode chamar.
pub struct Tool {
    pub name: String,
    pub description: String,
    /// JSON Schema dos argumentos (objeto `parameters` da API).
    pub parameters: Value,
    pub run: ToolFn,
}

impl Tool {
    /// Ferramenta a partir de uma função síncrona (o caso comum).
    pub fn sync(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
        f: fn(Value) -> Result<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
            run: Arc::new(move |args| Box::pin(async move { f(args) })),
        }
    }

    /// Ferramenta a partir de uma closure assíncrona (I/O, estado capturado,
    /// clientes MCP...).
    pub fn new<F, Fut>(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
        f: F,
    ) -> Self
    where
        F: Fn(Value) -> Fut + 'static,
        Fut: Future<Output = Result<String>> + 'static,
    {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
            run: Arc::new(move |args| Box::pin(f(args))),
        }
    }
}

/// Conjunto de ferramentas disponíveis para uma execução.
#[derive(Default)]
pub struct Registry {
    tools: Vec<Tool>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: Tool) -> &mut Self {
        self.tools.push(tool);
        self
    }

    /// Junta outro registro neste (útil pra somar tools do usuário / MCP).
    pub fn extend(&mut self, other: Registry) -> &mut Self {
        self.tools.extend(other.tools);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Nomes registrados, pra mensagens de erro / ajuda.
    pub fn names(&self) -> Vec<&str> {
        self.tools.iter().map(|t| t.name.as_str()).collect()
    }

    /// Traduz as ferramentas para o formato do campo `tools` da API.
    fn schema(&self) -> Vec<Value> {
        self.tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    }
                })
            })
            .collect()
    }

    async fn call(&self, name: &str, args: Value) -> Result<String> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name == name)
            .ok_or_else(|| format!("ferramenta desconhecida: {name}"))?;
        (tool.run)(args).await
    }
}

/// Laço de ferramentas sem teto de passos.
///
/// `messages` é mutado no lugar: fica com o histórico completo da execução
/// (pedidos de tool, resultados e a resposta final).
pub async fn resolve(
    messages: &mut Vec<Message>,
    registry: &Registry,
    report: &dyn Reporter,
) -> Result<String> {
    resolve_bounded(messages, registry, usize::MAX, report).await
}

/// Laço de ferramentas com teto de `max_steps` iterações. O texto do modelo é
/// transmitido via `report` conforme chega.
pub async fn resolve_bounded(
    messages: &mut Vec<Message>,
    registry: &Registry,
    max_steps: usize,
    report: &dyn Reporter,
) -> Result<String> {
    let schema = registry.schema();

    for _ in 0..max_steps {
        let reply = llm::stream_complete(messages, &schema, &mut |t| report.delta(t)).await?;
        let texto = reply.text().to_string();
        messages.push(reply.clone());

        let calls = match reply.tool_calls {
            Some(calls) if !calls.is_empty() => calls,
            _ => {
                report.end_turn();
                return Ok(texto);
            }
        };

        // Se o modelo falou antes de pedir ferramentas, fecha a linha.
        if !texto.is_empty() {
            report.end_turn();
        }

        for call in calls {
            report.tool_call(&call.function.name, &call.function.arguments);
            let args: Value =
                serde_json::from_str(&call.function.arguments).unwrap_or_else(|_| json!({}));
            let result = registry
                .call(&call.function.name, args)
                .await
                .unwrap_or_else(|e| format!("erro: {e}"));
            report.tool_result(&result);
            messages.push(Message::tool(call.id, result));
        }
    }

    Err(format!("limite de {max_steps} passos atingido sem resposta final").into())
}
