/**
 * Shared local search engine for the design sub-skills (slides, logo, CIP):
 * an RFC 4180 CSV reader with csv.DictReader semantics and a BM25 ranker.
 * Bun built-ins only.
 */

export type Row = Record<string, string>;

/** Parse RFC 4180 CSV text into records (quoted fields, embedded commas/newlines, "" escapes). */
export function parseCsv(text: string): string[][] {
  const records: string[][] = [];
  let record: string[] = [];
  let field = "";
  let inQuotes = false;
  let i = text.charCodeAt(0) === 0xfeff ? 1 : 0;
  const pushField = () => {
    record.push(field);
    field = "";
  };
  const pushRecord = () => {
    pushField();
    records.push(record);
    record = [];
  };
  for (; i < text.length; i++) {
    const ch = text[i]!;
    if (inQuotes) {
      if (ch === '"') {
        if (text[i + 1] === '"') {
          field += '"';
          i++;
        } else {
          inQuotes = false;
        }
      } else {
        field += ch;
      }
    } else if (ch === '"') {
      inQuotes = true;
    } else if (ch === ",") {
      pushField();
    } else if (ch === "\n" || ch === "\r") {
      if (ch === "\r" && text[i + 1] === "\n") i++;
      pushRecord();
    } else {
      field += ch;
    }
  }
  if (field !== "" || record.length > 0) pushRecord();
  // csv.DictReader skips blank lines.
  return records.filter((r) => !(r.length === 1 && r[0] === ""));
}

/** csv.DictReader equivalent: first record is the header; short rows get "" values. */
export function csvToRows(text: string): Row[] {
  const [header, ...records] = parseCsv(text);
  if (!header) return [];
  return records.map((values) => {
    const row: Row = {};
    header.forEach((name, index) => {
      row[name] = values[index] ?? "";
    });
    return row;
  });
}

export async function loadCsv(path: string): Promise<Row[]> {
  return csvToRows(await Bun.file(path).text());
}

/** BM25 ranking (k1=1.5, b=0.75); tokens are lowercase words longer than 2 characters. */
export class BM25 {
  corpus: string[][] = [];
  docLengths: number[] = [];
  avgdl = 0;
  idf = new Map<string, number>();
  docFreqs = new Map<string, number>();
  N = 0;
  private termFreqs: Array<Map<string, number>> = [];

  constructor(
    readonly k1 = 1.5,
    readonly b = 0.75,
  ) {}

  tokenize(text: unknown): string[] {
    return String(text)
      .toLowerCase()
      .replace(/[^\p{L}\p{N}\p{M}_\s]/gu, " ")
      .split(/\s+/)
      .filter((w) => w.length > 2);
  }

  fit(documents: string[]): void {
    this.corpus = documents.map((doc) => this.tokenize(doc));
    this.N = this.corpus.length;
    if (this.N === 0) return;
    this.docLengths = this.corpus.map((doc) => doc.length);
    this.avgdl = this.docLengths.reduce((a, b) => a + b, 0) / this.N;
    this.termFreqs = this.corpus.map((doc) => {
      const tf = new Map<string, number>();
      for (const word of doc) tf.set(word, (tf.get(word) ?? 0) + 1);
      for (const word of tf.keys()) this.docFreqs.set(word, (this.docFreqs.get(word) ?? 0) + 1);
      return tf;
    });
    for (const [word, freq] of this.docFreqs) {
      this.idf.set(word, Math.log((this.N - freq + 0.5) / (freq + 0.5) + 1));
    }
  }

  /** Scores for every document, highest first (stable for ties, like Python's sorted). */
  score(query: string): Array<[number, number]> {
    const queryTokens = this.tokenize(query);
    const scores: Array<[number, number]> = this.termFreqs.map((tf, idx) => {
      let score = 0;
      const docLen = this.docLengths[idx]!;
      for (const token of queryTokens) {
        const idf = this.idf.get(token);
        if (idf === undefined) continue;
        const freq = tf.get(token) ?? 0;
        score += (idf * (freq * (this.k1 + 1))) / (freq + this.k1 * (1 - this.b + (this.b * docLen) / this.avgdl));
      }
      return [idx, score];
    });
    return scores.sort((a, b) => b[1] - a[1]);
  }
}

export interface DomainConfig {
  file: string;
  search_cols: string[];
  output_cols: string[];
}

/** BM25 search over one CSV; returns projected rows with a positive score. */
export async function searchCsv(
  filepath: string,
  searchCols: string[],
  outputCols: string[],
  query: string,
  maxResults: number,
): Promise<Row[]> {
  if (!(await Bun.file(filepath).exists())) return [];
  const data = await loadCsv(filepath);
  const bm25 = new BM25();
  bm25.fit(data.map((row) => searchCols.map((col) => row[col] ?? "").join(" ")));
  const results: Row[] = [];
  for (const [idx, score] of bm25.score(query).slice(0, maxResults)) {
    if (score <= 0) continue;
    const row = data[idx]!;
    const projected: Row = {};
    for (const col of outputCols) if (col in row) projected[col] = row[col]!;
    results.push(projected);
  }
  return results;
}

/** Keyword-count domain detection; first domain wins ties, fallback when nothing matches. */
export function detectDomainByKeywords(
  query: string,
  domainKeywords: Record<string, string[]>,
  fallback: string,
): string {
  const queryLower = query.toLowerCase();
  let best = fallback;
  let bestScore = 0;
  for (const [domain, keywords] of Object.entries(domainKeywords)) {
    const score = keywords.filter((kw) => queryLower.includes(kw)).length;
    if (score > bestScore) {
      best = domain;
      bestScore = score;
    }
  }
  return best;
}

export interface SearchResult {
  domain: string;
  query?: string;
  file?: string;
  count?: number;
  results?: Row[];
  error?: string;
}

/** Build the search(query, domain, maxResults) function used by every design sub-skill. */
export function createDomainSearch(options: {
  dataDir: string;
  config: Record<string, DomainConfig>;
  domainKeywords: Record<string, string[]>;
  fallbackDomain: string;
}) {
  const { dataDir, config, domainKeywords, fallbackDomain } = options;
  const detectDomain = (query: string) => detectDomainByKeywords(query, domainKeywords, fallbackDomain);
  const search = async (query: string, domain?: string | null, maxResults = 3): Promise<SearchResult> => {
    const resolved = domain ?? detectDomain(query);
    const cfg = config[resolved] ?? config[fallbackDomain]!;
    const filepath = `${dataDir}/${cfg.file}`;
    if (!(await Bun.file(filepath).exists())) return { error: `File not found: ${filepath}`, domain: resolved };
    const results = await searchCsv(filepath, cfg.search_cols, cfg.output_cols, query, maxResults);
    return { domain: resolved, query, file: cfg.file, count: results.length, results };
  };
  return { detectDomain, search };
}
