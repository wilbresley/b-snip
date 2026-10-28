# Dados e persistência

O EXE não guarda fila de cópia em disco (é RAM, some ao sair). O que sobrevive a reboot é **expansor + flags + posição das janelas**.

## Onde fica

| Modo | Pasta |
|------|--------|
| Padrão | `%LOCALAPPDATA%\b-snip\` |
| Portátil | Pasta do EXE, se existir `portable.txt` ao lado |

A aba **Dados** da ajuda liga/desliga o portátil (cria/apaga o flag). Sem permissão (AD / Program Files) o interruptor não muda e avisa.

Migração na 1ª execução: se a pasta nova estiver vazia, copia `%LOCALAPPDATA%\b-abcABC\` e o valor Run antigo.

## Arquivos

| Arquivo | Conteúdo |
|---------|----------|
| `ClipboardExpander.ini` | Features, posições, gatilhos/nomes, `WelcomeShown` |
| `expander.db` | Pacotes ricos (unicode/html/rtf/png/dib), WAL |
| `portable.txt` | Só o flag, ao lado do EXE |
| `open_expander.flag` | Sentinela pós-import (some na abertura) |

INI **não** é a fonte da verdade do conteúdo rico. Depois do primeiro save, o SQLite manda; o INI fica com nomes/gatilhos/flags.

## SQLite (`snippets`)

```
sort INTEGER PRIMARY KEY
trigger, name, unicode
html, rtf, png, dib  (BLOB)
bytes
```

`PRAGMA journal_mode=WAL`. Reiniciar o PC **não** apaga. Teto 8 MB por linha, 3–100 linhas.

## Backup

Na janela do expansor: exporta zip `b-snip-AAAA-MM-DD.zip` (INI + DB). Importar pede confirmação **no tema do app** (não MessageBox atrás), copia, reinicia, reabre o expansor.

## Fila (só memória)

- 100 itens, 2 MB cada, 32 MB no total
- Aviso ~80%; no teto não entra mais nada (não sobrescreve o mais antigo)
- Limpar (`Ctrl+Alt+R`, desligar cópia, ou “limpar ao colar o último”) zera a fila **e** o clipboard do Windows
