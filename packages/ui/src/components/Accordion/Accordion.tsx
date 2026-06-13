import { ChevronRight } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useState } from 'react';

import { cn } from '../../lib/cn';
import { transition } from '../../lib/motion';
import { Button } from '../Button';

export interface AccordionProps {
  title: string;
  content: string | React.ReactNode;
  defaultOpen?: boolean;
}

export function Accordion({ title, content, defaultOpen = false }: AccordionProps) {
  const [open, setOpen] = useState(defaultOpen);

  return (
    <div className="surface-card overflow-hidden rounded-lg">
      {/* Ant-Design Collapse grammar: a left caret that rotates 90° on expand,
         a subtly filled header that reads distinct from the white body, and a
         hairline divider between header and content. */}
      <Button
        variant="unstyled"
        onClick={() => setOpen((o) => !o)}
        aria-expanded={open}
        className={cn(
          'flex w-full items-center gap-2.5 px-4 py-3 text-left transition-colors',
          open ? 'bg-foreground/[0.04]' : 'bg-foreground/[0.02] hover:bg-foreground/[0.04]'
        )}
      >
        <ChevronRight
          size={14}
          className={cn(
            'shrink-0 text-foreground/45 transition-transform duration-200',
            open && 'rotate-90'
          )}
        />
        <span className="text-sm font-medium text-foreground/85">{title}</span>
      </Button>
      <AnimatePresence initial={false}>
        {open && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={transition.relaxed}
          >
            <div className="border-t border-[var(--border-soft)] px-4 py-4 text-sm leading-relaxed text-foreground/60">
              {typeof content === 'string' ? (
                // Trust boundary: `content` strings reach this sink ONLY from the
                // Support FAQ (apps/.../SupportPage), which passes `t(...)` values
                // from the statically-bundled @ajh/translations resources (en/de
                // JSON, no network/user/AI input). Every other caller (AnalysisResults,
                // StepExtras) passes JSX (React.ReactNode), which takes the `else`
                // branch below and never touches dangerouslySetInnerHTML. No
                // untrusted (AI-generated / resume-derived / network) text is rendered
                // here, so the markup is safe to inject without sanitization.
                <div dangerouslySetInnerHTML={{ __html: content }} />
              ) : (
                content
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
