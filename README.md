# vulcano

CLI de estudo em Rust que percorre a evolução das formas de usar um LLM — de
"um prompt, uma resposta" até uma equipe de agentes coordenados. Cada modo é um
degrau, construído sobre o anterior. Provedor: DeepSeek (API compatível com a
da OpenAI).

Não pretende substituir ferramentas como opencode ou Claude Code — é uma base
mínima e legível pra entender como elas funcionam por dentro.

## Modos

| Modo | Geração | O que faz |
|------|---------|-----------|
| `ask` | 1ª | um prompt entra, um texto sai (sem memória) |
| `chat` | 2ª | loop; cada turno entra no histórico enviado ao modelo |
| `tools` | 3ª | o modelo pede ferramentas; executamos e devolvemos o resultado |
| `agent` | 4ª | tarefa + ferramentas de sistema (arquivo, shell); itera até concluir |
| `crew` | 5ª | equipe de agentes com papéis, em pipeline fixa |
| `manager` | 6ª | um gerente decide em runtime quem faz o quê |

## Rodar

```sh
export DEEPSEEK_API_KEY=sk-...            # ou ~/.config/vulcano/env (ver abaixo)

cargo run -- ask "o que é um LLM?"
cargo run -- chat
cargo run -- tools "quanto é 137 + 856?"
cargo run -- agent "liste os .rs e conte as linhas de cada um"
cargo run -- crew "documente o módulo llm.rs" > entrega.md
cargo run -- manager "escreva um parágrafo sobre o modo chat"
```

Texto do modelo vai pro **stdout**; o "chrome" (ferramentas, etapas, tempos)
vai pro **stderr**. Em `crew`/`manager`, só a entrega final sai no stdout.

## Configuração — `~/.config/vulcano/`

```
~/.config/vulcano/
├── env                    # CHAVE=valor; fallback pra DEEPSEEK_API_KEY
├── mcp.json               # servidores MCP (formato { "mcpServers": { ... } })
└── skills/
    └── <nome>/SKILL.md    # instruções em Markdown, injetadas com --skill <nome>
```

- **Skills**: `cargo run -- ask --skill conciso "..."` concatena o `SKILL.md` ao prompt.
- **MCP**: qualquer modo com ferramentas (`agent`, `crew`, `manager`) soma as tools
  anunciadas pelos servidores do `mcp.json`. Arquivo ausente ou servidor com falha
  não quebram a execução.

Base do diretório: `$VULCANO_CONFIG_DIR`, senão `$XDG_CONFIG_HOME/vulcano`, senão
`~/.config/vulcano`.

## Estrutura

```
src/
├── main.rs      # dispatch argv[1] -> modo
├── config.rs    # ~/.config/vulcano/ (env, mcp.json, skills)
├── llm.rs       # ponto único da API: stream_complete (SSE)
├── ui.rs        # trait Reporter + Term (stdout/stderr, cor, tempos)
├── tools.rs     # Registry + Tool (closure async) + laço resolve/resolve_bounded
├── toolbox.rs   # ferramentas de sistema concretas
├── skills.rs    # carga de SKILL.md
├── mcp.rs       # cliente MCP stdio (JSON-RPC) -> Registry
├── chat.rs      # Chat (histórico)
├── crew.rs      # Agent + Crew (pipeline)
└── modes/       # ask, chat, tools, agent, crew, manager
```

Adicionar um modo: novo arquivo em `src/modes/`, uma linha em `MODES`
(`src/modes/mod.rs`) e um braço no `match` de `main.rs`.
