# utf8proj-lsp

Language Server Protocol implementation for utf8proj project files.

## LSP v0 Scope

This is the initial release of the utf8proj language server. It provides foundational IDE features for `.proj` files.

### Supported Features

| Feature | Status | Description |
|---------|--------|-------------|
| **Diagnostics** | Full | Real-time parse errors and semantic warnings |
| **Hover** | Full | Task info with schedule dates, slack, critical path |
| **Document Symbols** | Full | Navigate profiles, resources, tasks |

### Diagnostic Parity

LSP diagnostics use the same analysis engine as the CLI (`utf8proj check`):
- Same diagnostic codes (E001, W001, H001, I001)
- Same messages and notes
- Severity mapping: Error/Warning/Hint/Info

The LSP does **not** support CLI policy options (`--strict`, `--quiet`) since these are CI/CD concerns, not IDE concerns.

### File Format Support

| Format | Extension | Status |
|--------|-----------|--------|
| Native DSL | `.proj` | Supported |
| TaskJuggler | `.tjp` | Not supported |

TJP support may be added in a future version.

### Not Yet Implemented

The following features are planned for future versions:

- Go to Definition (for task/resource references)
- Find References
- Code Completion (keywords, identifiers)
- Rename Symbol
- Workspace-wide analysis

## Usage

### Neovim (with nvim-lspconfig)

```lua
local lspconfig = require('lspconfig')
local configs = require('lspconfig.configs')

if not configs.utf8proj then
  configs.utf8proj = {
    default_config = {
      cmd = { 'utf8proj-lsp' },
      filetypes = { 'proj' },
      root_dir = lspconfig.util.find_git_ancestor,
      settings = {},
    },
  }
end

lspconfig.utf8proj.setup{}
```

Add filetype detection in `~/.config/nvim/filetype.lua`:
```lua
vim.filetype.add({
  extension = {
    proj = 'proj',
  },
})
```

### VS Code

A VS Code extension is not yet available. Contributions welcome.

## Building

```bash
cargo build --release -p utf8proj-lsp
```

The binary will be at `target/release/utf8proj-lsp`.

## Protocol Details

- Transport: stdio
- Sync mode: Full document sync
- Capabilities: textDocument/hover, textDocument/publishDiagnostics, textDocument/documentSymbol
