import { defineConfig } from "vite";
import basicSsl from "@vitejs/plugin-basic-ssl";

// GitHub Pages serves the demo at
// https://larsbrubaker.github.io/AstroRock/
// so all asset paths must be prefixed accordingly. `./` works both there
// and locally under `vite dev`.
export default defineConfig(({ command }) => ({
  base: "./",
  // Stamped into the bundle and appended to the wasm-pack asset URLs in
  // main.ts. The pkg/ files are served with stable (unhashed) names, so
  // without this browsers keep serving a stale cached wasm long after a
  // deploy.
  define: {
    __BUILD_ID__: JSON.stringify(Date.now().toString(36)),
  },
  // basic-ssl gives the dev server a self-signed cert so a phone on the
  // same Wi-Fi gets a secure context; `host: true` binds 0.0.0.0 so the
  // phone can reach it via the printed Network URL. SSL is opt-in
  // (`VITE_SSL=1 bun run dev`) because local tooling browsers reject
  // self-signed certs.
  plugins: command === "serve" && process.env.VITE_SSL ? [basicSsl()] : [],
  server: { host: true },
  preview: { host: true },
}));
