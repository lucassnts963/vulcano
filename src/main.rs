// Projeto incremental: alguns itens são seams pra próximos degraus (MCP,
// skills, tools do usuário) e ainda não têm chamador.
#![allow(dead_code)]

mod chat;
mod config;
mod crew;
mod llm;
mod mcp;
mod modes;
mod skills;
mod toolbox;
mod tools;
mod ui;

/// Alias de erro compartilhado por todos os modos.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str);
    let rest: &[String] = if args.len() > 2 { &args[2..] } else { &[] };

    match mode {
        Some("ask") => modes::ask::run(rest).await,
        Some("chat") => modes::chat::run(rest).await,
        Some("tools") => modes::tools::run(rest).await,
        Some("agent") => modes::agent::run(rest).await,
        Some("crew") => modes::crew::run(rest).await,
        Some("manager") => modes::manager::run(rest).await,
        None | Some("help") | Some("-h") | Some("--help") => {
            modes::print_help();
            Ok(())
        }
        Some(other) => {
            eprintln!("Modo desconhecido: {other}\n");
            modes::print_help();
            std::process::exit(2);
        }
    }
}
