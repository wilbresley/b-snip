<p align="center">
  <img src="assets/b-snip.png" alt="b-snip" width="280">
</p>

<h1 align="center">b-snip</h1>

<p align="center">
  <strong>Fila de clipboard inteligente + expansor de texto + transformador</strong><br>
  Um único EXE para Windows 10 e 11. Sem instalador. Sem AutoHotkey. Sem runtime.
</p>

<p align="center">
  <img alt="versão" src="https://img.shields.io/badge/versão-1.0.0-7c3aed">
  <img alt="plataforma" src="https://img.shields.io/badge/Windows-10%20%7C%2011-111827">
  <img alt="linguagem" src="https://img.shields.io/badge/Rust-stable%20GNU-dea584">
  <img alt="licença" src="https://img.shields.io/badge/repo-publico-16a34a">
</p>

---

## Proposta

O **b-snip** fica na bandeja e intercepta o que o Windows copia, para colar **em ordem** (FIFO), expandir gatilhos digitáveis e mudar maiúsculas no lugar — tudo com janelas escuras (`#2b2b2b`) e um EXE só.

Nasceu de um script AutoHotkey v2 de uso diário e foi reescrito em **Rust + egui + Win32**, para durar anos sem depender de runtime instalado.

| Diferencial | O que faz |
|-------------|-----------|
| Fila FIFO rica | `Ctrl+C` entra; `Ctrl+V` cola do mais antigo. Unicode + HTML + RTF + imagem do trecho |
| Loop ou fim | Loop cicla a fila. Sem loop, cola até acabar. “Limpar ao colar o último” é o inverso do Loop |
| Pausa inteligente | Arquivo, pasta e print sem texto não entram. Se a fila tem itens, pausa sozinha. Texto com imagem entra |
| Expansor | Gatilhos (`!mail`, etc.) colam um pacote rico gravado em SQLite. `!data` → `DD/MM/AAAA` |
| Transformador | `Ctrl+Alt+T` → 1 título / 2 maiúscula / 3 minúscula. Preserva tags do texto rico |
| Portátil de verdade | Um `b-snip.exe`. Dados em `%LOCALAPPDATA%\b-snip\` ou ao lado do EXE (`portable.txt`) |

---


## Download

O EXE desta versão está nas [releases](https://github.com/wilbresley/b-snip/releases/latest).

Versões anteriores permanecem na mesma página.

## Como usar

1. Compile (`.\scripts\run.ps1`) ou rode `dist\b-snip.exe`
2. Ícone na **bandeja** (Windows 11: seta ^)
3. Copie textos com `Ctrl+C` — o balão `1/5` aparece quando a fila tem itens
4. `Ctrl+V` cola o próximo da fila (não remove no modo Loop)
5. Clique **direito** no ícone → menu (Cópia, Funções, Sistema)

**Clique esquerdo no ícone**

- Fila com itens → pausa / retoma (não apaga)
- Fila vazia → liga / desliga a cópia contínua (desligar limpa fila + clipboard do Windows)

---

## Atalhos

| Atalho | Função |
|--------|--------|
| `Ctrl+C` / `Ctrl+X` | Entra na fila (mesmo texto de novo = outro `Ctrl+C`) |
| `Shift+Delete` | Recorta **sem** entrar na fila |
| `Ctrl+V` | Cola da fila com a tecla de verdade (ou o Windows, se a fila estiver vazia/pausada) |
| `Ctrl+Shift+V` | Cola da fila como texto puro |
| `Ctrl+Alt+R` | Limpa fila + clipboard do Windows (aviso + som) |
| `Ctrl+Alt+P` | Pausa / retoma a fila |
| `Ctrl+Alt+T` | Transformar seleção (1 / 2 / 3 / 4 cancela) |
| `Ctrl+Alt+E` | Abrir o expansor |
| `Ctrl+Alt+Shift+H` | Central de ajuda |
| `!data` | Data de hoje `DD/MM/AAAA` |

Ajuda completa também está **dentro do app** (primeira execução abre sozinha).

---

## Limites (v1.0.0)

| Recurso | Teto |
|---------|------|
| Itens na fila | 100 |
| Tamanho por item da fila | 2 MB |
| Total da fila | 32 MB |
| Aviso | ~80% |
| Linhas do expansor | 3 no começo, até 100 |
| Item do expansor | 8 MB |

Arquivo, pasta (`CF_HDROP`) e print puro (`CF_DIB` / bitmap sem HTML/RTF) **não entram** na fila.

---

## Gerar o programa

```powershell
# 1ª vez neste PC
.\scripts\setup-windows.ps1

# EXE de distribuição → dist\b-snip.exe
.\scripts\build-release.ps1

# Compila e já abre
.\scripts\run.ps1

# Testes
.\scripts\test.ps1
```

Detalhes, toolchain e fork: **[Guia para desenvolvedores](docs/GUIA-DEV.md)**.

---

## Documentação

| Documento | Conteúdo |
|-----------|----------|
| [Guia para desenvolvedores](docs/GUIA-DEV.md) | Fork, toolchain, scripts, gerar o próprio EXE |
| [O que foi necessário](docs/REQUISITOS.md) | Ferramentas, crates, APIs Win32, decisões de plataforma |
| [Arquitetura](docs/ARQUITETURA.md) | Threads, módulos, clipboard, gancho de teclado |
| [Mapa dos módulos](docs/MODULOS.md) | Cada arquivo em `src/` |
| [Dados e persistência](docs/DADOS.md) | INI, SQLite, modo portátil, backup zip |
| [Versão 1.0.0](docs/VERSAO-1.0.0.md) | O que esta versão entrega |

Índice: [docs/README.md](docs/README.md)

---

## Dados

- Padrão: `%LOCALAPPDATA%\b-snip\` (`ClipboardExpander.ini` + `expander.db`)
- Portátil: crie `portable.txt` ao lado do EXE (ou use a aba **Dados** na ajuda)
- Herda pasta antiga `%LOCALAPPDATA%\b-abcABC\` na primeira execução
- Backup / restauração: zip na janela do expansor

---

## Créditos

**WBC** · Windows 10/11 · Rust + egui + Win32

Versão **1.0.0** — outubro de 2026
