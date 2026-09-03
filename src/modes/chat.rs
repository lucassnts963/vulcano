//! 2ª geração: loop. Cada mensagem do usuário entra no array enviado ao modelo,
//! junto com todas as respostas anteriores.

use std::io::{self, Write};

use crate::Result;
use crate::chat::Chat;
use crate::llm::DEFAULT_SYSTEM_PROMPT;
use crate::{modes, skills, ui};

pub async fn run(args: &[String]) -> Result<()> {
    let cli = modes::parse(args);
    let system = skills::compose(DEFAULT_SYSTEM_PROMPT, &skills::load(&cli.skills)?);

    let mut chat = Chat::with_system_prompt(system);
    let report = ui::term();
    println!("Chat com DeepSeek. Digite 'sair' (ou Ctrl+D) para encerrar.\n");

    loop {
        print!("Você: ");
        io::stdout().flush().ok();

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break; // EOF (Ctrl+D)
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }
        if input.eq_ignore_ascii_case("sair") {
            break;
        }

        print!("\nDeepSeek: ");
        io::stdout().flush().ok();
        match chat.send(input, &report).await {
            Ok(_) => println!(),
            Err(e) => println!("\n[erro] {e}\n"),
        }
    }

    Ok(())
}
