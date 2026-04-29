export const EASY_MOTION_KEYS = "asdfjkl;ghetyuiopqwrvbnmcxz";

export function assignLabels(count: number, keys: string = EASY_MOTION_KEYS): string[] {
  if (count <= 0) return [];
  const k = keys.length;
  if (count <= k) {
    return keys.slice(0, count).split("");
  }

  // flash.nvim style: use the last p keys as prefixes for two-char labels.
  // Single-char labels = (k - p), two-char labels = p * k.
  // (k - p) + p * k >= count  <=>  p >= (count - k) / (k - 1)
  const p = Math.min(k, Math.ceil((count - k) / (k - 1)));

  const singles: string[] = keys.slice(0, k - p).split("");
  const doubles: string[] = [];
  for (let i = k - p; i < k; i++) {
    for (let j = 0; j < k; j++) {
      doubles.push(keys[i] + keys[j]);
    }
  }

  const result = [...singles, ...doubles];
  // Capacity is 26 + 26*26 = 702. Anything beyond that returns an empty
  // string so the UI can render it as "no label".
  while (result.length < count) result.push("");
  return result.slice(0, count);
}
