//! Camada de apresentação. Os modos falam com um `&dyn Reporter`; trocar o
//! visual — ou silenciar — é trocar a implementação, não mexer nos modos.

use std::io::{IsTerminal, Write};
use std::time::Duration;

/// Recebe os eventos de uma execução. Todos os métodos têm default vazio, então
/// dá pra implementar só o que interessa.
pub trait Reporter {
    /// Fragmento de texto recém-chegado do stream do modelo.
    fn delta(&self, _text: &str) {}
    /// Fim de um turno de streaming (hora de quebrar a linha).
    fn end_turn(&self) {}
    /// Começo de uma etapa nomeada (ex.: papel de um agente).
    fn step(&self, _label: &str) {}
    /// O modelo pediu uma ferramenta.
    fn tool_call(&self, _name: &str, _args: &str) {}
    /// Resultado que devolvemos pra ferramenta.
    fn tool_result(&self, _output: &str) {}
    /// Uma etapa terminou, com o tempo que levou.
    fn step_done(&self, _label: &str, _elapsed: Duration) {}
}

/// Não imprime nada. Para uso programático / testes.
pub struct Quiet;
impl Reporter for Quiet {}

/// Renderiza no terminal: "chrome" (ferramentas, etapas, tempos) sempre no
/// stderr, com cores quando é TTY. O texto do modelo vai pro stdout, a menos
/// que `deltas_no_stderr` — usado por `crew`/`manager`, onde só a entrega
/// final deve sair no stdout e o falatório dos sub-agentes é ruído.
pub struct Term {
    color: bool,
    deltas_no_stderr: bool,
}

impl Term {
    fn paint(&self, code: &str, s: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
}

/// Texto do modelo no stdout. Para `ask`, `chat`, `tools`, `agent`.
pub fn term() -> Term {
    Term { color: std::io::stderr().is_terminal(), deltas_no_stderr: false }
}

/// Texto do modelo no stderr (é chrome). Para `crew` e `manager`, que imprimem
/// a entrega final no stdout por conta própria.
pub fn chrome() -> Term {
    Term { color: std::io::stderr().is_terminal(), deltas_no_stderr: true }
}

const PREVIEW: usize = 240;

fn preview(s: &str) -> String {
    let corte: String = s.chars().take(PREVIEW).collect();
    let corte = corte.replace('\n', " ");
    if s.chars().count() > PREVIEW { format!("{corte}…") } else { corte }
}

impl Reporter for Term {
    fn delta(&self, text: &str) {
        if self.deltas_no_stderr {
            eprint!("{text}");
            let _ = std::io::stderr().flush();
        } else {
            print!("{text}");
            let _ = std::io::stdout().flush();
        }
    }

    fn end_turn(&self) {
        if self.deltas_no_stderr {
            eprintln!();
        } else {
            println!();
        }
    }

    fn step(&self, label: &str) {
        eprintln!("\n{}", self.paint("1;36", &format!("▸ {label}")));
    }

    fn tool_call(&self, name: &str, args: &str) {
        eprintln!("{}", self.paint("2", &format!("  ⚙ {name} {args}")));
    }

    fn tool_result(&self, output: &str) {
        eprintln!("{}", self.paint("2", &format!("  ↳ {}", preview(output))));
    }

    fn step_done(&self, label: &str, elapsed: Duration) {
        eprintln!("{}", self.paint("2", &format!("  ✓ {label} em {:.1}s", elapsed.as_secs_f64())));
    }
}
