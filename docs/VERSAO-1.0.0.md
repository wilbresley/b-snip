# b-snip 1.0.0

Primeira versão estável. Outubro de 2026. `Cargo.toml` → `version = "1.0.0"`.

Um EXE de clipboard para o Windows: fila, pausa, texto rico, expansor, SQLite, modo portátil e bandeja. Esta 1.0.0 já inclui o Ctrl+V com a tecla física e a fila que não pausa em texto.

## Entrega

- Um `b-snip.exe` (~8 MB), Windows 10/11 x64, GNU, LTO
- Ícone `assets/b-snip.png` no EXE, na bandeja e nas janelas
- Dados em `%LOCALAPPDATA%\b-snip\` (herda `b-abcABC`) ou `portable.txt`
- Instância única, tema `#2b2b2b`, ajuda na 1ª execução

## Funções nesta versão

**Fila** — FIFO Unicode+HTML+RTF+imagem; Loop (padrão) ou cola-e-acaba; “Limpar ao colar o último” xor Loop; pausa manual, e automática só em arquivo, pasta ou print sem texto; texto com imagem entra; `Ctrl+C` duas vezes no mesmo texto entra duas vezes; `Ctrl+V` usa a tecla física (se o clipboard estiver ocupado, usa tecla fabricada); `Ctrl+Shift+V` texto puro; teto 100 / 2 MB / 32 MB. O gancho e o aviso de clipboard são recolocados a cada 30 s. Colagem presa solta em 3 s.

**Expansor** — 3–100 linhas, gatilho + nome + pacote rico no SQLite, editor 800×900, `!data`, backup zip, import com confirmação e restart.

**Transform** — 1/2/3 no lugar, rico preservado, sem reselecionar.

**Bandeja** — esquerdo inteligente (pausa se tem fila, senão liga/desliga cópia); direito com categorias; iniciar com o sistema; som só na limpeza.

## Fora desta versão (de propósito)

- Linux / macOS
- Editor WYSIWYG tipo Word
- Fila persistida em disco
- Assinatura Authenticode
- Instalador

## Como o clipboard é lido

O segundo Ctrl+C no mesmo texto espera o Windows gravar (`SetTimer`) e limita o UTF-16 a `GlobalSize`. Não há thread extra lendo o clipboard no meio da cópia.
