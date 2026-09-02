import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const desktop = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const source = resolve(desktop, "src/assets/app-icon.png");
const destination = resolve(desktop, "src-tauri/icons");
const temporary = mkdtempSync(resolve(tmpdir(), "codexnotify-icons-"));
try {
  const normalized = resolve(temporary, "app-icon-square.png");
  await sharp(source)
    .trim({ background: { r: 0, g: 0, b: 0, alpha: 0 } })
    .resize(900, 900, { fit: "inside", withoutEnlargement: false })
    .extend({ top: 62, bottom: 62, left: 62, right: 62, background: { r: 0, g: 0, b: 0, alpha: 0 } })
    .resize(1024, 1024, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } })
    .png()
    .toFile(normalized);
  execFileSync(process.platform === "win32" ? "npx.cmd" : "npx", ["tauri", "icon", normalized, "--output", temporary], { cwd: desktop, stdio: "inherit" });
  for (const name of ["32x32.png", "64x64.png", "128x128.png", "128x128@2x.png", "icon.png", "icon.ico", "icon.icns"]) {
    copyFileSync(resolve(temporary, name), resolve(destination, name));
  }
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
