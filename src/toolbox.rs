//! Ferramentas concretas de sistema, reaproveitadas pelos modos `agent`,
//! `crew` e `manager`. O mecanismo genérico (registro + laço) fica em
//! [`crate::tools`]; aqui ficam as funções de verdade.

use std::process::Command;

use serde_json::{Value, json};

use crate::Result;
use crate::tools::{Registry, Tool};

/// Ler/escrever arquivo, listar diretório e rodar shell.
pub fn system() -> Registry {
    let mut r = Registry::new();

    r.register(Tool::sync(
        "listar_dir",
        "Lista os arquivos e subpastas de um diretório.",
        json!({
            "type": "object",
            "properties": { "caminho": { "type": "string", "description": "padrão: ." } }
        }),
        listar_dir,
    ));

    r.register(Tool::sync(
        "ler_arquivo",
        "Devolve o conteúdo de um arquivo de texto.",
        json!({
            "type": "object",
            "properties": { "caminho": { "type": "string" } },
            "required": ["caminho"]
        }),
        ler_arquivo,
    ));

    r.register(Tool::sync(
        "escrever_arquivo",
        "Cria ou sobrescreve um arquivo com o conteúdo dado.",
        json!({
            "type": "object",
            "properties": {
                "caminho": { "type": "string" },
                "conteudo": { "type": "string" }
            },
            "required": ["caminho", "conteudo"]
        }),
        escrever_arquivo,
    ));

    r.register(Tool::sync(
        "shell",
        "Executa um comando de shell (sh -c) e devolve stdout, stderr e o código de saída.",
        json!({
            "type": "object",
            "properties": { "comando": { "type": "string" } },
            "required": ["comando"]
        }),
        shell,
    ));

    r
}

fn listar_dir(args: Value) -> Result<String> {
    let caminho = args["caminho"].as_str().unwrap_or(".");
    let mut linhas = Vec::new();
    for entry in std::fs::read_dir(caminho)? {
        let entry = entry?;
        let marca = if entry.file_type()?.is_dir() { "dir " } else { "file" };
        linhas.push(format!("{marca}  {}", entry.file_name().to_string_lossy()));
    }
    linhas.sort();
    Ok(if linhas.is_empty() { "(vazio)".into() } else { linhas.join("\n") })
}

fn ler_arquivo(args: Value) -> Result<String> {
    let caminho = args["caminho"].as_str().ok_or("argumento 'caminho' ausente")?;
    Ok(std::fs::read_to_string(caminho)?)
}

fn escrever_arquivo(args: Value) -> Result<String> {
    let caminho = args["caminho"].as_str().ok_or("argumento 'caminho' ausente")?;
    let conteudo = args["conteudo"].as_str().unwrap_or("");
    std::fs::write(caminho, conteudo)?;
    Ok(format!("escrito ({} bytes) em {caminho}", conteudo.len()))
}

fn shell(args: Value) -> Result<String> {
    let comando = args["comando"].as_str().ok_or("argumento 'comando' ausente")?;
    let saida = Command::new("sh").arg("-c").arg(comando).output()?;

    let mut resultado = String::from_utf8_lossy(&saida.stdout).into_owned();
    if !saida.stderr.is_empty() {
        resultado.push_str("\n[stderr]\n");
        resultado.push_str(&String::from_utf8_lossy(&saida.stderr));
    }
    resultado.push_str(&format!("\n[exit {}]", saida.status.code().unwrap_or(-1)));
    Ok(resultado)
}
