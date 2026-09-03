//! Conversa com histórico.

use crate::Result;
use crate::llm::{self, DEFAULT_SYSTEM_PROMPT, Message};
use crate::ui::Reporter;

/// Cada [`send`](Chat::send) acrescenta a mensagem do usuário e a resposta do
/// assistente ao histórico, que é reenviado inteiro na próxima chamada.
pub struct Chat {
    history: Vec<Message>,
}

impl Chat {
    pub fn new() -> Self {
        Self::with_system_prompt(DEFAULT_SYSTEM_PROMPT)
    }

    pub fn with_system_prompt(prompt: impl Into<String>) -> Self {
        Self { history: vec![Message::system(prompt)] }
    }

    /// Envia uma mensagem do usuário; o texto do assistente é transmitido via
    /// `report` conforme chega e também devolvido inteiro no fim.
    pub async fn send(&mut self, message: &str, report: &dyn Reporter) -> Result<String> {
        self.history.push(Message::user(message));
        let reply = llm::stream_complete(&self.history, &[], &mut |t| report.delta(t)).await?;
        let texto = reply.text().to_string();
        self.history.push(reply);
        Ok(texto)
    }

    #[allow(dead_code)]
    pub fn history(&self) -> &[Message] {
        &self.history
    }
}

impl Default for Chat {
    fn default() -> Self {
        Self::new()
    }
}
