//! Agentes com papéis.
//!
//! Um [`Agent`] é um modo `agent` empacotado: persona (system prompt),
//! ferramentas e teto de passos próprios. A [`Crew`] roda vários em pipeline,
//! injetando a saída de cada um como contexto do próximo. O modo `manager`
//! reaproveita o [`Agent`] mas decide a ordem em tempo de execução.

use std::time::Instant;

use crate::Result;
use crate::llm::Message;
use crate::tools::{self, Registry};
use crate::ui::Reporter;

/// Um agente: papel + persona + ferramentas + teto de passos.
pub struct Agent {
    pub role: &'static str,
    pub system_prompt: String,
    pub registry: Registry,
    pub max_steps: usize,
}

impl Agent {
    pub fn new(role: &'static str, system_prompt: impl Into<String>) -> Self {
        Self {
            role,
            system_prompt: system_prompt.into(),
            registry: Registry::new(),
            max_steps: 12,
        }
    }

    pub fn with_tools(mut self, registry: Registry) -> Self {
        self.registry = registry;
        self
    }

    pub fn with_max_steps(mut self, n: usize) -> Self {
        self.max_steps = n;
        self
    }

    /// Roda uma instrução isolada e devolve o texto produzido.
    pub async fn run(&self, instruction: &str, report: &dyn Reporter) -> Result<String> {
        let mut messages = vec![
            Message::system(self.system_prompt.clone()),
            Message::user(instruction),
        ];
        tools::resolve_bounded(&mut messages, &self.registry, self.max_steps, report).await
    }
}

struct Step {
    agent: Agent,
    task: String,
}

/// Pipeline de etapas. A saída de cada uma vira contexto da próxima.
#[derive(Default)]
pub struct Crew {
    steps: Vec<Step>,
}

impl Crew {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn step(mut self, agent: Agent, task: impl Into<String>) -> Self {
        self.steps.push(Step { agent, task: task.into() });
        self
    }

    /// Executa as etapas em ordem e devolve a saída da última.
    pub async fn run(&self, objetivo: &str, report: &dyn Reporter) -> Result<String> {
        let mut contexto = String::new();
        let mut ultima = String::new();

        for step in &self.steps {
            report.step(step.agent.role);
            let t0 = Instant::now();

            let instrucao = build_instruction(objetivo, &step.task, &contexto);
            let saida = step.agent.run(&instrucao, report).await?;

            report.step_done(step.agent.role, t0.elapsed());
            contexto.push_str(&format!("\n--- Resultado de {} ---\n{saida}\n", step.agent.role));
            ultima = saida;
        }

        Ok(ultima)
    }
}

fn build_instruction(objetivo: &str, task: &str, contexto: &str) -> String {
    let mut s = format!("Objetivo geral: {objetivo}\n\nSua tarefa: {task}");
    if !contexto.is_empty() {
        s.push_str("\n\nContexto do trabalho já feito:\n");
        s.push_str(contexto);
    }
    s
}
