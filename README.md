<p align="center">
  <img src="assets/b-snip.png" alt="b-snip" width="280">
</p>

<h1 align="center">b-snip</h1>

<p align="center">
  <strong>Fila de clipboard inteligente + expansor de texto + transformador</strong><br>
  Um Ãºnico EXE para Windows 10 e 11. Sem instalador. Sem AutoHotkey. Sem runtime.
</p>

<p align="center">
  <img alt="versÃ£o" src="https://img.shields.io/badge/versÃ£o-1.0.0-7c3aed">
  <img alt="plataforma" src="https://img.shields.io/badge/Windows-10%20%7C%2011-111827">
  <img alt="linguagem" src="https://img.shields.io/badge/Rust-stable%20GNU-dea584">
  <img alt="licenÃ§a" src="https://img.shields.io/badge/repo-privado-6b7280">
</p>

---

## Proposta

O **b-snip** fica na bandeja e intercepta o que o Windows copia, para colar **em ordem** (FIFO), expandir gatilhos digitÃ¡veis e mudar maiÃºsculas no lugar â€” tudo com janelas escuras (`#2b2b2b`) e um EXE sÃ³.

Nasceu de um script AutoHotkey v2 de uso diÃ¡rio e foi reescrito em **Rust + egui + Win32**, para durar anos sem depender de runtime instalado.

| Diferencial | O que faz |
|-------------|-----------|
| Fila FIFO rica | `Ctrl+C` entra; `Ctrl+V` cola do mais antigo. Unicode + HTML + RTF + imagem do trecho |
| Loop ou fim | Loop cicla a fila. Sem loop, cola atÃ© acabar. â€œLimpar ao colar o Ãºltimoâ€ Ã© o inverso do Loop |
| Pausa inteligente | Arquivo, pasta e print sem texto nÃ£o entram. Se a fila tem itens, pausa sozinha. Texto com imagem entra |
| Expansor | Gatilhos (`!mail`, etc.) colam um pacote rico gravado em SQLite. `!data` â†’ `DD/MM/AAAA` |
| Transformador | `Ctrl+Alt+T` â†’ 1 tÃ­tulo / 2 maiÃºscula / 3 minÃºscula. Preserva tags do texto rico |
| PortÃ¡til de verdade | Um `b-snip.exe`. Dados em `%LOCALAPPDATA%\b-snip\` ou ao lado do EXE (`portable.txt`) |

---


## Download

O EXE desta versÃ£o estÃ¡ nas [releases](https://github.com/wilbresley/b-snip/releases/latest).

VersÃµes anteriores permanecem na mesma pÃ¡gina.
## Como usar

1. Compile (`.\scripts\run.ps1`) ou rode `dist\b-snip.exe`
2. Ãcone na **bandeja** (Windows 11: seta ^)
3. Copie textos com `Ctrl+C` â€” o balÃ£o `1/5` aparece quando a fila tem itens
4. `Ctrl+V` cola o prÃ³ximo da fila (nÃ£o remove no modo Loop)
5. Clique **direito** no Ã­cone â†’ menu (CÃ³pia, FunÃ§Ãµes, Sistema)

**Clique esquerdo no Ã­cone**

- Fila com itens â†’ pausa / retoma (nÃ£o apaga)
- Fila vazia â†’ liga / desliga a cÃ³pia contÃ­nua (desligar limpa fila + clipboard do Windows)

---

## Atalhos

| Atalho | FunÃ§Ã£o |
|--------|--------|
| `Ctrl+C` / `Ctrl+X` | Entra na fila (mesmo texto de novo = outro `Ctrl+C`) |
| `Shift+Delete` | Recorta **sem** entrar na fila |
| `Ctrl+V` | Cola da fila com a tecla de verdade (ou o Windows, se a fila estiver vazia/pausada) |
| `Ctrl+Shift+V` | Cola da fila como texto puro |
| `Ctrl+Alt+R` | Limpa fila + clipboard do Windows (aviso + som) |
| `Ctrl+Alt+P` | Pausa / retoma a fila |
| `Ctrl+Alt+T` | Transformar seleÃ§Ã£o (1 / 2 / 3 / 4 cancela) |
| `Ctrl+Alt+E` | Abrir o expansor |
| `Ctrl+Alt+Shift+H` | Central de ajuda |
| `!data` | Data de hoje `DD/MM/AAAA` |

Ajuda completa tambÃ©m estÃ¡ **dentro do app** (primeira execuÃ§Ã£o abre sozinha).

---

## Limites (v1.0.0)

| Recurso | Teto |
|---------|------|
| Itens na fila | 100 |
| Tamanho por item da fila | 2 MB |
| Total da fila | 32 MB |
| Aviso | ~80% |
| Linhas do expansor | 3 no comeÃ§o, atÃ© 100 |
| Item do expansor | 8 MB |

Arquivo, pasta (`CF_HDROP`) e print puro (`CF_DIB` / bitmap sem HTML/RTF) **nÃ£o entram** na fila.

---

## Gerar o programa

```powershell
# 1Âª vez neste PC
.\scripts\setup-windows.ps1

# EXE de distribuiÃ§Ã£o â†’ dist\b-snip.exe
.\scripts\build-release.ps1

# Compila e jÃ¡ abre
.\scripts\run.ps1

# Testes
.\scripts\test.ps1
```

Detalhes, toolchain e fork: **[Guia para desenvolvedores](docs/GUIA-DEV.md)**.

---

## DocumentaÃ§Ã£o

| Documento | ConteÃºdo |
|-----------|----------|
| [Guia para desenvolvedores](docs/GUIA-DEV.md) | Fork, toolchain, scripts, gerar o prÃ³prio EXE |
| [O que foi necessÃ¡rio](docs/REQUISITOS.md) | Ferramentas, crates, APIs Win32, decisÃµes de plataforma |
| [Arquitetura](docs/ARQUITETURA.md) | Threads, mÃ³dulos, clipboard, gancho de teclado |
| [Mapa dos mÃ³dulos](docs/MODULOS.md) | Cada arquivo em `src/` |
| [Dados e persistÃªncia](docs/DADOS.md) | INI, SQLite, modo portÃ¡til, backup zip |
| [VersÃ£o 1.0.0](docs/VERSAO-1.0.0.md) | O que esta versÃ£o entrega |

Ãndice: [docs/README.md](docs/README.md)

---

## Dados

- PadrÃ£o: `%LOCALAPPDATA%\b-snip\` (`ClipboardExpander.ini` + `expander.db`)
- PortÃ¡til: crie `portable.txt` ao lado do EXE (ou use a aba **Dados** na ajuda)
- Herda pasta antiga `%LOCALAPPDATA%\b-abcABC\` na primeira execuÃ§Ã£o
- Backup / restauraÃ§Ã£o: zip na janela do expansor

---

## CrÃ©ditos

**WBC** Â· Windows 10/11 Â· Rust + egui + Win32

VersÃ£o **1.0.0** â€” outubro de 2026
