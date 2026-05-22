/**
 * Thin compatibility wrapper — new code should use <Dropdown> from '@ajh/ui' directly.
 * Maps the old model-specific API (models/selectedModel/onSelectModel) to the generic one.
 */
import { Cpu } from 'lucide-react';

import { Dropdown } from '@ajh/ui';

import type { Model } from '@/types';

interface CustomDropdownProps {
  models: Model[];
  selectedModel: string;
  onSelectModel: (model: string) => void;
  searchable?: boolean;
  placeholder?: string;
}

export function CustomDropdown({
  models,
  selectedModel,
  onSelectModel,
  searchable,
  placeholder = 'Select a model…',
}: CustomDropdownProps) {
  return (
    <Dropdown
      options={models.map((m) => ({ value: m.name, label: m.name, meta: m.size }))}
      value={selectedModel}
      onChange={onSelectModel}
      placeholder={placeholder}
      icon={<Cpu size={13} />}
      searchable={searchable}
    />
  );
}
