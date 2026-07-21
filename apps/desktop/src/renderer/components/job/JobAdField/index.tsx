import { Briefcase, Link2 } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, cn, CollapsibleFileInput } from '@ajh/ui';

import { type JobAdProvenance, JobUrlImport } from '../JobUrlImport';

interface Props {
  label: string;
  value: string;
  onChange: (value: string) => void;
  uploading: boolean;
  onUpload: (file: File) => void;
  placeholder: string;
  uploadText: string;
  disabled?: boolean;
  /**
   * Optional URL-import sink (ADR-031): when provided, a successful URL import
   * calls this with the composed text + provenance so the caller can persist
   * `jobUrl`/`board`. When omitted, the import falls back to `onChange` (the
   * Resume Analyzer, which doesn't persist provenance). Manual edits still go
   * through `onChange`, so a paste-over there is the caller's cue to clear it.
   */
  onImport?: (text: string, provenance: JobAdProvenance) => void;
}

/**
 * Job-ad input: a collapsible paste/upload field with a "Link" button next to
 * Upload that reveals a URL importer (LinkedIn, Greenhouse, …). Used by AI
 * Generate and the Resume Analyzer.
 */
export function JobAdField({
  label,
  value,
  onChange,
  uploading,
  onUpload,
  placeholder,
  uploadText,
  disabled,
  onImport,
}: Props) {
  const { t } = useTranslation();
  const [showUrl, setShowUrl] = useState(false);

  return (
    <div className="space-y-1.5">
      {showUrl && !disabled && (
        <JobUrlImport
          onImport={(text, provenance) => {
            // Provenance-aware sink when the caller wants it; otherwise the plain
            // text setter (no provenance to persist).
            if (onImport) onImport(text, provenance);
            else onChange(text);
            setShowUrl(false);
          }}
          disabled={disabled}
        />
      )}
      <CollapsibleFileInput
        label={label}
        icon={Briefcase}
        value={value}
        onChange={onChange}
        uploading={uploading}
        onUpload={onUpload}
        accept=".pdf,.docx,.txt,.md,.markdown"
        placeholder={placeholder}
        disabled={disabled}
        uploadText={uploadText}
        textareaHeight={140}
        showCheckmark
        collapseLabel={t('common.collapse')}
        expandLabel={t('common.expand')}
        headerAction={
          <Button
            variant="info"
            onClick={() => setShowUrl((s) => !s)}
            className={cn(
              'flex items-center gap-1 rounded-md px-2 py-1 text-[10px] transition-colors h-auto',
              showUrl && 'border-brand/30 bg-brand/15 text-brand-soft'
            )}
          >
            <Link2 size={10} />
            {t('jobUrlImport.link')}
          </Button>
        }
      />
    </div>
  );
}
