//! Skills v1: instruções em Markdown que o usuário injeta no system prompt.
//!
//! Uma skill é `~/.config/vulcano/skills/<nome>/SKILL.md` (ou um `<nome>.md`
//! solto na mesma pasta). Frontmatter opcional:
//!
//! ```text
//! ---
//! name: revisor-pt
//! description: revisa texto em português com rigor
//! ---
//! (corpo com as instruções)
//! ```
//!
//! Os modos aceitam `--skill <nome>` (repetível) e concatenam o corpo ao
//! prompt. Próximo passo (v2): disclosure progressiva + uma tool `run_skill`
//! que executa scripts da pasta da skill.

use crate::Result;
use crate::config;

#[derive(Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub body: String,
}

/// Lê todas as skills disponíveis (pasta ausente = lista vazia).
pub fn discover() -> Vec<Skill> {
    let dir = config::skills_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let skill_md = if path.is_dir() {
            path.join("SKILL.md")
        } else if path.extension().is_some_and(|e| e == "md") {
            path.clone()
        } else {
            continue;
        };

        let Ok(raw) = std::fs::read_to_string(&skill_md) else {
            continue;
        };
        let fallback = path.file_stem().and_then(|s| s.to_str()).unwrap_or("skill");
        out.push(parse(&raw, fallback));
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Resolve nomes pedidos na linha de comando.
pub fn load(names: &[String]) -> Result<Vec<Skill>> {
    if names.is_empty() {
        return Ok(Vec::new());
    }

    let disponiveis = discover();
    let mut escolhidas = Vec::new();
    for n in names {
        match disponiveis.iter().find(|s| s.name.eq_ignore_ascii_case(n)) {
            Some(s) => escolhidas.push(s.clone()),
            None => {
                return Err(format!(
                    "skill '{n}' não encontrada em {}",
                    config::skills_dir().display()
                )
                .into());
            }
        }
    }
    Ok(escolhidas)
}

/// Junta um prompt base com o corpo das skills.
pub fn compose(base: &str, skills: &[Skill]) -> String {
    format!("{base}{}", addendum(skills))
}

/// Só a parte das skills, pra pendurar onde fizer sentido (system ou objetivo).
pub fn addendum(skills: &[Skill]) -> String {
    let mut s = String::new();
    for sk in skills {
        s.push_str(&format!("\n\n# Skill: {}\n", sk.name));
        if !sk.description.is_empty() {
            s.push_str(&format!("{}\n\n", sk.description));
        }
        s.push_str(&sk.body);
    }
    s
}

fn parse(raw: &str, fallback_name: &str) -> Skill {
    let mut name = fallback_name.to_string();
    let mut description = String::new();
    let mut body = raw;

    if let Some(rest) = raw.strip_prefix("---\n")
        && let Some(end) = rest.find("\n---")
    {
        let front = &rest[..end];
        body = rest[end + 4..].trim_start_matches(['\n', '\r']);
        for line in front.lines() {
            if let Some((k, v)) = line.split_once(':') {
                let v = v.trim().trim_matches('"');
                match k.trim() {
                    "name" => name = v.to_string(),
                    "description" => description = v.to_string(),
                    _ => {}
                }
            }
        }
    }

    Skill { name, description, body: body.trim().to_string() }
}
