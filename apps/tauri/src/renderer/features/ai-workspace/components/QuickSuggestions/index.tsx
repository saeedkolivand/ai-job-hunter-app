import { Button } from '@ajh/ui';

const SUGGESTIONS = [
  { q: 'How do I search for jobs on LinkedIn?', icon: '🔍' },
  { q: 'How do I tailor my resume for a role?', icon: '📝' },
  { q: 'What does the ATS score mean?', icon: '📊' },
  { q: 'How do I set up Autopilot?', icon: '🤖' },
] as const;

export function QuickSuggestions({ onSelect }: { onSelect: (question: string) => void }) {
  return (
    <div className="mt-6 grid grid-cols-2 gap-2 max-w-lg">
      {SUGGESTIONS.map(({ q, icon }) => (
        <Button
          key={q}
          onClick={() => onSelect(q)}
          className="flex items-start gap-2.5 rounded-xl border border-white/[0.07] bg-white/[0.02] px-3 py-2.5 text-left text-xs text-foreground/60 hover:border-brand/20 hover:bg-brand/5 hover:text-foreground/80 transition-all h-auto"
        >
          <span className="text-base leading-none shrink-0">{icon}</span>
          <span>{q}</span>
        </Button>
      ))}
    </div>
  );
}
