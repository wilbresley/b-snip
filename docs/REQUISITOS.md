# O que foi necessário para criar o b-snip

Lista objetiva de plataforma, ferramentas, crates e APIs. Nada disto é “nice to have”: sem cada bloco o EXE não fica portátil ou o gancho não se comporta como o AutoHotkey.

## Plataforma

| Item | Escolha |
|------|---------|
| SO alvo | Windows 10 e 11, x64 |
| Entrega | Um `b-snip.exe`, sem instalador, sem runtime |
| Origem | Script de clipboard em AutoHotkey v2 — fila FIFO, transform 1/2/3, expansor, `!data` |
| Linguagem | **Rust** (decidido para durar anos, EXE pequeno, Win32 de verdade) |
| GUI | **egui 0.37** + **eframe** (glow), tema `#2b2b2b` |
| Toolchain | `x86_64-pc-windows-gnu` (não MSVC) |

C# Native AOT chegou a ser a primeira sugestão (portar mais rápido). O critério “deixar mofar anos, um EXE, Win10/11, UI escura” virou Rust + egui.

## Ferramentas de build (máquina do desenvolvedor)

| Ferramenta | Uso |
|------------|-----|
| [Rustup](https://rustup.rs) + `stable` | `cargo`, `rustc` 1.99+ testado |
| Alvo `x86_64-pc-windows-gnu` | `rustup target add x86_64-pc-windows-gnu` |
| MinGW WinLibs (POSIX UCRT) | `gcc` para linkar; **`windres`** para o ícone do Explorer |
| PowerShell 5+ | Scripts em `scripts\` |
| Git | Histórico / GitHub |
| (opcional) winget | `BrechtSanders.WinLibs.POSIX.UCRT` no `setup-windows.ps1` |
| (opcional) Codegraph | Índice `.codegraph/` para agentes |

`build.rs` **falha** se `windres` não estiver no PATH. Por isso o alvo é GNU, não MSVC.

## Crates

Definidas em [`Cargo.toml`](../Cargo.toml):

| Crate | Papel |
|-------|--------|
| `eframe` 0.37 (`glow`, `default_fonts`) | Janela, viewports, loop da UI |
| `egui` 0.37 | Widgets, tema, cursores |
| `windows` 0.61.3 | Win32: gancho, clipboard, bandeja, registro, DWM, diálogos |
| `rusqlite` 0.32 `bundled` | SQLite embutido (expansor) — sem DLL do sistema |
| `zip` 2.2 `deflate` | Backup / import `b-snip-AAAA-MM-DD.zip` |
| `image` 0.25 `png` | Ícone PNG → RGBA e, no build, ICO DIB |
| `raw-window-handle` 0.6 | HWND da janela egui para DWM / always-on-top |

SQLite **bundled** é obrigatório: o PC destino não precisa ter SQLite instalado.

## APIs Win32 usadas

| API / constante | Para quê |
|-----------------|----------|
| `SetWindowsHookExW(WH_KEYBOARD_LL)` | Atalhos globais e hotstrings |
| `SendInput` | Backspace do expansor, Ctrl+V, transform |
| `ToUnicode` / estado de teclas | Buffer do expansor **sem** engolir dead keys |
| `AddClipboardFormatListener` + `WM_CLIPBOARDUPDATE` | Fila |
| `OpenClipboard` / `GetClipboardData` / `SetClipboardData` | Pacote Unicode + HTML + RTF + PNG + DIB |
| `CF_UNICODETEXT`, `CF_HDROP`, `CF_DIB`, `CF_BITMAP` | Texto vs arquivo vs print |
| `Shell_NotifyIconW` | Bandeja |
| `CreateMutexW` | Instância única |
| `RegSetValueExW` (Run) | Iniciar com o sistema |
| `SetTimer` / `KillTimer` | 2º Ctrl+C no mesmo texto; a cada 30 s reinstala gancho e listener de clipboard |
| `GetOpenFileNameW` / `GetSaveFileNameW` | Backup zip, sempre na frente do expansor |
| `DwmSetWindowAttribute` | Sem animação / cantos; host invisível |
| `mciSendStringW` | Som `assets/sondapp.mp3` só na limpeza |
| `AllowSetForegroundWindow` | Colar no app que está na frente |

## Assets obrigatórios

| Arquivo | Uso |
|---------|-----|
| [`assets/b-snip.png`](../assets/b-snip.png) | Ícone (bandeja, janelas, ICO do EXE) |
| [`assets/sondapp.mp3`](../assets/sondapp.mp3) | Som de limpar fila (embutido no EXE) |

`build.rs` recorta o quadrado do logo (PNG widescreen), gera ICO 16/32/48/256 em BMP DIB e liga com `windres`. PNG-in-ICO o Explorer GNU não mostra direito.

## Dados em runtime (não vão no EXE)

Ver [DADOS.md](DADOS.md). Resumo: `%LOCALAPPDATA%\b-snip\` ou pasta do EXE se existir `portable.txt`.

## O que *não* entra no produto

- Runtime .NET, Python, Electron, AutoHotkey
- Instalador MSI / serviço
- Linux / macOS
- Editor tipo Word no expansor (cola o pacote pronto do clipboard)
- Captura de arquivo/pasta/print na fila
