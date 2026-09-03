//! Onde o vulcano procura configuração do usuário.
//!
//! Base: `$VULCANO_CONFIG_DIR`, senão `$XDG_CONFIG_HOME/vulcano`, senão
//! `~/.config/vulcano`. Skills, MCP e o arquivo `env` (fallback de segredos)
//! penduram daqui.

use std::path::PathBuf;

/// Diretório de configuração (pode não existir).
pub fn dir() -> PathBuf {
    if let Ok(d) = std::env::var("VULCANO_CONFIG_DIR")
        && !d.is_empty()
    {
        return PathBuf::from(d);
    }

    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            PathBuf::from(home).join(".config")
        });

    base.join("vulcano")
}

/// `~/.config/vulcano/skills/` — uma pasta (ou .md) por skill.
pub fn skills_dir() -> PathBuf {
    dir().join("skills")
}

/// `~/.config/vulcano/mcp.json` — servidores MCP.
pub fn mcp_path() -> PathBuf {
    dir().join("mcp.json")
}

/// `~/.config/vulcano/env` — pares `CHAVE=valor` (fallback pra segredos).
pub fn env_path() -> PathBuf {
    dir().join("env")
}

/// Valor de `name`: primeiro do ambiente do processo, senão do arquivo `env`.
///
/// Formato do arquivo: uma `CHAVE=valor` por linha; `#` comenta a linha;
/// `export ` no início e aspas em volta do valor são toleradas.
pub fn env_or_file(name: &str) -> Option<String> {
    if let Ok(v) = std::env::var(name)
        && !v.is_empty()
    {
        return Some(v);
    }

    let raw = std::fs::read_to_string(env_path()).ok()?;
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if k.trim() != name {
            continue;
        }
        let v = v.trim().trim_matches('"').trim_matches('\'');
        if !v.is_empty() {
            return Some(v.to_string());
        }
    }
    None
}
