export interface FuzzyMatch {
  score: number;
  positions: number[]; // 0-indexed positions into target.toLowerCase()
}

// Fuzzy match. Lower score is better. Returns null when query is not a
// subsequence of target. Empty/whitespace-only queries match with score 0.
export function fuzzyMatch(query: string, target: string): FuzzyMatch | null {
  const q = query.trim().toLowerCase();
  if (q === "") return { score: 0, positions: [] };
  const t = target.toLowerCase();

  const subIdx = t.indexOf(q);
  if (subIdx >= 0) {
    const positions: number[] = [];
    for (let i = 0; i < q.length; i++) positions.push(subIdx + i);
    return { score: subIdx * 2 + (t.length - q.length), positions };
  }

  let ti = 0;
  let score = 0;
  let lastMatch = -2;
  const positions: number[] = [];
  for (let qi = 0; qi < q.length; qi++) {
    const ch = q[qi];
    let found = -1;
    while (ti < t.length) {
      if (t[ti] === ch) {
        found = ti;
        ti++;
        break;
      }
      ti++;
    }
    if (found < 0) return null;
    let charCost = found * 0.1 + 1;
    if (found === lastMatch + 1) charCost -= 0.8;
    if (found === 0 || isWordBoundary(t[found - 1])) charCost -= 0.5;
    score += Math.max(charCost, 0);
    lastMatch = found;
    positions.push(found);
  }
  // keep subsequence matches strictly worse than substring
  return { score: score + 100, positions };
}

export function fuzzyScore(query: string, target: string): number | null {
  return fuzzyMatch(query, target)?.score ?? null;
}

function isWordBoundary(ch: string): boolean {
  return ch === " " || ch === "-" || ch === "_" || ch === "/" || ch === ".";
}
