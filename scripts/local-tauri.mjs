// The dev server and local installer share the verified desktop native dependencies.
import { execFileSync, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { join } from "node:path";

const action = process.argv[2];
if (action !== "dev" && action !== "build") throw new Error("Expected dev or build");
const root = fileURLToPath(new URL("../", import.meta.url));
const windows = process.platform === "win32";
const env = { ...process.env };
const extraArgs = process.argv.slice(3);
if (windows || process.platform === "darwin") {
  env.SHERPA_ONNX_LIB_DIR = execFileSync(windows ? "python" : "python3", [
    join(root, "scripts/fetch-sherpa.py"), "--local-libs", join(root, "target/verified-sherpa"),
  ], { encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] }).trim();
}

const desktop = join(root, "apps/desktop");
const cli = join(desktop, "node_modules/@tauri-apps/cli/tauri.js");
const args = action === "dev" ? ["dev", "--config", "src-tauri/tauri.dev.conf.json"] : ["build", "--bundles", "app"];
if (windows) env.THINGMAKER_TAURI_EXTRA_ARGS = JSON.stringify(extraArgs);
const result = windows
  ? spawnSync("powershell.exe", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", join(root, "scripts/windows-build-env.ps1"), "-TauriAction", action], { cwd: desktop, env, stdio: "inherit" })
  : spawnSync(process.execPath, [cli, ...args, ...extraArgs], { cwd: desktop, env, stdio: "inherit" });
if (result.error) throw result.error;
process.exit(result.status ?? 1);
