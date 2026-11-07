/** Closed, non-executable grammar for design-system decision rules. */

export const CONDITION_SIGNALS: Readonly<Record<string, readonly string[]>> = {
  if_booking: ["booking", "appointment", "calendar"],
  if_boutique: ["boutique"],
  if_casual: ["casual", "playful"],
  if_checkout: ["checkout", "payment", "purchase"],
  if_children: ["child", "children", "kids"],
  if_collaboration: ["collaboration", "multiplayer", "co-edit"],
  if_competitive: ["competitive", "leaderboard"],
  if_content_focused: ["content", "article", "reading", "documentation"],
  if_conversion_focused: ["conversion", "sales", "signup", "purchase"],
  if_creative_field: ["creative", "artist", "portfolio"],
  if_crop_focused: ["crop", "farm", "agriculture"],
  if_dashboard: ["dashboard", "operations", "monitoring"],
  if_data_heavy: ["data heavy", "data-heavy", "analytics", "large dataset"],
  if_delivery: ["delivery", "courier", "shipping"],
  if_discovery_focused: ["discover", "discovery", "browse", "directory"],
  if_engagement_metric: ["engagement", "retention", "contribution"],
  if_experience_focused: ["experience", "immersive", "journey"],
  if_gamification: ["gamification", "badges", "streak"],
  if_health: ["health", "medical", "patient"],
  if_hero_needed: ["hero", "showcase", "launch"],
  if_large_dataset: ["large dataset", "thousands", "millions"],
  if_light_mode_needed: ["light mode", "light theme"],
  if_low_performance: ["low performance", "low-end", "slow device"],
  if_luxury: ["luxury", "premium", "high-end"],
  if_medication: ["medication", "medicine", "prescription"],
  if_meditation: ["meditation", "breathing", "mindfulness"],
  if_minimal_portfolio: ["minimal portfolio", "simple portfolio"],
  if_mobile: ["mobile", "phone", "tablet", "ios", "android"],
  if_personalized: ["personalized", "personalised", "recommendation"],
  if_pre_launch: ["pre-launch", "prelaunch", "coming soon", "waitlist"],
  if_salary_focused: ["salary", "compensation", "pay range"],
  if_team_collaboration: ["team collaboration", "team workspace"],
  if_trust_needed: ["trust", "secure", "verified", "authority"],
  if_ux_focused: ["ux", "usability", "accessibility", "accessible"],
  if_video_ready: ["video ready", "product video", "demo video"],
};

export const ALLOWED_CONDITIONS: ReadonlySet<string> = new Set(["must_have", ...Object.keys(CONDITION_SIGNALS)]);
export const ACTION_PREFIXES: ReadonlySet<string> = new Set(["constraint", "style", "pattern", "mode"]);
const TOKEN_ACTION_PREFIXES = new Set(["constraint", "style"]);
const TOKEN_RE = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const WORD = "[\\p{L}\\p{N}_]";

export function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\/]/g, "\\$&");
}

/** `(?<!\w)phrase(?!\w)` with Python's Unicode \w. */
export function phrasePattern(phrase: string, flags = "u"): RegExp {
  return new RegExp(`(?<!${WORD})${escapeRegExp(phrase)}(?!${WORD})`, flags);
}

const CONDITION_PATTERNS = new Map<string, RegExp[]>(
  Object.entries(CONDITION_SIGNALS).map(([condition, signals]) => [
    condition,
    signals.map((signal) => phrasePattern(signal)),
  ]),
);

export type DecisionRules = Map<string, string[]>;

export interface AppliedRules {
  activated: { condition: string; actions: string[] }[];
  style_ids: string[];
  constraints: string[];
  pattern: string | null;
  mode: string | null;
}

/* ---------- strict JSON (duplicate keys are an error) ---------- */

class StrictJsonParser {
  private index = 0;
  constructor(private readonly text: string) {}

  parse(): unknown {
    this.ws();
    const value = this.value();
    this.ws();
    if (this.index !== this.text.length) this.fail("Extra data");
    return value;
  }

  private fail(message: string): never {
    throw new SyntaxError(`${message}: char ${this.index}`);
  }

  private ws() {
    while (/[ \t\n\r]/.test(this.text[this.index] ?? "")) this.index++;
  }

  private value(): unknown {
    const ch = this.text[this.index];
    if (ch === "{") return this.object();
    if (ch === "[") return this.array();
    if (ch === '"') return this.string();
    if (ch === "t" && this.text.startsWith("true", this.index)) {
      this.index += 4;
      return true;
    }
    if (ch === "f" && this.text.startsWith("false", this.index)) {
      this.index += 5;
      return false;
    }
    if (ch === "n" && this.text.startsWith("null", this.index)) {
      this.index += 4;
      return null;
    }
    const match = /^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/.exec(this.text.slice(this.index));
    if (match) {
      this.index += match[0].length;
      return Number(match[0]);
    }
    this.fail("Expecting value");
  }

