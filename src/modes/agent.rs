//! 4ª geração: recebe uma tarefa e itera com ferramentas de sistema
//! (ler/escrever arquivo, listar diretório, shell) até concluir.
//!
//! Mesmo laço da 3ª geração ([`crate::tools::resolve_bounded`]) — muda o
//! system prompt (orientado a tarefa), o conjunto de ferramentas
//! ([`crate::toolbox::system`] + o que vier de MCP) e o teto de passos.

use crate::Result;
use crate::llm::Message;
use crate::{mcp, modes, skills, toolbox, tools, ui};

const MAX_STEPS: usize = 16;

const SYSTEM_PROMPT: &str = "Você é um agente autônomo rodando na máquina do usuário. \
    Cumpra a tarefa usando as ferramentas, um passo de cada vez, sem pedir confirmação. \
    Ao terminar, responda em texto com um resumo curto do que foi feito e não chame \
    mais nenhuma ferramenta. Responda em português.";

pub async fn run(args: &[String]) -> Result<()> {
    let cli = modes::parse(args);
    let tarefa = modes::prompt_from(&cli.rest)?;
    if tarefa.is_empty() {
        eprintln!("Nada na entrada. Uso: vulcano agent \"resuma o README em 3 linhas\"");
        return Ok(());
    }

    let system = skills::compose(SYSTEM_PROMPT, &skills::load(&cli.skills)?);

    let mut registry = toolbox::system();
    registry.extend(mcp::load_all().await);

    let report = ui::term();
    let mut messages = vec![Message::system(system), Message::user(tarefa)];

    tools::resolve_bounded(&mut messages, &registry, MAX_STEPS, &report).await?;
    Ok(())
}
