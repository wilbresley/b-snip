# Guia para desenvolvedores

Como clonar, gerar o `b-snip.exe` e não quebrar o que já funciona. Windows 10/11, PowerShell 5+.

## O que você precisa

| Peça | Por quê |
|------|---------|
| Windows 10 ou 11 (x64) | O app é Win32; não há Linux/macOS |
| [Rustup](https://rustup.rs) + toolchain **stable GNU** | `x86_64-pc-windows-gnu` |
| MinGW com `gcc` **e** `windres` | `build.rs` embute o `.ico` via `windres` |
| Git + (opcional) GitHub CLI | Fork / push |
| Rede na 1ª compilação | crates.io + `rusqlite` bundled (SQLite) |

Script que instala o que faltar:

```powershell
.\scripts\setup-windows.ps1
```

Alvo fixo em [`.cargo/config.toml`](../.cargo/config.toml) e [`rust-toolchain.toml`](../rust-toolchain.toml): `x86_64-pc-windows-gnu`.

MSVC (`x86_64-pc-windows-msvc`) **não** é o caminho deste repo: o ícone do Explorer depende do `windres` GNU.

## Scripts

Todos em [`scripts/`](../scripts/). Atalho na raiz: [`build-release.ps1`](../build-release.ps1).

| Script | Faz |
|--------|-----|
| `scripts\setup-windows.ps1` | rustup + alvo GNU + WinLibs (gcc/windres) |
| `scripts\build-release.ps1` | `cargo build --release` → `dist\b-snip.exe` |
| `scripts\build-debug.ps1` | build debug |
| `scripts\test.ps1` | `cargo test` |
| `scripts\run.ps1` | release + inicia o EXE |
| `scripts\_common.ps1` | PATH do MinGW / cargo (não rode sozinho) |

```powershell
.\scripts\build-release.ps1                 # gera dist\b-snip.exe
.\scripts\build-release.ps1 -CleanUserData  # idem e apaga %LOCALAPPDATA%\b-snip (teste “primeira vez”)
.\scripts\test.ps1
.\scripts\run.ps1
```

O build **encerra** `b-snip.exe` se estiver aberto (o Explorer trava o arquivo). Não apaga dados do usuário, salvo `-CleanUserData`.

## Fork: gerar o *seu* programa

1. Clone o repo
2. Rode `.\scripts\setup-windows.ps1`
3. Troque nome, mutex e pastas (veja abaixo)
4. Troque [`assets/b-snip.png`](../assets/b-snip.png) — o `build.rs` gera o `.ico` a cada compile
5. `.\scripts\build-release.ps1`
6. Distribua só o `dist\b-snip.exe` (e, se quiser pendrive, um `portable.txt` vazio ao lado)

### Onde o nome aparece

Se for rebatizar (ex.: `b-clip`):

| Lugar | Constante / valor |
|-------|-------------------|
| [`Cargo.toml`](../Cargo.toml) | `name`, `[[bin]].name` |
| [`src/paths.rs`](../src/paths.rs) | `APP_NAME`, `LEGACY_APP_FOLDER` |
| [`src/hook.rs`](../src/hook.rs) | mutex `Local\\b-snip-ClipboardExpander`, classe da janela de mensagem |
| [`src/startup.rs`](../src/startup.rs) | valor Run `b-snip` |
| [`src/main.rs`](../src/main.rs) | título do viewport |
| [`src/ui.rs`](../src/ui.rs) | títulos visíveis |
| [`assets/b-snip.png`](../assets/b-snip.png) | ícone |
| [`build.rs`](../build.rs) | `cargo:rerun-if-changed=assets/b-snip.png` |

Não mude o mutex e o `APP_NAME` pela metade: duas instâncias “diferentes” passam a conviver e brigam pelo clipboard.

## Testes

```powershell
.\scripts\test.ps1
```

Os testes cobrem limites da fila, INI, mapeamento de texto/HTML, zip de backup e a regra “mesmo texto só entra de novo com outro Ctrl+C”. Gancho, clipboard real e UI não têm teste automático (precisam do Windows ao vivo).

## Perfil de release

Em [`Cargo.toml`](../Cargo.toml):

```toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "unwind"
strip = true
```

`panic = "unwind"` de propósito: abort no gancho de teclado mata o processo sem diálogo (`windows_subsystem = "windows"`).

## Codegraph (opcional)

Se o MCP Codegraph estiver no Cursor:

```
codegraph init
```

Índice em `.codegraph/` (gitignored). Quem for mexer no código consulta `codegraph_explore` antes de procurar em série.

## Armadilhas

- **Dois `OpenClipboard` ao mesmo tempo** derrubam o processo. Tudo passa pelo `CLIP_GATE`. O worker espera; o gancho só tenta uma vez no Ctrl+V e, se o portão estiver ocupado, cai no `SendInput`.
- **Gancho `WH_KEYBOARD_LL`** não pode bloquear: `try_lock` + `try_send`. Nada de loop esperando o clipboard no callback.
- O Windows remove o gancho em silêncio depois de bloqueio de tela ou suspensão. A thread de mensagem reinstala gancho e listener a cada 30 s.
- Acentos / dead keys: o gancho **não** consome teclas comuns. Com a fila ativa o Ctrl+V físico segue (o item já está no clipboard). Transform e expansor continuam atalhos do programa.
- Instância única: mutex `Local\b-snip-ClipboardExpander`. Segunda abertura mostra janela “já em execução” no tema do app.
- Programas elevados (Admin) não vêem o gancho se o b-snip não estiver elevado — igual ao AHK.
- Alguns antivírus acusam hook + `SendInput`. Assinar o EXE ajuda se for distribuir.
