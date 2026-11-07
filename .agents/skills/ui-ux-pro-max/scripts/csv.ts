/**
 * RFC 4180 CSV reader/writer with Python `csv` module semantics
 * (excel dialect, non-strict): quoted fields may span lines, doubled quotes
 * escape a quote, blank lines are skipped by the record reader.
 */

export type Row = Record<string, string>;

/** Parse CSV text into rows of fields. Blank lines yield empty rows. */
export function parseCsv(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = "";
  let inQuotes = false;
  let fieldStarted = false;
  let i = 0;
  const n = text.length;

  const endRow = () => {
    if (fieldStarted || row.length > 0) {
      row.push(field);
    }
    rows.push(row);
    row = [];
    field = "";
    fieldStarted = false;
  };

  while (i < n) {
    const ch = text[i]!;
    if (inQuotes) {
      if (ch === '"') {
        if (text[i + 1] === '"') {
          field += '"';
          i += 2;
          continue;
        }
        inQuotes = false;
        i += 1;
        continue;
      }
      field += ch;
      i += 1;
      continue;
    }
    if (ch === '"' && field === "" && !fieldStarted) {
      inQuotes = true;
      fieldStarted = true;
      i += 1;
      continue;
    }
    if (ch === ",") {
      row.push(field);
      field = "";
      fieldStarted = false;
      i += 1;
      continue;
    }
    if (ch === "\r" || ch === "\n") {
      endRow();
      i += ch === "\r" && text[i + 1] === "\n" ? 2 : 1;
      continue;
    }
    field += ch;
    fieldStarted = true;
    i += 1;
  }
  if (fieldStarted || row.length > 0 || field !== "") {
    endRow();
  }
  return rows;
}

/** csv.DictReader semantics: first row is the header, blank rows are skipped. */
export function parseCsvRecords(text: string): Row[] {
  const rows = parseCsv(text.startsWith("﻿") ? text.slice(1) : text);
  const header = rows.shift();
  if (!header) return [];
  const records: Row[] = [];
  for (const values of rows) {
    if (values.length === 0) continue;
    const record: Row = {};
    header.forEach((name, index) => {
      record[name] = values[index] ?? "";
    });
    records.push(record);
  }
  return records;
}

const NEEDS_QUOTES = /[",\r\n]/;

/** Minimal quoting, like csv.writer(QUOTE_MINIMAL). */
export function formatCsv(rows: readonly (readonly string[])[], lineEnding = "\n"): string {
  return rows
    .map((row) =>
      row
        .map((value) => {
          const text = String(value ?? "");
          return NEEDS_QUOTES.test(text) ? `"${text.replaceAll('"', '""')}"` : text;
        })
        .join(","),
    )
    .join(lineEnding) + (rows.length ? lineEnding : "");
}

/** Format records with an explicit header order. */
export function formatCsvRecords(header: readonly string[], records: readonly Row[], lineEnding = "\n"): string {
  return formatCsv([header, ...records.map((record) => header.map((name) => record[name] ?? ""))], lineEnding);
}
