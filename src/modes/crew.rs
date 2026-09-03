//! 5ª geração: uma equipe de agentes com papéis, em pipeline fixa. Cada um faz
//! sua parte e passa o resultado adiante.

use crate::Result;
use crate::crew::{Agent, Crew};
use crate::{mcp, modes, skills, toolbox, ui};

pub async fn run(args: &[String]) -> Result<()> {
    let cli = modes::parse(args);
    let objetivo = modes::prompt_from(&cli.rest)?;
    if objetivo.is_empty() {
        eprintln!("Nada na entrada. Uso: vulcano crew \"documente o módulo llm.rs\"");
        return Ok(());
    }

    // Skills entram no objetivo -> chegam a todos os papéis via build_instruction.
    let objetivo = format!("{objetivo}{}", skills::addendum(&skills::load(&cli.skills)?));

    let mut research_tools = toolbox::system();
    research_tools.extend(mcp::load_all().await);

    let crew = Crew::new()
        .step(
            Agent::new(
                "Pesquisador",
                "Você investiga o código e reúne fatos. Use as ferramentas para ler \
                 arquivos e rodar comandos; não altere nada. Entregue uma lista objetiva \
                 de achados. Responda em português.",
            )
            .with_tools(research_tools)
            .with_max_steps(16),
            "Levante os fatos necessários para cumprir o objetivo.",
        )
        .step(
            Agent::new(
                "Redator",
                "Você escreve texto claro e direto a partir dos achados recebidos. Não \
                 invente o que não estiver no contexto. Responda em português.",
            ),
            "Escreva o material pedido no objetivo, usando os achados do Pesquisador.",
        )
        .step(
            Agent::new(
                "Revisor",
                "Você revisa e enxuga o texto recebido: corrige imprecisões e corta \
                 repetição. Devolva só a versão final. Responda em português.",
            ),
            "Revise o texto do Redator e entregue a versão final.",
        );

    let report = ui::chrome();
    let entrega = crew.run(&objetivo, &report).await?;
    println!("{entrega}");
    Ok(())
}
