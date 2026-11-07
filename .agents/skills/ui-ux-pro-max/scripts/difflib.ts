/**
 * Port of Python's difflib.SequenceMatcher(None, a, b).ratio() for strings,
 * including the autojunk "popular element" heuristic (len(b) >= 200).
 * Sequences are compared by Unicode code point, as in Python.
 */

interface Match {
  i: number;
  j: number;
  size: number;
}

function buildB2j(b: readonly string[]): { b2j: Map<string, number[]>; popular: Set<string> } {
  const b2j = new Map<string, number[]>();
  b.forEach((element, index) => {
    const indices = b2j.get(element);
    if (indices) indices.push(index);
    else b2j.set(element, [index]);
  });
  const popular = new Set<string>();
  const n = b.length;
  if (n >= 200) {
    const ntest = Math.floor(n / 100) + 1;
    for (const [element, indices] of b2j) {
      if (indices.length > ntest) popular.add(element);
    }
    for (const element of popular) b2j.delete(element);
  }
  return { b2j, popular };
}

function findLongestMatch(
  a: readonly string[],
  b: readonly string[],
  b2j: Map<string, number[]>,
  alo: number,
  ahi: number,
  blo: number,
  bhi: number,
): Match {
  let besti = alo;
  let bestj = blo;
  let bestsize = 0;
  let j2len = new Map<number, number>();
  for (let i = alo; i < ahi; i++) {
    const newj2len = new Map<number, number>();
    for (const j of b2j.get(a[i]!) ?? []) {
      if (j < blo) continue;
      if (j >= bhi) break;
      const k = (j2len.get(j - 1) ?? 0) + 1;
      newj2len.set(j, k);
      if (k > bestsize) {
        besti = i - k + 1;
        bestj = j - k + 1;
        bestsize = k;
      }
    }
    j2len = newj2len;
  }
  // No junk predicate is used, so extend with equal neighbours only (this
  // recovers matches around "popular" elements dropped by autojunk).
  while (besti > alo && bestj > blo && a[besti - 1] === b[bestj - 1]) {
    besti--;
    bestj--;
    bestsize++;
  }
  while (besti + bestsize < ahi && bestj + bestsize < bhi && a[besti + bestsize] === b[bestj + bestsize]) {
    bestsize++;
  }
  return { i: besti, j: bestj, size: bestsize };
}

/** Sum of matching block sizes (get_matching_blocks). */
function matchingCharacters(a: readonly string[], b: readonly string[]): number {
  const { b2j } = buildB2j(b);
  const queue: [number, number, number, number][] = [[0, a.length, 0, b.length]];
  let total = 0;
  while (queue.length) {
    const [alo, ahi, blo, bhi] = queue.pop()!;
    const { i, j, size } = findLongestMatch(a, b, b2j, alo, ahi, blo, bhi);
    if (size) {
      total += size;
      if (alo < i && blo < j) queue.push([alo, i, blo, j]);
      if (i + size < ahi && j + size < bhi) queue.push([i + size, ahi, j + size, bhi]);
    }
  }
  return total;
}

/** difflib.SequenceMatcher(None, a, b).ratio() */
export function sequenceRatio(a: string, b: string): number {
  const left = Array.from(a);
  const right = Array.from(b);
  const length = left.length + right.length;
  if (length === 0) return 1;
  return (2 * matchingCharacters(left, right)) / length;
}
