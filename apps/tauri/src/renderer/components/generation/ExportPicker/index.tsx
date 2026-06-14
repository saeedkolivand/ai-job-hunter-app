import { Download, Loader2 } from 'lucide-react';
import type { ReactNode } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, cn, ModalShell, SegmentedControl } from '@ajh/ui';

import type { TemplateId } from '@/lib/generate';

export const EXPORT_FORMATS = ['pdf', 'docx', 'txt'] as const;
export type ExportFormat = (typeof EXPORT_FORMATS)[number];

export interface ExportTemplateOption {
  id: TemplateId;
  label: string;
}

interface BaseProps {
  open: boolean;
  onClose: () => void;
  /** Which format chips to offer — defaults to PDF/DOCX/TXT. */
  formats?: readonly ExportFormat[];
  format: ExportFormat;
  onFormatChange: (format: ExportFormat) => void;
  /**
   * When provided, a second segmented control lets the user pick a template.
   * Hidden automatically for the `txt` format (plain text has no template).
   */
  templateId?: TemplateId;
  onTemplateChange?: (id: TemplateId) => void;
  templateOptions?: ExportTemplateOption[];
  /** z-index for the modal — forwarded to ModalShell (default 600). */
  zIndex?: number;
}

/**
 * "Immediate" mode (autopilot ApplyPage): the picker has no separate confirm
 * step — choosing a format fires `onExport(format)` and closes the modal. The
 * template/ATS are already chosen in the surrounding toolbar.
 */
interface ImmediateProps extends BaseProps {
  onExport: (format: ExportFormat) => void;
  children?: never;
}

/**
 * "Composed" mode (resumes GenerationCard): the caller renders its own action
 * buttons (e.g. "Export Resume" / "Export Cover Letter") as children, each
 * reading the picked `format` + `templateId`.
 */
interface ComposedProps extends BaseProps {
  onExport?: never;
  children: ReactNode;
}

export type ExportPickerProps = ImmediateProps | ComposedProps;

/**
 * Shared, accessible export-format picker used by every generation surface
 * (resumes list cards + autopilot ApplyPage). Built on the focus-trapped,
 * Escape-dismissable `ModalShell` from `@ajh/ui` so both call sites get the same
 * a11y guarantees instead of re-rolling an overlay.
 *
 * Two shapes (see the prop unions above):
 *  - immediate: pass `onExport` — clicking a format chip downloads that format.
 *  - composed: pass `children` — the caller owns the export action buttons.
 */
export function ExportPicker(props: ExportPickerProps) {
  const {
    open,
    onClose,
    formats = EXPORT_FORMATS,
    format,
    onFormatChange,
    templateId,
    onTemplateChange,
    templateOptions,
    zIndex,
  } = props;
  const { t } = useTranslation();

  const showTemplatePicker =
    format !== 'txt' &&
    templateId !== undefined &&
    onTemplateChange !== undefined &&
    templateOptions !== undefined &&
    templateOptions.length > 0;

  // In immediate mode a format click both selects and fires the export.
  const handleFormatSelect = (next: ExportFormat) => {
    onFormatChange(next);
    if ('onExport' in props && props.onExport) props.onExport(next);
  };

  return (
    <ModalShell open={open} onClose={onClose} ariaLabel={t('aiGenerate.export')} zIndex={zIndex}>
      <div className="space-y-4 p-5">
        <div>
          <h3 className="text-sm font-semibold text-foreground/90">
            {t('aiGenerate.exportTitle')}
          </h3>
          {'onExport' in props && (
            <p className="mt-0.5 text-[11px] text-foreground/45">
              {t('aiGenerate.exportSubtitle')}
            </p>
          )}
        </div>

        {'onExport' in props && props.onExport ? (
          // Immediate mode: one button per format → download on click.
          <div className="flex flex-col gap-1.5">
            {formats.map((fmt) => (
              <Button
                key={fmt}
                variant="unstyled"
                type="button"
                onClick={() => handleFormatSelect(fmt)}
                className={cn(
                  'flex w-full items-center gap-2.5 rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2.5 text-left transition-colors hover:border-brand/30 hover:bg-brand/8'
                )}
              >
                <Download size={14} className="shrink-0 text-brand-soft" />
                <span className="flex min-w-0 flex-1 flex-col">
                  <span className="text-[12px] font-medium text-foreground/85">
                    {t('aiGenerate.download', { fmt: fmt.toUpperCase() })}
                  </span>
                  <span className="text-[10px] text-foreground/45">
                    {t(`aiGenerate.export${fmt.charAt(0).toUpperCase()}${fmt.slice(1)}Desc`)}
                  </span>
                </span>
              </Button>
            ))}
          </div>
        ) : (
          // Composed mode: format (+ optional template) selectors, caller-owned actions.
          <>
            <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
              <SegmentedControl<ExportFormat>
                ariaLabel={t('resumes.generated.format')}
                size="sm"
                value={format}
                onChange={onFormatChange}
                options={formats.map((fmt) => ({ value: fmt, label: fmt.toUpperCase() }))}
              />
              {showTemplatePicker && (
                <SegmentedControl<TemplateId>
                  ariaLabel={t('resumes.generated.template')}
                  size="sm"
                  value={templateId}
                  onChange={onTemplateChange}
                  options={templateOptions.map(({ id, label }) => ({ value: id, label }))}
                />
              )}
            </div>
            <div className="flex flex-wrap items-center gap-2">{props.children}</div>
          </>
        )}
      </div>
    </ModalShell>
  );
}

/** Convenience: the download icon + spinner pattern used by composed-mode actions. */
export function ExportActionIcon({ loading }: { loading: boolean }) {
  return loading ? <Loader2 size={11} className="animate-spin" /> : <Download size={11} />;
}
