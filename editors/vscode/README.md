# MIPS Tools

Language support for MIPS assembly with intelligent code completion, hover documentation, goto definition, and real-time diagnostics.

## Features

- **Completions** - Suggestions for instructions, directives, and registers
- **Hover Documentation** - Documentation for instructions, directives and registers on hover
- **Goto Definition** - Jump to labels and macro definitions
- **Syntax Highlighting** - Syntax coloring for MIPS assembly
- **Diagnostics** - Syntax errors and semantic errors

## Supported Assemblers

- **GAS** (GNU Assembler) - Full MIPS32/MIPS64 ISA support
- **MARS** (MIPS Assembler and Runtime Simulator)
- **SPIM** (MIPS32 Simulator)

## Installation

Install directly from the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=omeyenburg.mips-tools) or [Open VSX](https://open-vsx.org/extension/omeyenburg/mips-tools).

Alternatively, download the `.vsix` file for your platform from the [GitHub Releases](https://github.com/omeyenburg/mips-language-server/releases) and install manually:

```bash
code --install-extension omeyenburg.mips-tools-*.vsix
```

## Configuration

Configure the extension via VS Code settings:

### `mips.version.dialect`

Select your MIPS assembler/simulator:
- `gas` - GNU Assembler (default)
- `mars` - MARS simulator
- `spim` - SPIM simulator
- `undefined` - Auto-detect

### `mips.version.revision`

MIPS ISA version (only relevant for GAS):
- MIPS32: `mips32r1`, `mips32r2`, `mips32r3`, `mips32r5`, `mips32r6`
- MIPS64: `mips64r1`, `mips64r2`, `mips64r3`, `mips64r5`, `mips64r6`
- Legacy: `mips1`, `mips2`, `mips3`, `mips4`, `mips5`

Default: `mips64r5`

### `mips.server.path`

Custom path to the language server binary. Leave empty to use the bundled server.

### Example Configuration

```json
{
  "mips.version.dialect": "mars",
  "mips.version.revision": "mips32r2"
}
```

## File Extensions

The extension activates for files with the following extensions:
- `.asm`
- `.s`
- `.mips`

## Links

- [GitHub Repository](https://github.com/omeyenburg/mips-language-server)
- [Issue Tracker](https://github.com/omeyenburg/mips-language-server/issues)
- [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=omeyenburg.mips-tools)
- [Open VSX](https://open-vsx.org/extension/omeyenburg/mips-tools)
