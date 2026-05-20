/**
 * Lightweight markdown renderer for AI chat messages.
 * Handles the subset LLMs actually produce: headings, bold, italic, inline
 * code, code blocks, ordered/unordered lists, blockquotes, and horizontal rules.
 * No external dependency.
 */
import { cn } from '../lib/cn';

interface Props {
  content: string;
  className?: string;
}

export function MarkdownMessage({ content, className }: Props) {
  return <div className={cn('markdown-message', className)}>{renderBlocks(content)}</div>;
}

function renderBlocks(text: string): React.ReactNode[] {
  const lines = text.split('\n');
  const nodes: React.ReactNode[] = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];
    if (!line) {
      i++;
      continue;
    }

    if (line.startsWith('```')) {
      const lang = line.slice(3).trim();
      const codeLines: string[] = [];
      i++;
      while (i < lines.length && !lines[i]?.startsWith('```')) {
        codeLines.push(lines[i] ?? '');
        i++;
      }
      i++;
      nodes.push(
        <pre
          key={nodes.length}
          className="my-2 overflow-x-auto rounded-lg bg-white/[0.06] px-4 py-3 text-[12px] font-mono text-foreground/85"
        >
          {lang && (
            <span className="mb-1 block text-[10px] uppercase tracking-wider text-foreground/35">
              {lang}
            </span>
          )}
          <code>{codeLines.join('\n')}</code>
        </pre>
      );
      continue;
    }

    const headingMatch = /^(#{1,3})\s+(.+)$/.exec(line);
    if (headingMatch && headingMatch[1] && headingMatch[2]) {
      const level = headingMatch[1].length;
      const sizeClass =
        level === 1
          ? 'text-base font-semibold'
          : level === 2
            ? 'text-sm font-semibold'
            : 'text-sm font-medium';
      nodes.push(
        <div key={nodes.length} className={cn('mb-1 mt-3 text-foreground/90', sizeClass)}>
          {renderInline(headingMatch[2])}
        </div>
      );
      i++;
      continue;
    }

    if (/^---+$/.test(line.trim())) {
      nodes.push(<hr key={nodes.length} className="my-3 border-white/10" />);
      i++;
      continue;
    }

    if (line.startsWith('> ')) {
      const quoteLines: string[] = [];
      while (i < lines.length && lines[i]?.startsWith('> ')) {
        quoteLines.push(lines[i]?.slice(2) ?? '');
        i++;
      }
      nodes.push(
        <blockquote
          key={nodes.length}
          className="my-2 border-l-2 border-brand/40 pl-3 italic text-foreground/60"
        >
          {renderBlocks(quoteLines.join('\n'))}
        </blockquote>
      );
      continue;
    }

    if (/^[-*+]\s/.test(line)) {
      const items: string[] = [];
      while (i < lines.length && /^[-*+]\s/.test(lines[i] ?? '')) {
        items.push((lines[i] ?? '').replace(/^[-*+]\s/, ''));
        i++;
      }
      nodes.push(
        <ul key={nodes.length} className="my-2 space-y-1 pl-4">
          {items.map((item, idx) => (
            <li key={idx} className="flex gap-2 text-sm text-foreground/80">
              <span className="mt-[6px] h-1.5 w-1.5 shrink-0 rounded-full bg-brand/60" />
              <span>{renderInline(item)}</span>
            </li>
          ))}
        </ul>
      );
      continue;
    }

    if (/^\d+\.\s/.test(line)) {
      const items: string[] = [];
      while (i < lines.length && /^\d+\.\s/.test(lines[i] ?? '')) {
        items.push((lines[i] ?? '').replace(/^\d+\.\s/, ''));
        i++;
      }
      nodes.push(
        <ol key={nodes.length} className="my-2 space-y-1 pl-4">
          {items.map((item, idx) => (
            <li key={idx} className="flex gap-2 text-sm text-foreground/80">
              <span className="shrink-0 font-medium tabular-nums text-brand/60">{idx + 1}.</span>
              <span>{renderInline(item)}</span>
            </li>
          ))}
        </ol>
      );
      continue;
    }

    if (line.trim() === '') {
      i++;
      continue;
    }

    const paraLines: string[] = [];
    while (
      i < lines.length &&
      (lines[i]?.trim() ?? '') !== '' &&
      !lines[i]?.startsWith('#') &&
      !lines[i]?.startsWith('```') &&
      !/^[-*+]\s/.test(lines[i] ?? '') &&
      !/^\d+\.\s/.test(lines[i] ?? '') &&
      !lines[i]?.startsWith('> ') &&
      !/^---+$/.test(lines[i]?.trim() ?? '')
    ) {
      paraLines.push(lines[i] ?? '');
      i++;
    }

    if (paraLines.length > 0) {
      nodes.push(
        <p key={nodes.length} className="text-sm leading-relaxed text-foreground/85">
          {renderInline(paraLines.join(' '))}
        </p>
      );
    }
  }

  return nodes;
}

function renderInline(text: string): React.ReactNode {
  const parts = text.split(/(\*\*[^*]+\*\*|__[^_]+__|`[^`]+`|\*[^*]+\*|_[^_]+_)/g);
  return parts.map((part, i) => {
    if (part.startsWith('**') || part.startsWith('__'))
      return (
        <strong key={i} className="font-semibold text-foreground/95">
          {part.slice(2, -2)}
        </strong>
      );
    if (part.startsWith('`'))
      return (
        <code
          key={i}
          className="rounded bg-white/[0.08] px-1.5 py-0.5 font-mono text-[11px] text-brand-soft"
        >
          {part.slice(1, -1)}
        </code>
      );
    if (part.startsWith('*') || part.startsWith('_'))
      return (
        <em key={i} className="italic text-foreground/80">
          {part.slice(1, -1)}
        </em>
      );
    return part;
  });
}
