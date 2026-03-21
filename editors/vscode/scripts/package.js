import { execFileSync, execSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(new URL(".", import.meta.url).pathname, "..", "..", "..");
const extDir = path.join(root, "editors", "vscode");
const pkgPath = path.join(extDir, "package.json");
const serverDir = path.join(extDir, "server");

// Usage:
//   node package.js --binary-path target/x86_64-unknown-linux-gnu/release/mips-language-server --os linux --cpu x64
const args = process.argv.slice(2);
function getArg(name) {
  const idx = args.indexOf(name);
  return idx >= 0 ? args[idx + 1] : undefined;
}

const binaryPath = getArg("--binary-path");
const os = getArg("--os");
const cpu = getArg("--cpu");
const outDir = getArg("--outDir") || path.join(extDir, "vsix");

if (!binaryPath || !os || !cpu) {
  console.error(
    "Missing args. Example:\n" +
      "  node scripts/package.js --binary-path target/x86_64-unknown-linux-gnu/release/mips-language-server --os linux --cpu x64\n",
  );
  process.exit(2);
}

const exeName = path.basename(binaryPath);

// Read + patch package.json (os/cpu)
const pkgRaw = fs.readFileSync(pkgPath, "utf8");
const pkg = JSON.parse(pkgRaw);

// Keep a backup to restore later
const restore = () => fs.writeFileSync(pkgPath, pkgRaw);

try {
  // Patch package.json with os/cpu
  pkg.os = [os];
  pkg.cpu = [cpu];
  fs.writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n");

  // Check that the release binary exists
  const builtPath = path.isAbsolute(binaryPath) 
    ? binaryPath 
    : path.join(root, binaryPath);

  if (!fs.existsSync(builtPath)) {
    throw new Error(`Release binary not found at: ${builtPath}\nPlease build it first with cargo.`);
  }

  // Ensure server directory exists, clean it
  fs.rmSync(serverDir, { recursive: true, force: true });
  fs.mkdirSync(serverDir, { recursive: true });

  const destPath = path.join(serverDir, exeName);
  fs.copyFileSync(builtPath, destPath);
  fs.chmodSync(destPath, 0o755);

  // Build extension JS bundle
  execSync(`npm ci`, { cwd: extDir, stdio: "inherit" });
  execSync(`npm run build`, { cwd: extDir, stdio: "inherit" });

  // Package VSIX with proper target platform
  fs.mkdirSync(outDir, { recursive: true });
  
  // Map os/cpu to vscode target platform
  const targetMap = {
    'linux-x64': 'linux-x64',
    'linux-arm64': 'linux-arm64',
    'darwin-x64': 'darwin-x64',
    'darwin-arm64': 'darwin-arm64',
    'win32-x64': 'win32-x64',
  };
  const vsceTarget = targetMap[`${os}-${cpu}`];
  if (!vsceTarget) {
    throw new Error(`Unknown platform combination: ${os}-${cpu}`);
  }
  
  execSync(`npx vsce package --no-dependencies --target ${vsceTarget}`, { cwd: extDir, stdio: "inherit" });

  // Move produced vsix into outDir (pick newest .vsix)
  const vsixFiles = fs
    .readdirSync(extDir)
    .filter((f) => f.endsWith(".vsix"))
    .map((f) => ({ f, t: fs.statSync(path.join(extDir, f)).mtimeMs }))
    .sort((a, b) => b.t - a.t);

  if (vsixFiles.length === 0) throw new Error("No .vsix produced by vsce.");

  const produced = path.join(extDir, vsixFiles[0].f);
  const outputName = `${pkg.publisher}.${pkg.name}-${pkg.version}-${os}-${cpu}.vsix`;
  const finalPath = path.join(outDir, outputName);

  fs.renameSync(produced, finalPath);
  console.log(`\nCreated: ${finalPath}\n`);
} finally {
  restore();
}