  private object(): Map<string, unknown> {
    const result = new Map<string, unknown>();
    this.index++;
    this.ws();
    if (this.text[this.index] === "}") {
      this.index++;
      return result;
    }
    for (;;) {
      this.ws();
      if (this.text[this.index] !== '"') this.fail("Expecting property name enclosed in double quotes");
      const key = this.string();
      this.ws();
      if (this.text[this.index] !== ":") this.fail("Expecting ':' delimiter");
      this.index++;
      this.ws();
      const value = this.value();
      if (result.has(key)) throw new DuplicateKeyError(key);
      result.set(key, value);
      this.ws();
      const next = this.text[this.index];
      if (next === ",") {
        this.index++;
        continue;
      }
      if (next === "}") {
        this.index++;
        return result;
      }
      this.fail("Expecting ',' delimiter");
    }
  }

  private array(): unknown[] {
    const result: unknown[] = [];
    this.index++;
    this.ws();
    if (this.text[this.index] === "]") {
      this.index++;
      return result;
    }
    for (;;) {
      this.ws();
      result.push(this.value());
      this.ws();
      const next = this.text[this.index];
      if (next === ",") {
        this.index++;
        continue;
      }
      if (next === "]") {
        this.index++;
        return result;
      }
      this.fail("Expecting ',' delimiter");
    }
  }

  private string(): string {
    const start = this.index;
    this.index++;
    let out = "";
    for (;;) {
      const ch = this.text[this.index];
      if (ch === undefined) {
        this.index = start;
        this.fail("Unterminated string starting at");
      }
      if (ch === '"') {
        this.index++;
        return out;
      }
      if (ch === "\\") {
        const escape = this.text[this.index + 1];
        const simple: Record<string, string> = { '"': '"', "\\": "\\", "/": "/", b: "\b", f: "\f", n: "\n", r: "\r", t: "\t" };
        if (escape !== undefined && escape in simple) {
          out += simple[escape];
          this.index += 2;
          continue;
        }
        if (escape === "u") {
          const hex = this.text.slice(this.index + 2, this.index + 6);
          if (!/^[0-9a-fA-F]{4}$/.test(hex)) this.fail("Invalid \\uXXXX escape");
          out += String.fromCharCode(parseInt(hex, 16));
          this.index += 6;
          continue;
        }
        this.fail("Invalid \\escape");
      }
      if (ch < " ") this.fail("Invalid control character at");
      out += ch;
      this.index++;
    }
  }
}

class DuplicateKeyError extends Error {
  constructor(readonly key: string) {
    super(`duplicate decision-rule key: ${key}`);
  }
}

/** Parse the canonical condition -> action-array representation. */
export function parseDecisionRules(raw: string | null | undefined): DecisionRules {
  let rules: unknown;
  try {
    rules = new StrictJsonParser(raw || "{}").parse();
  } catch (error) {
    if (error instanceof DuplicateKeyError) throw new Error(error.message);
    throw new Error(`invalid decision-rule JSON: ${(error as Error).message}`);
  }
  if (!(rules instanceof Map)) throw new Error("decision rules must be a JSON object");
  const parsed: DecisionRules = new Map();
  for (const [condition, actions] of rules) {
    if (!ALLOWED_CONDITIONS.has(condition)) throw new Error(`unknown decision-rule condition: ${condition}`);
    if (!Array.isArray(actions) || actions.length === 0) {
      throw new Error(`${condition} must map to a non-empty action array`);
    }
    for (const action of actions) validateAction(action);
    if (new Set(actions).size !== actions.length) throw new Error(`${condition} contains duplicate actions`);
    parsed.set(condition, actions as string[]);
  }
  return parsed;
}

function validateAction(action: unknown): asserts action is string {
  if (typeof action !== "string" || !action.includes(":")) {
    throw new Error(`action must use a known prefix: ${String(action)}`);
  }
  const separator = action.indexOf(":");
  const prefix = action.slice(0, separator);
  const value = action.slice(separator + 1);
  if (!ACTION_PREFIXES.has(prefix)) throw new Error(`unknown decision-rule action: ${action}`);
  if (TOKEN_ACTION_PREFIXES.has(prefix) && !TOKEN_RE.test(value)) {
    throw new Error(`invalid ${prefix} action value: ${value}`);
  }
  if (prefix === "pattern" && !value.trim()) throw new Error("pattern action must name a pattern");
  if (prefix === "mode" && value !== "dark" && value !== "light") throw new Error("mode action must be dark or light");
}

/** Return deterministic mutations and an audit trail; never execute data. */
export function applyDecisionRules(rules: DecisionRules, query: string | null | undefined): AppliedRules {
  const normalized = String(query ?? "").toLowerCase();
  const result: AppliedRules = { activated: [], style_ids: [], constraints: [], pattern: null, mode: null };
  for (const [condition, actions] of rules) {
    const active =
      condition === "must_have" || (CONDITION_PATTERNS.get(condition) ?? []).some((pattern) => pattern.test(normalized));
    if (!active) continue;
    result.activated.push({ condition, actions: [...actions] });
    for (const action of actions) {
      const separator = action.indexOf(":");
      const prefix = action.slice(0, separator);
      const value = action.slice(separator + 1);
      if (prefix === "style" && !result.style_ids.includes(value)) result.style_ids.push(value);
      else if (prefix === "constraint" && !result.constraints.includes(value)) result.constraints.push(value);
      else if (prefix === "pattern") result.pattern = value;
      else if (prefix === "mode") result.mode = value;
    }
  }
  return result;
}

/** JSON-friendly view of parsed rules (Maps serialize as {}). */
export function decisionRulesToObject(rules: DecisionRules): Record<string, string[]> {
  return Object.fromEntries(rules);
}
