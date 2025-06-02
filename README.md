# mips-language-server
A fast language server for the MIPS Instruction Set.
Written in Rust with tower-lsp (async) / lsp-server (sync) and tree-sitter.
This language server is specific to the 32 bit instruction set

## ⚡️ Features
- General completions for instructions, registers and directives
- Hover information for instructions, registers and directives
- Goto definition for jump labels

## 📋 Planned features
- Better parsing & linting of instructions
- Context specific completions
- Formatting
- References
- Documentation
- Possibly macros (like in Mars)
- 64-bit instruction set

## Building from source
Rust is required.
Run `cargo build --features "async"` (or with `--features "sync"`)

## 📈 Testing
This was mostly tested with the latest neovim release together with the plugins lsp-config, nvim-cmp and blink.nvim.
Further testing in different environments is highly appreciated.

## Further resources
- https://www.cs.cornell.edu/courses/cs3410/2008fa/MIPS_Vol2.pdf
- http://www.cs.unibo.it/~solmi/teaching/arch_2002-2003/AssemblyLanguageProgDoc.pdf
- https://en.wikibooks.org/wiki/MIPS_Assembly/Instruction_Formats
