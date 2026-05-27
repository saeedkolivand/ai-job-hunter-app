import { CollapsibleFileInput } from '@ajh/ui';

const ACCEPT_ATTR = '.pdf,.docx,.txt,.md,.markdown';

interface FileInputProps {
  label: string;
  icon: React.ElementType;
  value: string;
  onChange: (v: string) => void;
  uploading: boolean;
  onUpload: (file: File) => void;
  disabled?: boolean;
  t: (key: string) => string;
}

export function FileInput({
  label,
  icon,
  value,
  onChange,
  uploading,
  onUpload,
  disabled,
  t,
}: FileInputProps) {
  return (
    <CollapsibleFileInput
      label={label}
      icon={icon}
      value={value}
      onChange={onChange}
      uploading={uploading}
      onUpload={onUpload}
      accept={ACCEPT_ATTR}
      placeholder={t('aiGenerate.placeholder').replace('…', '')}
      disabled={disabled}
      uploadText={t('aiGenerate.upload')}
      textareaHeight={140}
      showCheckmark
    />
  );
}
