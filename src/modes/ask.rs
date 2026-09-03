//! 1ª geração: entrada única -> saída única. Sem histórico.

use crate::Result;
use crate::llm::{self, DEFAULT_SYSTEM_PROMPT, Message};
use crate::ui::{self, Reporter};
use crate::{modes, skills};

pub async fn run(args: &[String]) -> Result<()> {
    let cli = modes::parse(args);
    let prompt = modes::prompt_from(&cli.rest)?;
    if prompt.is_empty() {
        eprintln!("Nada na entrada. Uso: vulcano ask \"sua pergunta\"");
        return Ok(());
    }

    let system = skills::compose(DEFAULT_SYSTEM_PROMPT, &skills::load(&cli.skills)?);
    let report = ui::term();
    let messages = [Message::system(system), Message::user(prompt)];

    llm::stream_complete(&messages, &[], &mut |t| report.delta(t)).await?;
    report.end_turn();
    Ok(())
}
