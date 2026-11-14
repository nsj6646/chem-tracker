import { describe, expect, test } from "bun:test";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dir, "..");

function collectFiles(dir: string): string[] {
  const results: string[] = [];
  for (const entry of readdirSync(dir)) {
    const fullPath = join(dir, entry);
    if (statSync(fullPath).isDirectory()) {
      results.push(...collectFiles(fullPath));
    } else if (fullPath.endsWith(".ts") || fullPath.endsWith(".tsx")) {
      results.push(fullPath);
    }
  }
  return results;
}

describe("Repository Guardrails", () => {
  test("every frontend invoke() command matches Tauri generate_handler! registration", () => {
    const srcFiles = collectFiles(join(ROOT, "src"));
    const frontendCmds = new Set<string>();
    const invokeRe = /\binvoke(?:<[^>]+>)?\(\s*"([^"]+)"/g;

    for (const file of srcFiles) {
      const content = readFileSync(file, "utf8");
      for (const match of content.matchAll(invokeRe)) {
        const cmd = match[1];
        if (!cmd.startsWith("plugin:")) {
          frontendCmds.add(cmd);
        }
      }
    }

    const libRs = readFileSync(join(ROOT, "src-tauri/src/lib.rs"), "utf8");
    const handlerBlock = libRs.match(/generate_handler!\[([\s\S]*?)\]/);
    expect(handlerBlock).not.toBeNull();

    const rustCmds = new Set(
      [...handlerBlock![1].matchAll(/commands::([a-z0-9_]+)/g)].map(
        (m) => m[1],
      ),
    );

    expect([...frontendCmds].sort()).toEqual([...rustCmds].sort());
  });

  test("no confidential spreadsheets, databases, or private keys are tracked in git", () => {
    const proc = Bun.spawnSync(["git", "ls-files"], { cwd: ROOT });
    expect(proc.exitCode).toBe(0);

    const tracked = proc.stdout
      .toString("utf8")
      .split(/\r?\n/)
      .filter(Boolean);
    const forbiddenExt = /\.(xlsx|xls|db|sqlite|mdb|accdb|pem|key|p12|pfx)$/i;
    const forbiddenDotEnv = /(^|\/)\.env(\..+)?$/i;

    const violations = tracked.filter(
      (path) => forbiddenExt.test(path) || forbiddenDotEnv.test(path),
    );
    expect(violations).toEqual([]);
  });
});
