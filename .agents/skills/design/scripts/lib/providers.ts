/**
 * Shared provider plumbing for the design generators (logo, CIP, icon):
 * .env loading, public-HTTPS media URL validation, JSON requests with safe
 * redirects, image downloads and the Gemini REST generateContent call.
 * Bun built-ins only (fetch, Bun.file, Bun.write, Bun.sleep).
 */

import { isIP } from "node:net";
import { homedir } from "node:os";
import { extname, join } from "node:path";

export const HTTP_USER_AGENT = "ui-ux-pro-max/2.5 (design generation)";
export const GEMINI_API_BASE = "https://generativelanguage.googleapis.com/v1beta";

/** File extensions for the image types Gemini returns as inline data. */
export const IMAGE_EXTENSIONS: Record<string, string> = {
  "image/png": ".png",
  "image/jpeg": ".jpg",
  "image/webp": ".webp",
};

const SKILL_DIR = join(import.meta.dir, "..", "..");

/** Load .env files in priority order without overriding the process environment. */
export async function loadEnv(paths = [join(SKILL_DIR, ".env"), join(homedir(), ".claude", "skills", ".env"), join(homedir(), ".claude", ".env")]): Promise<void> {
  for (const envPath of paths) {
    const file = Bun.file(envPath);
    if (!(await file.exists())) continue;
    for (const raw of (await file.text()).split(/\r?\n/)) {
      const line = raw.trim();
      if (!line || line.startsWith("#") || !line.includes("=")) continue;
      const index = line.indexOf("=");
      const key = line.slice(0, index);
      if (!(key in process.env)) process.env[key] = line.slice(index + 1).replace(/^["']+|["']+$/g, "");
    }
  }
}

function ipv4ToInt(ip: string): number {
  return ip.split(".").reduce((acc, octet) => acc * 256 + Number(octet), 0);
}

const NON_GLOBAL_IPV4: Array<[string, number]> = [
  ["0.0.0.0", 8],
  ["10.0.0.0", 8],
  ["100.64.0.0", 10],
  ["127.0.0.0", 8],
  ["169.254.0.0", 16],
  ["172.16.0.0", 12],
  ["192.0.0.0", 24],
  ["192.0.2.0", 24],
  ["192.88.99.0", 24],
  ["192.168.0.0", 16],
  ["198.18.0.0", 15],
  ["198.51.100.0", 24],
  ["203.0.113.0", 24],
  ["240.0.0.0", 4],
  ["255.255.255.255", 32],
];

function isGlobalIpv4(ip: string): boolean {
  const value = ipv4ToInt(ip);
  return !NON_GLOBAL_IPV4.some(([base, bits]) => {
    const size = 2 ** (32 - bits);
    const start = ipv4ToInt(base);
    return value >= start && value < start + size;
  });
}

function expandIpv6(ip: string): number[] | null {
  let address = ip;
  const v4 = address.match(/(\d+\.\d+\.\d+\.\d+)$/);
  if (v4) {
    const n = ipv4ToInt(v4[1]!);
    address = address.slice(0, -v4[1]!.length) + `${(n >>> 16).toString(16)}:${(n & 0xffff).toString(16)}`;
  }
  const [head, tail] = address.split("::");
  const headParts = head ? head.split(":") : [];
  const tailParts = tail !== undefined && tail !== "" ? tail.split(":") : [];
  const missing = 8 - headParts.length - tailParts.length;
  if (address.includes("::") ? missing < 0 : missing !== 0) return null;
  const parts = [...headParts, ...Array(address.includes("::") ? missing : 0).fill("0"), ...tailParts];
  return parts.map((p) => parseInt(p, 16));
}

function isGlobalIpv6(ip: string): boolean {
  const words = expandIpv6(ip);
  if (!words) return false;
  const [w0, w1, , , , w5, w6, w7] = words as [number, number, number, number, number, number, number, number];
  if (words.every((w) => w === 0)) return false; // ::
  if (words.slice(0, 7).every((w) => w === 0) && w7 === 1) return false; // ::1
  if (words.slice(0, 5).every((w) => w === 0) && w5 === 0xffff) {
    return isGlobalIpv4(`${w6 >> 8}.${w6 & 0xff}.${w7 >> 8}.${w7 & 0xff}`); // IPv4-mapped
  }
  if ((w0 & 0xfe00) === 0xfc00) return false; // fc00::/7 unique local
  if ((w0 & 0xffc0) === 0xfe80) return false; // fe80::/10 link local
  if ((w0 & 0xff00) === 0xff00) return false; // multicast
  if (w0 === 0x2001 && w1 === 0x0db8) return false; // documentation
  if (w0 === 0x2001 && w1 < 0x0200) return false; // IETF protocol assignments
  if (w0 === 0x0064 && w1 === 0xff9b) return false; // NAT64
  if (w0 === 0x0100 && words.slice(1, 4).every((w) => w === 0)) return false; // discard-only
  return true;
}

/** Reject anything but public HTTPS URLs without credentials (SSRF guard for provider-returned media URLs). */
export function validatePublicHttpsUrl(url: string): void {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    throw new Error("Provider returned an invalid media URL");
  }
  if (parsed.protocol !== "https:" || !parsed.hostname || parsed.username || parsed.password) {
    throw new Error("Provider returned an invalid media URL");
  }
  const hostname = parsed.hostname.toLowerCase().replace(/^\[|\]$/g, "").replace(/\.$/, "");
  if (hostname === "localhost" || [".localhost", ".local", ".internal"].some((suffix) => hostname.endsWith(suffix))) {
    throw new Error("Provider media URL used a local hostname");
  }
  const family = isIP(hostname);
  if (family === 4 && !isGlobalIpv4(hostname)) throw new Error("Provider media URL used a non-public address");
  if (family === 6 && !isGlobalIpv6(hostname)) throw new Error("Provider media URL used a non-public address");
}

/** fetch that follows at most 5 redirects, validating each target like the Python safe redirect handler. */
export async function safeFetch(url: string, init: RequestInit & { timeoutMs?: number } = {}): Promise<Response> {
  const { timeoutMs = 60_000, ...rest } = init;
  let current = url;
  for (let hop = 0; hop <= 5; hop++) {
    const response = await runtime.fetch(current, { ...rest, redirect: "manual", signal: AbortSignal.timeout(timeoutMs) });
    if (response.status < 300 || response.status >= 400 || !response.headers.get("location")) return response;
    const next = new URL(response.headers.get("location")!, current).toString();
    validatePublicHttpsUrl(next);
    current = next;
  }
  throw new Error("Provider request failed: too many redirects");
}

export type ApiKeyHeader = "Authorization" | "x-api-key";

export interface JsonRequestOptions {
  method?: string;
  payload?: unknown;
  apiKeyHeader?: ApiKeyHeader;
}

export function authHeaders(apiKey: string, apiKeyHeader: string = "Authorization"): Record<string, string> {
  if (apiKeyHeader === "Authorization") return { Authorization: `Bearer ${apiKey}` };
  if (apiKeyHeader === "x-api-key") return { "x-api-key": apiKey };
  throw new Error("Unsupported API key header");
}

async function jsonRequest(url: string, apiKey: string, options: JsonRequestOptions = {}): Promise<any> {
  const { method = "GET", payload, apiKeyHeader = "Authorization" } = options;
  const headers: Record<string, string> = {
    ...authHeaders(apiKey, apiKeyHeader),
    Accept: "application/json",
    "User-Agent": HTTP_USER_AGENT,
  };
  const body = payload !== undefined ? JSON.stringify(payload) : undefined;
  if (body !== undefined) headers["Content-Type"] = "application/json";
  let response: Response;
  try {
    response = await safeFetch(url, { method, headers, body, timeoutMs: 60_000 });
  } catch (error) {
    throw new Error(`Provider request failed: ${(error as Error).message}`);
  }
  const text = await response.text();
  if (!response.ok) throw new Error(`Provider request failed (${response.status}): ${text.slice(0, 300)}`);
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`Provider request failed: ${(error as Error).message}`);
  }
}

