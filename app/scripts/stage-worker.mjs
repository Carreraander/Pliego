// Compila `pliego-worker` y lo deja en `src-tauri/binaries/pliego-worker-<triple>[.exe]`, donde Tauri lo
// busca como sidecar (`bundle.externalBin`). Tauri lo instala junto a `pliego-app.exe` con el nombre
// `pliego-worker.exe`, que es donde `WorkerConfig::discover` lo espera.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const run = (args) => execFileSync("cargo", args, { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] });

const triple = /^host: (.+)$/m.exec(execFileSync("rustc", ["-vV"], { encoding: "utf8" }))?.[1];
if (!triple) throw new Error("no se pudo averiguar el triple de destino de rustc");
const ext = process.platform === "win32" ? ".exe" : "";

execFileSync("cargo", ["build", "--release", "-p", "pliego-worker"], { cwd: root, stdio: "inherit" });
// Respeta CARGO_TARGET_DIR (en acr12 apunta a D:\dev-cache\target\<clon>).
const targetDir = JSON.parse(run(["metadata", "--format-version", "1", "--no-deps"])).target_directory;

const dest = join(root, "app", "src-tauri", "binaries");
mkdirSync(dest, { recursive: true });
const out = join(dest, `pliego-worker-${triple}${ext}`);
copyFileSync(join(targetDir, "release", `pliego-worker${ext}`), out);
console.log(`worker preparado: ${out}`);
