//! Cada modo é um degrau na evolução das LLMs.
//!
//! Todo modo expõe `run(args: &[String]) -> crate::Result<()>`. Adicionar um
//! novo degrau = novo arquivo aqui + uma linha em [`MODES`] + um braço no
//! `match` de `main`.

pub mod agent;
pub mod ask;
pub mod chat;
pub mod crew;
pub mod manager;
pub mod tools;

use std::io::{self, Read};

use crate::Result;

struct Mode {
    name: &'static str,
    about: &'static str,
}

const MODES: &[Mode] = &[
    Mode {
        name: "ask",
        about: "1ª geração — um prompt entra, um texto sai (sem memória)",
    },
    Mode {
        name: "chat",
        about: "2ª geração — loop; cada turno entra no histórico enviado ao modelo",
    },
    Mode {
        name: "tools",
        about: "3ª geração — o modelo pede ferramentas; executamos e devolvemos",
    },
    Mode {
        name: "agent",
        about: "4ª geração — tarefa + ferramentas de sistema; itera até concluir",
    },
    Mode {
        name: "crew",
        about: "5ª geração — equipe de agentes com papéis, em pipeline fixa",
    },
    Mode {
        name: "manager",
        about: "6ª geração — um gerente decide em runtime quem faz o quê",
    },
    // 7ª geração — MCP/skills: ferramentas externas plugadas pelo usuário.
];

pub fn print_help() {
    println!("vulcano — modos que acompanham a evolução das LLMs\n");
    println!("USO:\n    vulcano <modo> [--skill <nome>]... [texto]\n");
    println!("MODOS:");
    for m in MODES {
        println!("    {:<8}{}", m.name, m.about);
    }
    println!("\n--skill <nome>  injeta ~/.config/vulcano/skills/<nome>/SKILL.md no prompt");
    println!("mcp             ~/.config/vulcano/mcp.json pluga ferramentas externas nos modos com tools");
}

/// Flags comuns extraídas dos argumentos de um modo.
pub struct Cli {
    /// Nomes passados em `--skill <nome>` (repetível).
    pub skills: Vec<String>,
    /// O que sobrou — vira o prompt/tarefa.
    pub rest: Vec<String>,
}

/// Separa `--skill <nome>` do resto.
pub fn parse(args: &[String]) -> Cli {
    let mut skills = Vec::new();
    let mut rest = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--skill" | "-s" => {
                if let Some(v) = it.next() {
                    skills.push(v.clone());
                }
            }
            _ => rest.push(a.clone()),
        }
    }
    Cli { skills, rest }
}

/// Prompt de uso único: junta os argumentos, ou lê o stdin se não houver.
pub fn prompt_from(args: &[String]) -> Result<String> {
    if args.is_empty() {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        Ok(buf.trim().to_string())
    } else {
        Ok(args.join(" "))
    }
}
