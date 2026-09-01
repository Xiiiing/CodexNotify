import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const desktop = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const root = resolve(desktop, "../..");
execFileSync("cargo", ["build", "-p", "codex-notify-hook"], { cwd: root, stdio: "inherit" });
const triple = execFileSync("rustc", ["--print", "host-tuple"], { encoding: "utf8" }).trim();
const extension = process.platform === "win32" ? ".exe" : "";
const source = resolve(root, `target/debug/codex-notify-hook${extension}`);
const destination = resolve(desktop, `src-tauri/binaries/codex-notify-hook-${triple}${extension}`);
mkdirSync(dirname(destination), { recursive: true });
copyFileSync(source, destination);
console.log(`Staged ${destination}`);
