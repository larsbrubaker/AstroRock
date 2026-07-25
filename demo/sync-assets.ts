// Copy runtime-fetched assets (music) from the repo's canonical
// assets/ tree into Vite's public dir before dev/build. The canonical
// copies live outside demo/ because the native shell reads them too;
// demo/public/music is generated and gitignored.
import { cpSync, mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const dest = join(here, "public", "music");
mkdirSync(dest, { recursive: true });
cpSync(join(here, "..", "assets", "music"), dest, { recursive: true });
console.log("synced assets/music -> demo/public/music");
