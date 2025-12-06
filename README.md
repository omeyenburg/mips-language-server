# mips-language-server

A fast language server for the MIPS Instruction Set Architecture. Currently only supporting the 32 bit instruction set.

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
- https://www.cs.cornell.edu/courses/cs3410/2008fa/MIPS_Vol2.pdf
- http://www.cs.unibo.it/~solmi/teaching/arch_2002-2003/AssemblyLanguageProgDoc.pdf
- https://en.wikibooks.org/wiki/MIPS_Assembly/Instruction_Formats
