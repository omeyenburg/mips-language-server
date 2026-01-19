# mips-language-server

A fast language server for the MIPS32/MIPS64 Instruction Set Architecture.  
Supported assemblers: GAS, MARS, SPIM

Written in Rust with [tower-lsp-server](https://github.com/tower-lsp-community/tower-lsp-server) and [tree-sitter](https://github.com/tree-sitter/tree-sitter).

## Features
- Completion
- Hover information

## Planned Features
- Better parsing & linting of instructions
- Goto definition
- Context specific completions
- Formatting
- References
- Documentation
- Macros

## Installation

Currently, you have to build the source yourself.

Clone the repo and simply run:
```
cargo build --release
```

You can find the resulting binary in `target/release/`.

## Configuration

### Settings

#### dialect

The dialect of your assembler/simulator (not case-sensitive).

Available: mars, spim, gas, unspecified

#### version

Mips ISA Version of you assembler/simulator. Not relevant if using mars or spim (not case-sensitive).

Available:
- Legacy MIPS versions: 'mips1' (Mips I), 'mips2', (Mips II), 'mips3', (Mips III), 'mips4' (Mips IV), 'mips5' (Mips V)
- MIPS32 versions:      'mips32r1',       'mips32r2',         'mips32r3',          'mips32r5',        'mips32r6'
- MIPS64 versions:      'mips64r1',       'mips64r2',         'mips64r3',          'mips64r5',        'mips64r6'

### Example Configuration

```json
{
    "settings": {
        "mipsls": {
            "dialect": "Mars",
            "version": "Mips I"
        }
    }
}
```

## Editor Integration

### NeoVim

With the nvim-lspconfig plugin:

```lua
local config = {
    cmd = { '/path/to/compiled/binary' },
    filetypes = { 'asm' },
    settings = {
        mipsls = {
            dialect = 'gas',
            version = 'mips32r5',
        }
    }
}

vim.lsp.config("mipsls", config)
vim.lsp.enable("mipsls")
```

## Testing
This was mostly tested with the latest NeoVim release together with the plugins lsp-config, nvim-cmp and blink.nvim.
Further testing in different environments is highly appreciated.

## Further reading
- MIPS64 Vol II: https://www.cipunited.com/xlx/files/document/202008/1205490289250.pdf
- MIPS Assembly/Instruction Formats: https://en.wikibooks.org/wiki/MIPS_Assembly/Instruction_Formats
- Assembly Language Programmer’s Guide: http://www.cs.unibo.it/~solmi/teaching/arch_2002-2003/AssemblyLanguageProgDoc.pdf
