// Compila o `psm` (servidor MCP + runtime) em release e o coloca onde o Tauri espera um
// executável externo: src-tauri/binaries/psm-<triplo>.exe. O instalador o copia para junto do app.
import { execSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..", "..");
const triple = execSync("rustc -vV").toString().match(/host: (\S+)/)[1];
const ext = process.platform === "win32" ? ".exe" : "";

execSync("cargo build --release -p sprite-cli", { cwd: root, stdio: "inherit" });
const out = join(here, "..", "src-tauri", "binaries");
mkdirSync(out, { recursive: true });
copyFileSync(join(root, "target", "release", `psm${ext}`), join(out, `psm-${triple}${ext}`));
console.log(`psm pronto em src-tauri/binaries/psm-${triple}${ext}`);
