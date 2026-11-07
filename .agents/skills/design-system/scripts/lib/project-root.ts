/**
 * The design skills can be installed outside the project they operate on
 * (user-level skills, plugin cache), so the project root is resolved from the
 * working directory: the nearest ancestor holding assets/design-tokens.json,
 * assets/design-tokens.css, package.json or .git. DESIGN_SYSTEM_PROJECT_ROOT
 * overrides it explicitly.
 */

import { stat } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";

const MARKERS = [join("assets", "design-tokens.json"), join("assets", "design-tokens.css"), "package.json", ".git"];

async function exists(path: string): Promise<boolean> {
  return stat(path).then(
    () => true,
    () => false,
  );
}

export async function findProjectRoot(): Promise<string> {
  const override = process.env.DESIGN_SYSTEM_PROJECT_ROOT;
  if (override) return resolve(override);
  const start = resolve(process.cwd());
  for (let candidate = start; ; ) {
    for (const marker of MARKERS) if (await exists(join(candidate, marker))) return candidate;
    const parent = dirname(candidate);
    if (parent === candidate) return start;
    candidate = parent;
  }
}

/** Python urllib.parse.quote(text) equivalent (safe="/"). */
export function pyQuote(text: string): string {
  return encodeURIComponent(text)
    .replace(/[!'()*]/g, (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`)
    .replace(/%2F/g, "/");
}
