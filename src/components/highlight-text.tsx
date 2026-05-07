import { Fragment } from "react";
import { fuzzyMatch } from "@/lib/fuzzy";

interface Props {
  text: string;
  query: string;
}

export function HighlightText({ text, query }: Props) {
  if (query.trim() === "") return <>{text}</>;
  const match = fuzzyMatch(query, text);
  if (!match || match.positions.length === 0) return <>{text}</>;

  // Coalesce contiguous positions into single highlighted segments to avoid
  // one <span> per char, which would also defeat any letter-spacing / kerning.
  const hits = new Set(match.positions);
  const segments: Array<{ text: string; matched: boolean }> = [];
  let buf = "";
  let bufMatched = false;
  for (let i = 0; i < text.length; i++) {
    const matched = hits.has(i);
    if (i === 0) {
      buf = text[i];
      bufMatched = matched;
      continue;
    }
    if (matched === bufMatched) {
      buf += text[i];
    } else {
      segments.push({ text: buf, matched: bufMatched });
      buf = text[i];
      bufMatched = matched;
    }
  }
  if (buf) segments.push({ text: buf, matched: bufMatched });

  return (
    <>
      {segments.map((seg, i) =>
        seg.matched ? (
          <span
            key={i}
            className="font-semibold bg-amber-500/25 text-[var(--text-primary)] rounded-sm"
          >
            {seg.text}
          </span>
        ) : (
          <Fragment key={i}>{seg.text}</Fragment>
        ),
      )}
    </>
  );
}
