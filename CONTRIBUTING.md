# Contribuindo com o rustop-tui

Valeu por querer ajudar! 

## Rodando localmente

```bash
git clone https://github.com/willianctti/rustop-tui.git
cd rustop-tui
cargo run
```

## Antes de abrir um PR

```bash
cargo fmt          # formata o código
cargo build         # garante que compila
cargo test          # roda a suíte de testes
cargo clippy        # (se disponível) lint de boas práticas
```

## Estrutura do projeto

Veja a seção "Arquitetura" no [README.md](README.md) — cada arquivo em `src/`
tem uma responsabilidade única (coleta de sistema, coleta de Docker, estado
da aplicação, ou desenho da UI). Tente manter essa separação.

## Ideias de contribuição

Dá uma olhada na seção "Roadmap" do README, ou abra uma issue com sua ideia
antes de meter a mão no código pra alinharmos o design da feature.

## Reportando bugs

Abra uma issue com:
- Distribuição/versão do Linux
- Saída de `rustc --version`
- Passos pra reproduzir
- Se possível, um print ou GIF do problema
