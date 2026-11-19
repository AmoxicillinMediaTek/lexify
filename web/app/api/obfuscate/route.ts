import { spawn } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

export const runtime = "nodejs";

const maxSourceBytes = 256 * 1024;
function runObfuscator(inputPath: string, args: string[], cwd: string): Promise<string> {
  return new Promise((resolve, reject) => {
    const child = spawn("cargo", ["run", "--quiet", "--", inputPath, ...args], {
      cwd,
      stdio: ["ignore", "pipe", "pipe"],
      timeout: 60_000,
      windowsHide: true,
    });
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8").on("data", (chunk: string) => {
      stdout += chunk;
    });
    child.stderr.setEncoding("utf8").on("data", (chunk: string) => {
      stderr += chunk;
    });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0) resolve(stdout);
      else reject(new Error(stderr.trim() || `Obfuscator exited with status ${code ?? "unknown"}.`));
    });
  });
}

export async function POST(request: Request) {
  let tempDir: string | undefined;
  try {
    const payload: unknown = await request.json();
    if (!payload || typeof payload !== "object") {
      return Response.json({ error: "Send Lua source and options." }, { status: 400 });
    }
    const body = payload as Record<string, unknown>;
    if (typeof body.source !== "string" || !body.source.trim()) {
      return Response.json({ error: "Enter some Lua source first." }, { status: 400 });
    }
    if (Buffer.byteLength(body.source, "utf8") > maxSourceBytes) {
      return Response.json({ error: "Source is larger than the 256 KB demo limit." }, { status: 413 });
    }

    // Presets are expanded in the UI into explicit pass toggles. Run the CLI at
    // Basic so turning an individual pass off remains respected by the request.
    const seed = typeof body.seed === "string" && /^\d{1,16}$/.test(body.seed) ? body.seed : "1";
    const args = ["--level", "basic", "--seed", seed];
    const boolFlags: Array<[string, string]> = [
      ["minify", "--minify"], ["rename", "--rename"], ["encryptStrings", "--encrypt-strings"],
      ["obfuscateNumbers", "--obfuscate-numbers"], ["obfuscateBooleans", "--obfuscate-booleans"],
      ["obfuscateTables", "--obfuscate-tables"], ["controlFlow", "--control-flow"],
      ["deadCode", "--dead-code"], ["splitStrings", "--split-strings"],
      ["lookupTables", "--lookup-tables"], ["randomizeLookup", "--randomize-lookup"],
      ["sharedLookup", "--shared-lookup"],
    ];
    for (const [key, flag] of boolFlags) if (body[key] === true) args.push(flag);
    if (body.splitStrings === true && Number.isInteger(body.splitSize)) {
      const size = Number(body.splitSize);
      if (size >= 1 && size <= 64) args.push("--split-size", String(size));
    }

    const repoRoot = path.resolve(process.cwd(), "..");
    tempDir = await mkdtemp(path.join(os.tmpdir(), "lexify-web-"));
    const inputPath = path.join(tempDir, "input.lua");
    await writeFile(inputPath, body.source, { encoding: "utf8", mode: 0o600 });
    const output = await runObfuscator(inputPath, args, repoRoot);
    return Response.json({ output });
  } catch (error) {
    const message = error instanceof Error ? error.message : "Obfuscation failed.";
    const safeMessage = message.slice(0, 1500);
    return Response.json({ error: safeMessage }, { status: 422 });
  } finally {
    if (tempDir) await rm(tempDir, { recursive: true, force: true });
  }
}
