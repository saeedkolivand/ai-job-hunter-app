import { CollapsibleFileInput } from '@ajh/ui';

const ACCEPT_ATTR = '.pdf,.docx,.txt,.md,.markdown';

interface CollapsibleInputProps {
  label: string;
  icon: React.ElementType;
  value: string;
  onChange: (v: string) => void;
  uploading: boolean;
  onUpload: (f: File) => void;
  placeholder: string;
  disabled?: boolean;
  t: (key: string) => string;
}

export function CollapsibleInput({
  label,
  icon,
  value,
  onChange,
  uploading,
  onUpload,
  placeholder,
  disabled,
  t,
}: CollapsibleInputProps) {
  return (
    <CollapsibleFileInput
      label={label}
      icon={icon}
      value={value}
      onChange={onChange}
      uploading={uploading}
      onUpload={onUpload}
      accept={ACCEPT_ATTR}
      placeholder={placeholder}
      disabled={disabled}
      uploadText={t('analyze.uploadButton')}
      textareaHeight={140}
      showCheckmark
    />
  );
}
