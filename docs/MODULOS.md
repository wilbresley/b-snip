# Mapa dos módulos

Entrada: [`src/main.rs`](../src/main.rs). Build do ícone: [`build.rs`](../build.rs).

| Arquivo | Responsabilidade |
|---------|------------------|
| [`src/main.rs`](../src/main.rs) | Instância única, migração, carrega INI+SQLite, sobe gancho + eframe |
| [`src/app.rs`](../src/app.rs) | `eframe::App`, host invisível, DWM, welcome, restart |
| [`src/state.rs`](../src/state.rs) | `AppState`, fila, `copy_gen`, toasts, persist |
| [`src/hook.rs`](../src/hook.rs) | Gancho, bandeja, worker, atalhos, timer do 2º Ctrl+C |
| [`src/clipboard.rs`](../src/clipboard.rs) | Captura/grava/limpa clipboard; `CLIP_GATE`; pausa só em arquivo ou imagem sem texto |
| [`src/payload.rs`](../src/payload.rs) | `ClipPayload`, limites 2/32/8 MB, mensagens de fila cheia |
| [`src/input.rs`](../src/input.rs) | `SendInput`, beep, espera modificadores |
| [`src/ui.rs`](../src/ui.rs) | Fila, expansor, editor, ajuda, toasts, tema |
| [`src/text.rs`](../src/text.rs) | Title/upper/lower, HTML visível, `!data`, CF_HTML |
| [`src/config.rs`](../src/config.rs) | INI, features, 3–100 linhas |
| [`src/store.rs`](../src/store.rs) | SQLite do expansor |
| [`src/paths.rs`](../src/paths.rs) | `%LOCALAPPDATA%\b-snip`, `portable.txt`, legado `b-abcABC` |
| [`src/startup.rs`](../src/startup.rs) | Run key `b-snip` (herda `b-abcABC`) |
| [`src/backup.rs`](../src/backup.rs) | Zip INI+DB |
| [`src/dialog.rs`](../src/dialog.rs) | File picker na frente do always-on-top |
| [`src/icon.rs`](../src/icon.rs) | PNG → HICON / egui |
| [`src/sound.rs`](../src/sound.rs) | MP3 via MCI, só na limpeza |

Constantes de produto: `APP_NAME = "b-snip"` em `paths.rs`. Versão: `1.0.0` em [`Cargo.toml`](../Cargo.toml).
