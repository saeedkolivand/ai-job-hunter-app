import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

/**
 * Markdown component map — design tokens only, no hardcoded hex.
 *
 * Heading mapping (job title is <h2> in the detail pane, so nest under it):
 *   h1 → <h2>  large  font-semibold
 *   h2 → <h3>  medium font-semibold
 *   h3 → <h4>  caption font-semibold
 *   h4 → <h5>  caption font-medium (de-emphasised sub-head)
 *
 * No heading levels are collapsed or skipped — each maps to the next real
 * HTML element one step lower, preserving WCAG 1.3.1 outline order.
 */
const mdComponents: React.ComponentProps<typeof ReactMarkdown>['components'] = {
  h1: ({ children }) => (
    <h2 className="mt-3 text-sm font-semibold leading-snug text-foreground/90 first:mt-0">
      {children}
    </h2>
  ),
  h2: ({ children }) => (
    <h3 className="mt-2 text-caption font-semibold leading-snug text-foreground/90 first:mt-0">
      {children}
    </h3>
  ),
  h3: ({ children }) => (
    <h4 className="mt-2 text-caption font-semibold leading-snug text-foreground/80 first:mt-0">
      {children}
    </h4>
  ),
  h4: ({ children }) => (
    <h5 className="mt-1.5 text-caption font-medium leading-snug text-foreground/75 first:mt-0">
      {children}
    </h5>
  ),
  p: ({ children }) => (
    <p className="leading-relaxed text-caption text-foreground/80">{children}</p>
  ),
  ul: ({ children }) => <ul className="list-disc pl-5 leading-relaxed">{children}</ul>,
  ol: ({ children }) => <ol className="list-decimal pl-5 leading-relaxed">{children}</ol>,
  li: ({ children }) => <li className="my-0.5">{children}</li>,
  strong: ({ children }) => <strong className="font-semibold">{children}</strong>,
  // Links: render as brand-colored plain text; job descriptions should not be live nav.
  a: ({ children }) => <span className="text-brand">{children}</span>,
  // Tables: block-scroll wrapper so wide tables don't overflow max-w-prose.
  table: ({ children }) => (
    <div className="block w-full overflow-x-auto">
      <table className="w-full border-collapse text-caption">{children}</table>
    </div>
  ),
  th: ({ children }) => (
    <th className="border border-border px-2 py-1 text-left font-semibold text-foreground/80">
      {children}
    </th>
  ),
  td: ({ children }) => (
    <td className="border border-border px-2 py-1 text-foreground/70">{children}</td>
  ),
  // Inline code and fenced code blocks.
  code: ({ children, className }) => {
    const isBlock = Boolean(className);
    return isBlock ? (
      <code className={className}>{children}</code>
    ) : (
      <code className="rounded bg-muted px-1 font-mono text-fine-print">{children}</code>
    );
  },
  pre: ({ children }) => (
    <pre className="overflow-x-auto rounded bg-muted p-3 font-mono text-fine-print leading-relaxed">
      {children}
    </pre>
  ),
};

interface JobDescriptionProps {
  /** Raw markdown (or plain text) string to render. */
  markdown: string;
  className?: string;
}

/**
 * Renders a job description as GFM markdown with design-token-only styling.
 * Links are rendered as non-navigable brand-colored spans (job ad safety).
 * Tables and code blocks scroll horizontally so they don't overflow max-w-prose.
 *
 * Pure presentational — no IPC, no routing, no store.
 */
export function JobDescription({ markdown, className }: JobDescriptionProps) {
  return (
    <div className={className}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={mdComponents}>
        {markdown}
      </ReactMarkdown>
    </div>
  );
}
