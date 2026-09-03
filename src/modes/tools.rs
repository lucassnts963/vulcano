//! 3ª geração: um prompt entra, o modelo escolhe ferramentas, o texto sai.

use serde_json::{Value, json};

use crate::Result;
use crate::llm::Message;
use crate::tools::{self, Registry, Tool};
use crate::ui;

pub async fn run(args: &[String]) -> Result<()> {
    let prompt = crate::modes::prompt_from(args)?;
    if prompt.is_empty() {
        eprintln!("Nada na entrada. Uso: vulcano tools \"quanto é 12 + 30?\"");
        return Ok(());
    }

    let registry = build_registry();
    let report = ui::term();
    let mut messages = vec![
        Message::system(
            "Você é um assistente que usa as ferramentas disponíveis quando elas ajudam. \
             Responda em português.",
        ),
        Message::user(prompt),
    ];

    tools::resolve(&mut messages, &registry, &report).await?;
    Ok(())
}

/// Ferramentas de exemplo. Adicionar uma nova = mais um `register` aqui.
fn build_registry() -> Registry {
    let mut registry = Registry::new();

    registry.register(Tool::sync(
        "soma",
        "Soma dois números e retorna o resultado.",
        json!({
            "type": "object",
            "properties": {
                "a": { "type": "number" },
                "b": { "type": "number" }
            },
            "required": ["a", "b"]
        }),
        soma,
    ));

    registry.register(Tool::sync(
        "clima",
        "Retorna a condição do tempo agora para uma cidade.",
        json!({
            "type": "object",
            "properties": { "cidade": { "type": "string" } },
            "required": ["cidade"]
        }),
        clima,
    ));

    registry
}

fn soma(args: Value) -> Result<String> {
    let a = args["a"].as_f64().ok_or("argumento 'a' ausente ou não numérico")?;
    let b = args["b"].as_f64().ok_or("argumento 'b' ausente ou não numérico")?;
    Ok((a + b).to_string())
}

fn clima(args: Value) -> Result<String> {
    let cidade = args["cidade"].as_str().unwrap_or("cidade desconhecida");
    Ok(format!("Em {cidade} está 24°C, céu limpo."))
}
