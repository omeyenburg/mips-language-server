# mips-language-server

A fast language server for the MIPS32/MIPS64 Instruction Set Architecture.

Supported assemblers: GAS, MARS, SPIM

Written in Rust with [tower-lsp-server](https://github.com/tower-lsp-community/tower-lsp-server) and [tree-sitter](https://github.com/tree-sitter/tree-sitter).

## Features
- Completions for instructions, registers and directives
- Hover information for instructions, registers and directives
- Goto definition for jump labels

## Planned features
- Better parsing & linting of instructions
- Context specific completions
- Formatting
- References
- Documentation
- Macros
- Support for pseudo instructions (per assembler)
- Support for 64-bit instruction set

## Testing
This was mostly tested with the latest neovim release together with the plugins lsp-config, nvim-cmp and blink.nvim.
Further testing in different environments is highly appreciated.

## Further reading
- MIPS64 VolII: https://www.cipunited.com/xlx/files/document/202008/1205490289250.pdf
- MIPS Assembly/Instruction Formats: https://en.wikibooks.org/wiki/MIPS_Assembly/Instruction_Formats
- Assembly Language Programmer’s Guide: http://www.cs.unibo.it/~solmi/teaching/arch_2002-2003/AssemblyLanguageProgDoc.pdf
