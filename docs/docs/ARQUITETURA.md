# Arquitetura

Processo único, três threads, um mutex de estado. O desenho existe para o gancho de teclado **nunca** travar e o clipboard **nunca** ser aberto em paralelo.

```
                    ┌─────────────────────────────────────┐
                    │  main  (eframe / egui / glow)       │
                    │  viewports: fila, expansor, ajuda,  │
                    │  transform, toast, “já em execução” │
                    └──────────────┬──────────────────────┘
                                   │ Arc<Mutex<AppState>>
          ┌────────────────────────┼────────────────────────┐
          ▼                        ▼                        ▼
 ┌─────────────────┐    ┌──────────────────┐    ┌─────────────────────┐
 │ hook / mensagem │    │ worker           │    │ UI (já acima)       │
 │ WH_KEYBOARD_LL  │──► │ OpenClipboard    │    │ request_repaint     │
 │ bandeja, timer  │    │ fila, paste,     │    │                     │
 │ WM_CLIPBOARDUPD │    │ expand, toast    │    │                     │
 └─────────────────┘    └──────────────────┘    └─────────────────────┘
```

## Threads

| Thread | Nome | Faz | Não faz |
|--------|------|-----|---------|
| UI | eframe | Desenha, tema, viewports | Clipboard, SendInput pesado |
| Mensagem | `b-snip-hook` | Gancho, bandeja, `WM_CLIPBOARDUPDATE`, `WM_TIMER`, um `try_set` no Ctrl+V | Esperar o clipboard, loop de `OpenClipboard` |
| Worker | `b-snip-worker` | Captura/grava clipboard, cola de fallback, expansor, toasts | Bloquear o gancho |

Comandos passam por `sync_channel` (`WorkCmd`). O gancho só faz `try_lock` + `try_send`, exceto o `try_set_payload` do Ctrl+V (abaixo). Um `CLIP_GATE` serializa quem abre o clipboard: o worker espera; o gancho desiste na hora se o portão estiver ocupado.

## Fila de cópia

1. `Ctrl+C` / `Ctrl+X` → `copy_gen += 1` e `SetTimer` (~200 ms) na janela de mensagem
2. O Windows grava o clipboard
3. `WM_CLIPBOARDUPDATE` → worker captura o **pacote** (`ClipPayload`)
4. Conteúdo **novo** entra na fila; UPDATE duplicado do Windows é ignorado
5. Se o 2º `Ctrl+C` for o **mesmo** texto (Windows não manda UPDATE), o timer manda `UserCopy` e o item entra de novo
6. Arquivo, pasta (`CF_HDROP`) ou print puro (bitmap/DIB/PNG sem texto e sem HTML/RTF) pausam a fila se ela já tem itens, e não entram. Texto que também traz um bitmap de preview entra normal
7. `Ctrl+V` com fila ativa (`copy_on && !paused && !queue.is_empty()`) grava o próximo item no clipboard e **deixa a tecla física passar**. Jogo que ignora tecla injetada cola como o Windows. Se o `CLIP_GATE` estiver ocupado, o gancho engole a tecla e o worker cola com `SendInput` (Bloco de Notas)
8. Repetição de tecla (segurar V) não avança a fila. No soltar, se esse colar esvaziou a fila com “limpar ao colar o último”, o worker espera ~150 ms e esvazia o clipboard do Windows
9. `is_pasting` preso mais de 3 s é solto, para a fila voltar a aceitar cópia sem desligar o programa

Pacote (`src/payload.rs`): `unicode` + `html` + `rtf` + `png` + `dib`. O Ctrl+V real devolve o pacote inteiro; `Ctrl+Shift+V` grava só o unicode e deixa a tecla passar. O fallback `SendInput` faz o mesmo.

**Não** abrir o clipboard numa `thread::spawn` no meio da cópia: isso fechava o processo (o 1.0.0 espera o Windows gravar com `SetTimer` e limita o UTF-16 a `GlobalSize`).

A cada 30 s a thread do gancho reinstala `WH_KEYBOARD_LL` e o `AddClipboardFormatListener`. O Windows tira os dois em silêncio depois de bloqueio de tela ou suspensão; o ícone da bandeja continua. Se o V estiver apertado, essa volta espera o próximo ciclo.

## Expansor

Buffer de digitação no gancho (`TYPE_BUF`, 64 chars). Match de gatilho → `WorkCmd::Expand`: backspaces, grava o pacote no clipboard, `Ctrl+V`, marca `skip_clip` para **não** entrar na fila (`!data` incluso).

Conteúdo mora no SQLite (`expander.db`), não na RAM além do que a UI precisa. 3 linhas no começo, `+` até 100.

## Transformador

`Ctrl+Alt+T` copia a seleção, mostra 1/2/3/4, mapeia só o texto visível (HTML/RTF preservam tags), cola no lugar. **Não** reseleciona. `skip_clip` para não enfileirar.

## UI

Host eframe 1×1 px, fora da tela, `WS_EX_TOOLWINDOW` + `NOACTIVATE` nas janelas “Fila de Cópia” / host, para não piscar na primeira cópia. Viewports always-on-top, centradas no monitor do mouse. Diálogos nativos de arquivo baixam o topmost na hora do picker.

## Instância única

Mutex `Local\b-snip-ClipboardExpander`. Segunda abertura: viewport egui “já em execução”, não `MessageBox`.