/** Download a provider-hosted image without forwarding any API key. */
async function downloadImage(url: string, outputPath: string, providerName: string): Promise<void> {
  validatePublicHttpsUrl(url);
  let data: Uint8Array;
  try {
    const response = await safeFetch(url, {
      headers: { Accept: "image/*", "User-Agent": HTTP_USER_AGENT },
      timeoutMs: 120_000,
    });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const contentType = (response.headers.get("content-type") ?? "").split(";")[0]!.trim().toLowerCase();
    if (!contentType.startsWith("image/")) throw new Error(`${providerName} output is not an image (${contentType})`);
    data = new Uint8Array(await response.arrayBuffer());
  } catch (error) {
    const message = (error as Error).message;
    if (message.includes("is not an image")) throw error;
    throw new Error(`Unable to download ${providerName} image: ${message}`);
  }
  if (data.length === 0) throw new Error(`${providerName} returned an empty image`);
  await Bun.write(outputPath, data);
}

/** Gemini REST generateContent (the API the google-genai SDK wraps). */
async function geminiGenerateContent(model: string, apiKey: string, body: unknown): Promise<GeminiResponse> {
  const response = await runtime.fetch(`${GEMINI_API_BASE}/models/${encodeURIComponent(model)}:generateContent`, {
    method: "POST",
    headers: { "x-goog-api-key": apiKey, "Content-Type": "application/json", "User-Agent": HTTP_USER_AGENT },
    body: JSON.stringify(body),
    signal: AbortSignal.timeout(300_000),
  });
  const text = await response.text();
  if (!response.ok) throw new Error(`${response.status} ${text.slice(0, 300)}`);
  return JSON.parse(text) as GeminiResponse;
}

