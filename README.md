# mips-language-server

A fast language server for the MIPS32/MIPS64 Instruction Set Architecture.  
Supported assemblers: GAS, MARS, SPIM

Written in Rust with [tower-lsp-server](https://github.com/tower-lsp-community/tower-lsp-server) and [tree-sitter](https://github.com/tree-sitter/tree-sitter).

## Features
- Completion
- Hover information

## Planned features
- Better parsing & linting of instructions
- Goto definition
- Context specific completions
- Formatting
- References
- Documentation
- Macros

## Testing
This was mostly tested with the latest NeoVim release together with the plugins lsp-config, nvim-cmp and blink.nvim.
Further testing in different environments is highly appreciated.

## Further reading
- MIPS64 Vol II: https://www.cipunited.com/xlx/files/document/202008/1205490289250.pdf
- MIPS Assembly/Instruction Formats: https://en.wikibooks.org/wiki/MIPS_Assembly/Instruction_Formats
- Assembly Language Programmer’s Guide: http://www.cs.unibo.it/~solmi/teaching/arch_2002-2003/AssemblyLanguageProgDoc.pdf
