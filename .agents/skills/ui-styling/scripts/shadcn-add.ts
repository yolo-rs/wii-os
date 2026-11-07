#!/usr/bin/env bun
/**
 * shadcn/ui Component Installer
 *
 * Adds shadcn/ui components with the shadcn CLI run by Bun
 * (`bunx --bun shadcn@<version> add ...`), refusing to silently replace
 * components that already exist.
 *
 * Examples:
 *   bun scripts/shadcn-add.ts button
 *   bun scripts/shadcn-add.ts button card dialog
 *   bun scripts/shadcn-add.ts --all
 *   bun scripts/shadcn-add.ts button --overwrite
 *   bun scripts/shadcn-add.ts button card --dry-run
 *   bun scripts/shadcn-add.ts --list
 */

import { $ } from "bun";
import { basename, join, resolve } from "node:path";
import { parseArgs } from "node:util";

export interface RunResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

/** Runs a command in cwd; replaceable in tests (like patching subprocess.run). */
export type Runner = (cmd: string[], cwd: string) => Promise<RunResult>;

export const defaultRunner: Runner = async (cmd, cwd) => {
  const result = await $`${cmd}`.cwd(cwd).nothrow().quiet();
  return { exitCode: result.exitCode, stdout: result.stdout.toString(), stderr: result.stderr.toString() };
};

export type Outcome = [success: boolean, message: string];

export class ShadcnInstaller {
  readonly projectRoot: string;
  readonly componentsJson: string;

  constructor(
    projectRoot?: string | null,
    readonly dryRun = false,
    readonly runner: Runner = defaultRunner,
  ) {
    this.projectRoot = resolve(projectRoot ?? process.cwd());
    this.componentsJson = join(this.projectRoot, "components.json");
  }

  /** True when components.json exists (shadcn initialized). */
  async checkShadcnConfig(): Promise<boolean> {
    return Bun.file(this.componentsJson).exists();
  }

  /** Names of components already present in <components alias>/ui/*.tsx. */
  async getInstalledComponents(): Promise<string[]> {
    if (!(await this.checkShadcnConfig())) return [];
    try {
      const config = (await Bun.file(this.componentsJson).json()) as { aliases?: { components?: string } };
      const componentsDir = join(this.projectRoot, (config.aliases?.components ?? "components").replace("@/", ""));
      return [...new Bun.Glob("*.tsx").scanSync({ cwd: join(componentsDir, "ui"), onlyFiles: true })].map((f) => basename(f, ".tsx"));
    } catch {
      return [];
    }
  }

  /** shadcn version pinned by the project's package.json, else "latest". */
  async getShadcnVersion(): Promise<string> {
    try {
      const pkg = (await Bun.file(join(this.projectRoot, "package.json")).json()) as Record<string, Record<string, string> | undefined>;
      for (const section of ["dependencies", "devDependencies"]) {
        const version = pkg[section]?.shadcn;
        if (version) return version.replace(/^[\^~>=<]+/, "").split(/\s+/)[0]!;
      }
    } catch {
      // missing or invalid package.json
    }
    return "latest";
  }

  /** The command as documented (`bunx --bun shadcn@<version> ...`). */
  private async command(args: string[]): Promise<string[]> {
    return ["bunx", "--bun", `shadcn@${await this.getShadcnVersion()}`, ...args];
  }

  private async execute(cmd: string[], success: string, failure: string): Promise<Outcome> {
    if (this.dryRun) return [true, `Would run: ${cmd.join(" ")}`];
    // `bunx` is `bun x`: run it through the current Bun executable so it is always found.
    const exec = [process.execPath, "x", ...cmd.slice(1)];
    let result: RunResult;
    try {
      result = await this.runner(exec, this.projectRoot);
    } catch (error) {
      return [false, `bun not found or failed to start (${(error as Error).message}). Install Bun from https://bun.sh`];
    }
    if (result.exitCode !== 0) return [false, `${failure}: ${result.stderr || result.stdout || `exit code ${result.exitCode}`}`];
    return [true, result.stdout ? `${success}\n\nOutput:\n${result.stdout}` : success];
  }

  async addComponents(components: string[], overwrite = false): Promise<Outcome> {
    if (!components.length) return [false, "No components specified"];
    if (!(await this.checkShadcnConfig())) return [false, "shadcn not initialized. Run 'bunx --bun shadcn@latest init' first"];

    const installed = await this.getInstalledComponents();
    const already = components.filter((c) => installed.includes(c));
    if (already.length && !overwrite) {
      return [false, `Components already installed: ${already.join(", ")}. Use --overwrite to reinstall`];
    }
    const cmd = await this.command(["add", ...components, ...(overwrite ? ["--overwrite"] : [])]);
    return this.execute(cmd, `Successfully added components: ${components.join(", ")}`, "Failed to add components");
  }

  async addAllComponents(overwrite = false): Promise<Outcome> {
    if (!(await this.checkShadcnConfig())) return [false, "shadcn not initialized. Run 'bunx --bun shadcn@latest init' first"];
    const cmd = await this.command(["add", "--all", ...(overwrite ? ["--overwrite"] : [])]);
    return this.execute(cmd, "Successfully added all components", "Failed to add all components");
  }

  async listInstalled(): Promise<Outcome> {
    if (!(await this.checkShadcnConfig())) return [false, "shadcn not initialized"];
    const installed = await this.getInstalledComponents();
    if (!installed.length) return [true, "No components installed"];
    return [true, "Installed components:\n" + installed.sort().map((c) => `  - ${c}`).join("\n")];
  }
}

const HELP = `usage: shadcn-add.ts [-h] [--all] [--overwrite] [--dry-run] [--list] [--project-root PROJECT_ROOT] [components ...]

Add shadcn/ui components to your project (runs bunx --bun shadcn@<version> add)`;

/** CLI; returns the process exit status. */
export async function main(argv = process.argv.slice(2), runner: Runner = defaultRunner): Promise<number> {
  const { values, positionals } = parseArgs({
    args: argv,
    allowPositionals: true,
    options: {
      all: { type: "boolean" },
      overwrite: { type: "boolean" },
      "dry-run": { type: "boolean" },
      list: { type: "boolean" },
      "project-root": { type: "string" },
      help: { type: "boolean", short: "h" },
    },
  });
  if (values.help) {
    console.log(HELP);
    return 0;
  }
  const installer = new ShadcnInstaller(values["project-root"], Boolean(values["dry-run"]), runner);
  let outcome: Outcome;
  if (values.list) outcome = await installer.listInstalled();
  else if (values.all) outcome = await installer.addAllComponents(Boolean(values.overwrite));
  else if (!positionals.length) {
    console.log(HELP);
    return 1;
  } else outcome = await installer.addComponents(positionals, Boolean(values.overwrite));
  console.log(outcome[1]);
  return outcome[0] ? 0 : 1;
}

if (import.meta.main) process.exit(await main());
