import { spawn } from "node:child_process";
import os from "node:os";
import path from "node:path";

const cargoBin = path.join(os.homedir(), ".cargo", "bin");
for (const key of ["Path", "PATH"]) {
  const cur = process.env[key] || "";
  if (!cur.toLowerCase().includes(cargoBin.toLowerCase())) {
    process.env[key] = `${cargoBin}${path.delimiter}${cur}`;
  }
}

const args = process.argv.slice(2);
const tauriCliJs = path.resolve("node_modules", "@tauri-apps", "cli", "tauri.js");

const child = spawn(process.execPath, [tauriCliJs, ...args], {
  stdio: "inherit",
  env: process.env,
});

child.on("exit", (code) => {
  process.exit(code ?? 0);
});
