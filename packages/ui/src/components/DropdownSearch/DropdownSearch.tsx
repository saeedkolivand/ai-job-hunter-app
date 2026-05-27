import { Search } from 'lucide-react';

export function DropdownSearch({
  search,
  setSearch,
  searchRef,
}: {
  search: string;
  setSearch: (value: string) => void;
  searchRef: React.RefObject<HTMLInputElement | null>;
}) {
  return (
    <div className="border-b border-white/[0.06] px-2 py-2">
      <div className="flex items-center gap-2 rounded-lg bg-white/[0.04] px-2.5 py-1.5">
        <Search size={11} className="shrink-0 text-foreground/30" />
        <input
          ref={searchRef}
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search…"
          className="flex-1 bg-transparent text-xs text-foreground outline-none placeholder:text-foreground/25"
        />
      </div>
    </div>
  );
}
