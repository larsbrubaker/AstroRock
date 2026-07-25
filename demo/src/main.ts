// Browser bootstrap for AstroRock's single Rust/agg-gui app.
//
// The Rust wasm module (demo-wgpu's `web_shell`) owns everything
// platform-generic the moment it loads: canvas sizing, the frame loop,
// pointer / wheel / keyboard input, and DPR tracking. This file only
// loads the wasm module itself and reports boot failures.
//
// Must NOT render visible UI beyond the canvas — every button, label,
// menu, and readout is painted by agg-gui inside the canvas. See
// CLAUDE.md.

// Build stamp injected by vite (see vite.config.ts `define`) — appended
// to the pkg/ asset URLs so a new deploy always busts the browser cache.
declare const __BUILD_ID__: string;

// wasm-pack --no-typescript does not emit .d.ts files; we reference the
// generated module structurally instead.
type WasmModule = {
  default: (url?: string | URL | { module_or_path: string | URL }) => Promise<unknown>;
};

const canvas = document.getElementById("astrorock-canvas") as HTMLCanvasElement;

function showBootError(err: unknown): void {
  console.error("astrorock: failed to boot wasm app", err);
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    return;
  }
  canvas.width = Math.max(1, canvas.clientWidth || window.innerWidth);
  canvas.height = Math.max(1, canvas.clientHeight || window.innerHeight);
  ctx.fillStyle = "#04040c";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = "#f2f2f7";
  ctx.font = "20px sans-serif";
  ctx.fillText("AstroRock failed to load.", 24, 48);
  ctx.font = "14px sans-serif";
  ctx.fillText("Check that wasm-pack output exists at demo/public/pkg.", 24, 78);
  ctx.fillText(String((err as Error)?.message ?? err ?? ""), 24, 102);
}

async function boot(): Promise<void> {
  // Resolve against `document.baseURI` so the URL is correct regardless
  // of where Vite places the bundle (under `/assets/` after build) and
  // under the `/AstroRock/` Pages sub-path.
  const v = `?v=${__BUILD_ID__}`;
  const url = new URL(`pkg/astrorock_wasm.js${v}`, document.baseURI).href;
  const mod = (await import(/* @vite-ignore */ url)) as WasmModule;
  const wasmUrl = new URL(`pkg/astrorock_wasm_bg.wasm${v}`, document.baseURI).href;
  // Module init runs the Rust `#[wasm_bindgen(start)]`, which boots the
  // whole shell (input, frame loop, rendering).
  await mod.default({ module_or_path: wasmUrl });
}

void boot().catch(showBootError);
