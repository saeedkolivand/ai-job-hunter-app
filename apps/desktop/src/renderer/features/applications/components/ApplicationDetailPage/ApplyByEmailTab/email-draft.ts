/** Split raw model output per the OUTPUT CONTRACT: line 1 is "Subject: …". */
export function splitEmail(raw: string): { subject: string; body: string } {
  const firstLine = raw.split('\n')[0] ?? '';
  const m = /^Subject:\s*(.*)$/i.exec(firstLine);
  if (!m) return { subject: '', body: raw.trim() };
  return {
    subject: m[1]?.trim() ?? '',
    body: raw.slice(firstLine.length).replace(/^\n/, '').trim(),
  };
}
