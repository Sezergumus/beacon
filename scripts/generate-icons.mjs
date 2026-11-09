import { execFileSync } from "node:child_process";
import { copyFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const icons = path.join(root, "src-tauri", "icons");
const cli = path.join(root, "node_modules", "@tauri-apps", "cli", "tauri.js");

// Always derive icons from the clean RGBA master, never the white-backed JPEG.
execFileSync(
  process.execPath,
  [cli, "icon", path.join(icons, "icon-manifest.json"), "--output", icons],
  { cwd: root, stdio: "inherit" },
);
copyFileSync(path.join(icons, "app-icon.png"), path.join(root, "public", "beacon-icon.png"));
