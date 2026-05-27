// ── Design utilities ──────────────────────────────────────────────────────
export { cn } from './lib/cn';
export * from './lib/motion';
export { applyTheme, getActiveTheme, restoreTheme, type ThemeId, THEMES } from './lib/theme';

// ── Primitives ────────────────────────────────────────────────────────────
export { ActionTile } from './components/ActionTile';
export { Button, type ButtonProps } from './components/Button';
export {
  CollapsibleFileInput,
  type CollapsibleFileInputProps,
} from './components/CollapsibleFileInput';
export { Dropdown, type DropdownOption, type DropdownProps } from './components/Dropdown';
export { GlassCard, type GlassCardProps } from './components/GlassCard';
export { IconBadge, type IconBadgeProps } from './components/IconBadge';
export { IconText } from './components/IconText';
export { Input, type InputProps } from './components/Input';
export { LocationInput, type LocationInputProps } from './components/LocationInput';
export { ProgressBar, type ProgressBarProps } from './components/ProgressBar';
export { RefreshButton } from './components/RefreshButton';
export { SectionHeader } from './components/SectionHeader';
export { SectionLabel } from './components/SectionLabel';
export { SelectDropdown } from './components/SelectDropdown';
export { SourceBadge, type SourceBadgeProps } from './components/SourceBadge';
export { TextArea, type TextAreaProps } from './components/TextArea';

// ── Overlays & Modals ─────────────────────────────────────────────────────
export { ConfirmModal } from './components/ConfirmModal';
export { GlassOverlay } from './components/GlassOverlay';
export { ModalShell, type ModalShellProps } from './components/ModalShell';
export {
  type NotificationItem,
  NotificationProvider,
  type NotificationVariant,
  useNotification,
} from './components/Notification';
export { type ToastItem, ToastProvider, type ToastVariant, useToast } from './components/Toast';

// ── Feedback States ───────────────────────────────────────────────────────
export { EmptyState } from './components/EmptyState';
export { ErrorBoundary } from './components/ErrorBoundary';
export { ErrorState } from './components/ErrorState';
export { CardSkeleton, RowSkeleton, Skeleton } from './components/LoadingSkeleton';

// ── Content Rendering ─────────────────────────────────────────────────────
export { MarkdownMessage } from './components/MarkdownMessage';
export { StreamingText } from './components/StreamingText';

// ── Composition Helpers ───────────────────────────────────────────────────
export { FloatingIcon } from './components/FloatingIcon';
export { OptionTile } from './components/OptionTile';
export { SettingsSection } from './components/SettingsSection';
export { StepDots } from './components/StepDots';

// ── Hooks ─────────────────────────────────────────────────────────────────
export { useFocusTrap } from './hooks/use-focus-trap';
