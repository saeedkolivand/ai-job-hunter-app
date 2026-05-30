import { Briefcase, Link2 } from 'lucide-react';
import { useState } from 'react';

import { Button, cn, CollapsibleFileInput } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

import { JobUrlImport } from '../JobUrlImport';

interface Props {
  label: string;
  value: string;
  onChange: (value: string) => void;
  uploading: boolean;
  onUpload: (file: File) => void;
  placeholder: string;
  uploadText: string;
  disabled?: boolean;
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
}: Props) {
  const { t } = useTranslation();
  const [showUrl, setShowUrl] = useState(false);

  return (
    <div className="space-y-1.5">
      {showUrl && !disabled && (
        <JobUrlImport
          onImport={(text) => {
            onChange(text);
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
        headerAction={
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setShowUrl((s) => !s)}
            className={cn(
              'flex items-center gap-1 rounded-md px-2 py-1 text-[10px] transition-colors h-auto',
              showUrl
                ? 'bg-brand/15 text-brand-soft'
                : 'bg-white/[0.04] text-foreground/50 hover:text-foreground/80'
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