export interface GeminiPart {
  text?: string;
  inlineData?: { mimeType?: string; data?: string };
}

export interface GeminiResponse {
  candidates?: Array<{ finishReason?: string; content?: { parts?: GeminiPart[] } }>;
}

/** First inline image of a Gemini response, decoded. */
export function geminiInlineImage(response: GeminiResponse): { mimeType: string; data: Uint8Array } | null {
  for (const part of response.candidates?.[0]?.content?.parts ?? []) {
    const inline = part.inlineData;
    if (inline?.data && (inline.mimeType ?? "").startsWith("image/")) {
      return { mimeType: inline.mimeType!, data: Uint8Array.from(Buffer.from(inline.data, "base64")) };
    }
  }
  return null;
}

/** Concatenated text of the first candidate ('' when blocked or empty). */
export function geminiText(response: GeminiResponse): string {
  return (response.candidates?.[0]?.content?.parts ?? []).map((p) => p.text ?? "").join("");
}

/** Return outputPath with the extension of the returned image type (unknown types keep the path). */
export function pathForImageType(outputPath: string, mimeType: string): string {
  const extension = IMAGE_EXTENSIONS[mimeType.split(";")[0]!.trim().toLowerCase()];
  const suffix = extname(outputPath).toLowerCase();
  if (extension === undefined || suffix === extension || (suffix === ".jpeg" && extension === ".jpg")) return outputPath;
  return outputPath.slice(0, outputPath.length - extname(outputPath).length) + extension;
}

/** Python-style strftime("%Y%m%d_%H%M%S") for generated filenames. */
export function timestamp(date = new Date()): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}${p(date.getMonth() + 1)}${p(date.getDate())}_${p(date.getHours())}${p(date.getMinutes())}${p(date.getSeconds())}`;
}

/**
 * Indirection for network and timing so tests can stub them, like
 * unittest.mock.patch.object on the Python modules.
 */
export const runtime = {
  fetch: (input: string, init?: RequestInit): Promise<Response> => fetch(input, init),
  sleep: (ms: number): Promise<void> => Bun.sleep(ms),
  jsonRequest,
  downloadImage,
  geminiGenerateContent,
};
