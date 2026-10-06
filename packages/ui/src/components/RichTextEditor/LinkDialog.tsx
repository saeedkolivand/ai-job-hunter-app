import { cn } from '../../lib/cn';
import { Button } from '../Button';
import { Input } from '../Input';
import { ModalShell } from '../ModalShell';
import { linkUrlHint, type ToolbarLabels } from './toolbarLabels';
import type { LinkDialogState } from './useLinkDialog';

interface LinkDialogProps {
  open: boolean;
  /** Whether the selection is already a link (shows the Remove button). */
  linkActive: boolean;
  labels: Required<ToolbarLabels>;
  dialog: LinkDialogState;
}

/** The add/edit-link modal: label + URL fields, suggestions pick-list, actions. */
export function LinkDialog({ open, linkActive, labels: l, dialog }: LinkDialogProps) {
  const {
    linkLabel,
    setLinkLabel,
    linkUrl,
    setLinkUrl,
    urlError,
    setUrlError,
    closeLinkDialog,
    applyLink,
    removeLink,
    pickSuggestion,
    visibleSuggestions,
  } = dialog;

  return (
    <ModalShell
      open={open}
      onClose={closeLinkDialog}
      maxWidth="max-w-sm"
      ariaLabel={l.linkDialogTitle}
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          applyLink();
        }}
        className="flex flex-col gap-3 p-4"
      >
        <h2 className="text-sm font-semibold text-foreground/90">{l.linkDialogTitle}</h2>
        <label className="flex flex-col gap-1 text-xs text-foreground/60">
          {l.linkLabelField}
          <Input value={linkLabel} onChange={(e) => setLinkLabel(e.target.value)} autoFocus />
        </label>
        <label className="flex flex-col gap-1 text-xs text-foreground/60">
          {l.linkUrlField}
          <Input
            type="url"
            value={linkUrl}
            placeholder={l.linkUrlPlaceholder}
            onChange={(e) => {
              setLinkUrl(e.target.value);
              if (urlError) setUrlError(false);
            }}
            aria-invalid={urlError}
            className={cn(urlError && 'border-red-500/50')}
          />
        </label>
        {urlError && <p className="text-xs text-red-300">{l.linkUrlError}</p>}
        {visibleSuggestions.length > 0 && (
          <div className="flex flex-col gap-1">
            <span className="text-xs font-medium text-foreground/50">{l.linkSuggestionsTitle}</span>
            <ul className="flex max-h-40 flex-col gap-0.5 overflow-y-auto rounded-md border border-white/[0.06] bg-white/[0.02] p-1">
              {visibleSuggestions.map((s) => (
                <li key={`${s.label}\u0000${s.url}`}>
                  <Button
                    type="button"
                    variant="unstyled"
                    // Filling the fields must not blur/close the dialog inputs.
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => pickSuggestion(s)}
                    aria-label={`${s.label} — ${s.url}`}
                    className={cn(
                      'flex w-full items-baseline justify-between gap-3 rounded px-2 py-1.5 text-left transition-colors',
                      'hover:bg-brand/10 focus-visible:bg-brand/10'
                    )}
                  >
                    <span className="truncate text-xs text-foreground/90">{s.label}</span>
                    <span className="shrink-0 truncate font-mono text-[0.7rem] text-foreground/45">
                      {linkUrlHint(s.url)}
                    </span>
                  </Button>
                </li>
              ))}
            </ul>
          </div>
        )}
        <div className="flex items-center justify-between gap-2 pt-1">
          {linkActive ? (
            <Button type="button" variant="danger" size="sm" onClick={removeLink}>
              {l.linkRemove}
            </Button>
          ) : (
            <span />
          )}
          <div className="flex gap-2">
            <Button type="button" variant="ghost" size="sm" onClick={closeLinkDialog}>
              {l.linkCancel}
            </Button>
            <Button type="submit" variant="primary" size="sm">
              {l.linkSave}
            </Button>
          </div>
        </div>
      </form>
    </ModalShell>
  );
}
