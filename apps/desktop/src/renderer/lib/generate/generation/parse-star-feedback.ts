/** Which of the STAR components the candidate's answer actually covers. */
export interface StarCompleteness {
  situation: boolean;
  task: boolean;
  action: boolean;
  result: boolean;
}

/** Parsed STAR-rubric feedback on one typed practice answer — session-only,
 *  never persisted to the aiGenerations aggregate. */
export interface StarFeedback {
  strengths: string[];
  gaps: string[];
  star: StarCompleteness;
  /** One tightened rewrite of the candidate's answer. */
  rewrite: string;
}

type Section = 'none' | 'strengths' | 'gaps' | 'star' | 'rewrite';

const SECTION_BY_KEY: Record<string, Section> = {
  strengths: 'strengths',
  gaps: 'gaps',
  star: 'star',
  rewrite: 'rewrite',
};

/** Lenient present/missing read for a STAR sub-field value ("present", "yes",
 *  "true" -> present; anything else, including "missing"/"no"/empty, -> absent). */
function isPresent(raw: string): boolean {
  const v = raw.trim().toLowerCase();
  return v.startsWith('present') || v.startsWith('yes') || v === 'true';
}

/** Strip a leading bullet/numbering marker from a list line. */
function stripBullet(line: string): string {
  return line.replace(/^\s*[-*\d.)]+\s*/, '').trim();
}

/** Push a GAPS bullet unless it is the model's literal "no gaps" marker (per
 *  the prompt's `- None` fallback for a clean answer) — otherwise a genuinely
 *  gap-free answer renders a false amber "None" gap in the UI. */
function pushGap(gaps: string[], text: string): void {
  const cleaned = stripBullet(text);
  if (cleaned && cleaned.toLowerCase() !== 'none') gaps.push(cleaned);
}

/**
 * Lenient parser for the delimited STAR-feedback output (`STRENGTHS:` /
 * `GAPS:` / `STAR:` / `REWRITE:` sections). Tolerates bullet-prefix variants,
 * inline content on a section-header line, and missing STAR sub-fields
 * (default to `missing`). Provider-agnostic — never assumes valid JSON.
 */
export function parseStarFeedback(raw: string): StarFeedback {
  const strengths: string[] = [];
  const gaps: string[] = [];
  const star: StarCompleteness = { situation: false, task: false, action: false, result: false };
  const rewriteLines: string[] = [];
  let section: Section = 'none';

  for (const line of raw.split(/\r?\n/)) {
    const header = line.match(/^\s*(STRENGTHS|GAPS|STAR|REWRITE)\s*:\s*(.*)$/i);
    if (header) {
      const key = (header[1] ?? '').toLowerCase();
      section = SECTION_BY_KEY[key] ?? 'none';
      const inline = header[2]?.trim();
      if (inline) {
        if (section === 'strengths') strengths.push(stripBullet(inline));
        else if (section === 'gaps') pushGap(gaps, inline);
        else if (section === 'rewrite') rewriteLines.push(inline);
      }
      continue;
    }

    if (section === 'star') {
      const situation = line.match(/^\s*SITUATION:\s*(.+)$/i);
      const task = line.match(/^\s*TASK:\s*(.+)$/i);
      const action = line.match(/^\s*ACTION:\s*(.+)$/i);
      const result = line.match(/^\s*RESULT:\s*(.+)$/i);
      if (situation) star.situation = isPresent(situation[1] ?? '');
      else if (task) star.task = isPresent(task[1] ?? '');
      else if (action) star.action = isPresent(action[1] ?? '');
      else if (result) star.result = isPresent(result[1] ?? '');
      continue;
    }
    if (!line.trim()) continue;
    if (section === 'strengths') strengths.push(stripBullet(line));
    else if (section === 'gaps') pushGap(gaps, line);
    else if (section === 'rewrite') rewriteLines.push(line.trim());
  }

  return {
    strengths: strengths.filter(Boolean),
    gaps: gaps.filter(Boolean),
    star,
    rewrite: rewriteLines.join(' ').trim(),
  };
}
