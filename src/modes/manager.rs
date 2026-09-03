//! 6ª geração: um agente-gerente decide, em tempo de execução, quem faz o quê.
//!
//! Diferença pra 5ª geração (`crew`): lá a ordem Pesquisador -> Redator ->
//! Revisor é fixa no código. Aqui o gerente é um modo `agent` cuja única
//! ferramenta é `delegar(membro, tarefa)` — ele mesmo escolhe a ordem, pode
//! repetir um membro, pular outro, ou pedir uma correção.

use std::rc::Rc;
use std::time::Instant;

use serde_json::{Value, json};

use crate::Result;
use crate::crew::Agent;
use crate::llm::Message;
use crate::tools::{self, Registry, Tool};
use crate::ui::{self, Reporter};
use crate::{mcp, modes, skills, toolbox};

const MAX_STEPS: usize = 14;

const SYSTEM_PROMPT: &str = "Você é um gerente de projeto. Você NÃO faz o trabalho: \
    quebra o objetivo em tarefas e delega cada uma ao membro certo com a ferramenta \
    `delegar`. Fluxo usual: Pesquisador (levanta fatos) -> Redator (escreve) -> \
    Revisor (revisa). Ao delegar, passe uma instrução completa e autossuficiente, \
    incluindo o que os membros anteriores já produziram. Se o resultado vier ruim, \
    delegue de novo com correções. Quando tiver a versão final do Revisor, responda \
    com ela e pare. Responda em português.";

pub async fn run(args: &[String]) -> Result<()> {
    let cli = modes::parse(args);
    let objetivo = modes::prompt_from(&cli.rest)?;
    if objetivo.is_empty() {
        eprintln!("Nada na entrada. Uso: vulcano manager \"documente o módulo llm.rs\"");
        return Ok(());
    }
    let objetivo = format!("{objetivo}{}", skills::addendum(&skills::load(&cli.skills)?));

    let report: Rc<dyn Reporter> = Rc::new(ui::chrome());
    let team = Rc::new(build_team().await);

    let mut registry = Registry::new();
    registry.register(delegar_tool(Rc::clone(&team), Rc::clone(&report)));

    let mut messages = vec![Message::system(SYSTEM_PROMPT), Message::user(objetivo)];
    let entrega =
        tools::resolve_bounded(&mut messages, &registry, MAX_STEPS, report.as_ref()).await?;
    println!("{entrega}");
    Ok(())
}

async fn build_team() -> Vec<Agent> {
    let mut research_tools = toolbox::system();
    research_tools.extend(mcp::load_all().await);

    vec![
        Agent::new(
            "Pesquisador",
            "Você investiga o código e reúne fatos. Use as ferramentas para ler arquivos \
             e rodar comandos; não altere nada. Entregue uma lista objetiva de achados. \
             Responda em português.",
        )
        .with_tools(research_tools)
        .with_max_steps(16),
        Agent::new(
            "Redator",
            "Você escreve texto claro e direto a partir dos achados recebidos. Não invente \
             o que não estiver no contexto. Responda em português.",
        ),
        Agent::new(
            "Revisor",
            "Você revisa e enxuga o texto recebido: corrige imprecisões e corta repetição. \
             Devolva só a versão final. Responda em português.",
        ),
    ]
}

/// A ferramenta que o gerente usa pra acionar um membro da equipe.
fn delegar_tool(team: Rc<Vec<Agent>>, report: Rc<dyn Reporter>) -> Tool {
    let papeis: Vec<&str> = team.iter().map(|a| a.role).collect();
    let descricao = format!(
        "Entrega uma tarefa a um membro da equipe ({}) e devolve o resultado dele.",
        papeis.join(", ")
    );

    Tool::new(
        "delegar",
        descricao,
        json!({
            "type": "object",
            "properties": {
                "membro": { "type": "string", "enum": papeis },
                "tarefa": { "type": "string", "description": "instrução completa e autossuficiente" }
            },
            "required": ["membro", "tarefa"]
        }),
        move |args: Value| {
            let team = Rc::clone(&team);
            let report = Rc::clone(&report);
            async move {
                let membro = args["membro"].as_str().unwrap_or_default();
                let tarefa = args["tarefa"].as_str().unwrap_or_default();

                let Some(agent) = team.iter().find(|a| a.role.eq_ignore_ascii_case(membro)) else {
                    let nomes: Vec<&str> = team.iter().map(|a| a.role).collect();
                    return Ok(format!(
                        "membro desconhecido: {membro:?}. Use um de: {}",
                        nomes.join(", ")
                    ));
                };

                report.step(agent.role);
                let t0 = Instant::now();
                let saida = agent.run(tarefa, report.as_ref()).await?;
                report.step_done(agent.role, t0.elapsed());
                Ok(saida)
            }
        },
    )
}
