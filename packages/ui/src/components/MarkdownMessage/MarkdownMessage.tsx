/**
 * Lightweight markdown renderer for AI chat messages and release notes.
 * Handles the subset that LLMs and changelogs actually produce: headings, bold,
 * italic, inline code, code blocks, ordered/unordered lists, blockquotes,
 * horizontal rules, and `[text](url)` links. No external dependency.
 *
 * Links: the renderer is IPC-free, so it can't open a URL itself. Pass
 * `onLinkClick` (e.g. wired to the Tauri opener) to make links clickable;
 * without it, a link renders as plain label text — never a raw `<a href>` that
 * would navigate the webview.
 */
import { cn } from '../../lib/cn';

type LinkClick = ((url: string) => void) | undefined;

interface Props {
  content: string;
  className?: string;
  /** Open a link's URL (e.g. via the system browser). Omit to render link text only. */
  onLinkClick?: (url: string) => void;
}

export function MarkdownMessage({ content, className, onLinkClick }: Props) {
  return (
    <div className={cn('markdown-message', className)}>{renderBlocks(content, onLinkClick)}</div>
  );
}

function renderBlocks(text: string, onLinkClick: LinkClick): React.ReactNode[] {
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
          {renderInline(headingMatch[2], onLinkClick)}
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
          {renderBlocks(quoteLines.join('\n'), onLinkClick)}
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
              <span>{renderInline(item, onLinkClick)}</span>
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
              <span>{renderInline(item, onLinkClick)}</span>
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
          {renderInline(paraLines.join(' '), onLinkClick)}
        </p>
      );
    }
  }

  return nodes;
}

// `[label](url)` first so it wins over `*`/`_` emphasis inside the same string.
const INLINE_SPLIT = /(\[[^\]]+\]\([^)]+\)|\*\*[^*]+\*\*|__[^_]+__|`[^`]+`|\*[^*]+\*|_[^_]+_)/g;
const LINK_RE = /^\[([^\]]+)\]\(([^)]+)\)$/;

function renderInline(text: string, onLinkClick: LinkClick): React.ReactNode {
  const parts = text.split(INLINE_SPLIT);
  return parts.map((part, i) => {
    const link = LINK_RE.exec(part);
    if (link) {
      const label = link[1] ?? '';
      const url = link[2] ?? '';
      if (onLinkClick) {
        return (
          <a
            key={i}
            role="link"
            tabIndex={0}
            onClick={() => onLinkClick(url)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                onLinkClick(url);
              }
            }}
            className="cursor-pointer text-brand-soft underline underline-offset-2 hover:text-brand"
          >
            {label}
          </a>
        );
      }
      // No handler → show the label only (never a raw href that navigates the webview).
      return <span key={i}>{label}</span>;
    }
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
