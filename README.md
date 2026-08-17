# rustop-tui 

Monitor de sistema moderno pra terminal, feito em **Rust** com **[ratatui](https://ratatui.rs)**.
Acompanha CPU (total e por núcleo), RAM, rede e containers Docker em tempo real,
com gráficos animados direto no terminal — sem depender de GUI.

![status](https://img.shields.io/badge/status-em%20desenvolvimento-yellow)
![license](https://img.shields.io/badge/license-MIT-blue)

## Screenshots
<img width="1357" height="688" alt="image" src="https://github.com/user-attachments/assets/010b00dd-82c7-4e6c-9ad6-1b3c2fb34a3b" />

<img width="1357" height="665" alt="image" src="https://github.com/user-attachments/assets/271522a8-7799-4f3a-91a0-579ddbd0f82e" />

<img width="1354" height="657" alt="image" src="https://github.com/user-attachments/assets/f71b60a2-7f96-448a-aa67-846c7c3ec2a1" />


## Funcionalidades

- **CPU** — uso total, por núcleo (barras coloridas por carga) e histórico em sparkline
- **RAM/Swap** — gauge de uso, valores em GB/MB
- **Rede** — taxa de download/upload em tempo real com sparklines
- **Processos** — tabela navegável ordenada por uso de CPU
- **Containers Docker** — CPU, memória e rede de cada container rodando (via `docker stats`)
- **Cores** que mudam conforme a carga (verde → amarelo → vermelho) e spinner animado
- **Leve**: lê direto de `/proc`, sem dependências pesadas

## Como baixar e instalar

### Pré-requisito: Rust

Precisa do toolchain do Rust instalado (`cargo` + `rustc`, versão 1.75+). Se
ainda não tem:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 1. Clonar o repositório

```bash
git clone https://github.com/willianctti/rustop-tui.git
cd rustop-tui
```

### 2. Rodar

**Opção A — build + execução em um comando (bom pra testar):**

```bash
cargo run --release
```

**Opção B — compilar e instalar no seu `$PATH` (fica disponível como comando `rustop-tui` em qualquer pasta):**

```bash
cargo install --path .
rustop-tui
```

**Opção C — build manual, sem instalar:**

```bash
cargo build --release
./target/release/rustop-tui
```

O binário final fica em `target/release/rustop-tui` — é um único executável,
pode copiar pra qualquer lugar do `$PATH` (ex: `sudo cp target/release/rustop-tui /usr/local/bin/`).

## Atalhos

| Tecla         | Ação                          |
|---------------|-------------------------------|
| `q` / `Esc`   | Sair                          |
| `Tab` / `→`   | Próxima aba                   |
| `←`           | Aba anterior                  |
| `↑` / `↓`     | Navegar na lista de processos |

## Arquitetura

```
src/
├── main.rs    # setup do terminal, event loop, timers de animação/coleta
├── app.rs     # estado da aplicação, histórico dos gráficos, abas
├── system.rs  # coleta de CPU/RAM/rede/processos lendo /proc diretamente
├── docker.rs  # coleta de stats de containers via `docker stats`
└── ui.rs      # todos os widgets e o desenho da tela (ratatui)
```

Duas taxas de atualização rodam em paralelo:
- **Animação** (100ms): spinner e qualquer efeito visual
- **Métricas** (1s): leitura real de CPU/RAM/rede/processos, no mesmo ritmo do htop/btop

Os stats do Docker rodam numa thread separada (a cada ~2s) pra não travar a UI
enquanto o comando `docker stats` responde.

## Testes

```bash
cargo test
```

Os testes cobrem os parsers mais sensíveis a erro: leitura de `/proc/stat`,
cálculo de % de CPU a partir de jiffies, parsing da saída do `docker stats`
e formatação de bytes.

## Roadmap / ideias de contribuição

- [ ] Gráfico de disco (uso e I/O)
- [ ] Modo "compacto" pra rodar em barras de status (tmux/polybar)
- [ ] Exportar métricas pra Prometheus
- [ ] Configuração via arquivo (`~/.config/rustop-tui/config.toml`)
- [ ] Empacotar como `.deb` e Snap

PRs são bem-vindos! Veja [CONTRIBUTING.md](CONTRIBUTING.md).

## Licença

MIT — veja [LICENSE](LICENSE).
# rustop-tui
