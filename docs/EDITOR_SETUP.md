# Editor Setup

This guide covers syntax highlighting and LSP setup for `.proj` files.

## Available Files

```
syntax/
├── utf8proj.tmLanguage.json  # TextMate grammar (VS Code, Sublime, Zed)
├── proj.vim                   # Vim syntax (Neovim, Vim)
└── ftdetect/
    └── proj.vim              # Filetype detection for Vim
```

## Highlighted Elements

| Element | Scope | Example |
|---------|-------|---------|
| Block keywords | `keyword.declaration` | `project`, `task`, `resource`, `calendar` |
| Properties | `keyword.other.property` | `start:`, `effort:`, `depends:`, `assign:` |
| Dependency types | `constant.language.dependency-type` | `FS`, `SS`, `FF`, `SF` |
| Status keywords | `constant.language.status` | `not_started`, `in_progress`, `complete` |
| Days | `constant.language.day` | `mon`, `tue`, `wed` |
| Booleans | `constant.language.boolean` | `true`, `false` |
| Dates | `constant.numeric.date` | `2025-02-01` |
| Durations | `constant.numeric.duration` | `5d`, `2w`, `8h` |
| Percentages | `constant.numeric.percentage` | `50%`, `100%` |
| Numbers | `constant.numeric` | `850`, `1.5` |
| Strings | `string.quoted.double` | `"Project Name"` |
| Comments | `comment.line` | `# This is a comment` |
| Operators | `keyword.operator` | `..`, `+`, `-`, `@`, `*`, `/` |

## Editor Setup

### VS Code

1. Copy to VS Code extensions:
   ```bash
   mkdir -p ~/.vscode/extensions/utf8proj-syntax
   cp syntax/utf8proj.tmLanguage.json ~/.vscode/extensions/utf8proj-syntax/
   ```

2. Create `~/.vscode/extensions/utf8proj-syntax/package.json`:
   ```json
   {
     "name": "utf8proj-syntax",
     "version": "0.1.0",
     "engines": { "vscode": "^1.50.0" },
     "contributes": {
       "languages": [{
         "id": "proj",
         "extensions": [".proj"],
         "aliases": ["utf8proj", "proj"]
       }],
       "grammars": [{
         "language": "proj",
         "scopeName": "source.proj",
         "path": "./utf8proj.tmLanguage.json"
       }]
     }
   }
   ```

3. Restart VS Code.

### Neovim (Recommended)

Neovim provides both syntax highlighting and full LSP support (diagnostics, hover, go-to-definition, find-references).

**Syntax highlighting:**

```bash
mkdir -p ~/.config/nvim/syntax ~/.config/nvim/ftdetect
cp syntax/proj.vim ~/.config/nvim/syntax/
cp syntax/ftdetect/proj.vim ~/.config/nvim/ftdetect/
```

**LSP setup** (for full IDE features):

First, build the LSP server:
```bash
cd /path/to/utf8proj
cargo build --release -p utf8proj-lsp
```

Then configure in your `init.lua`:
```lua
require('lspconfig.configs').utf8proj = {
  default_config = {
    cmd = { '/path/to/utf8proj/target/release/utf8proj-lsp' },
    filetypes = { 'proj' },
    root_dir = function() return vim.fn.getcwd() end,
  },
}
require('lspconfig').utf8proj.setup{}
```

Restart Neovim and open a `.proj` file.

### Traditional Vim (Syntax Only)

Traditional Vim has no built-in LSP support. You get syntax highlighting only.

```bash
mkdir -p ~/.vim/syntax ~/.vim/ftdetect
cp syntax/proj.vim ~/.vim/syntax/
cp syntax/ftdetect/proj.vim ~/.vim/ftdetect/
```

Restart Vim and open a `.proj` file.

> **Note:** For LSP features in Vim, you'd need plugins like `vim-lsp` or `coc.nvim`. This is outside utf8proj scope. We recommend using Neovim for the full experience.

### Zed

1. Copy grammar to Zed extensions directory
2. Configure in `settings.json`:
   ```json
   {
     "languages": {
       "proj": {
         "tab_size": 4
       }
     }
   }
   ```

### Sublime Text

Copy `syntax/utf8proj.tmLanguage.json` to:
- macOS: `~/Library/Application Support/Sublime Text/Packages/User/`
- Linux: `~/.config/sublime-text/Packages/User/`
- Windows: `%APPDATA%\Sublime Text\Packages\User\`

## Example

```proj
# CRM Migration Project
project "CRM Migration" {
    start: 2025-02-01
    currency: USD
}

calendar "standard" {
    working_days: mon-fri
    working_hours: 09:00-17:00
    holiday "New Year" 2025-01-01
}

resource dev "Developer" {
    rate: 850/day
    capacity: 1.0
}

task design "Design Phase" {
    task wireframes "Wireframes" {
        effort: 3d
        assign: dev
    }
    task mockups "Mockups" {
        effort: 5d
        assign: dev@50%
        depends: wireframes +1d
    }
}

milestone launch "Go Live" {
    depends: design FF
}
```
