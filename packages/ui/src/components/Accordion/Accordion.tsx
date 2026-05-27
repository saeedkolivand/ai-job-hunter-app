import { ChevronDown } from 'lucide-react';
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
    <div
      className={cn(
        'overflow-hidden rounded-xl border-transparent transition-colors duration-150',
        open ? 'border-transparent bg-white/[0.04]' : 'border-transparent bg-white/[0.02]'
      )}
    >
      <Button
        variant="ghost"
        onClick={() => setOpen((o) => !o)}
        className="flex w-full items-center justify-between gap-4 px-5 py-4 text-left hover:bg-transparent"
      >
        <span
          className={cn(
            'text-sm font-medium transition-colors',
            open ? 'text-foreground/90' : 'text-foreground/65'
          )}
        >
          {title}
        </span>
        <ChevronDown
          size={15}
          className={cn(
            'shrink-0 text-foreground/30 transition-transform duration-200',
            open && 'rotate-180'
          )}
        />
      </Button>
      <AnimatePresence initial={false}>
        {open && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={transition.relaxed}
          >
            <div className="border-transparent px-5 py-4 text-sm leading-relaxed text-foreground/60">
              {typeof content === 'string' ? (
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
