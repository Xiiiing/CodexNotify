import sharp from "sharp";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const desktop = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const source = resolve(desktop, "src-tauri/icons/app-icon.svg");
const destination = resolve(desktop, "src-tauri/icons/icon.png");
await sharp(source).resize(512, 512).png().toFile(destination);
console.log(`Rendered ${destination}`);
