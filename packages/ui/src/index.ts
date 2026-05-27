// ── Design utilities ──────────────────────────────────────────────────────
export { cn } from './lib/cn';
export * from './lib/motion';
export { applyTheme, getActiveTheme, restoreTheme, type ThemeId, THEMES } from './lib/theme';

// ── Primitives ────────────────────────────────────────────────────────────
export { Accordion, type AccordionProps } from './components/Accordion/index';
export { ActionTile } from './components/ActionTile/index';
export { Button, type ButtonProps } from './components/Button/index';
export {
  CollapsibleFileInput,
  type CollapsibleFileInputProps,
} from './components/CollapsibleFileInput/index';
export { Dropdown, type DropdownOption, type DropdownProps } from './components/Dropdown/index';
export { GlassCard, type GlassCardProps } from './components/GlassCard/index';
export { IconBadge, type IconBadgeProps } from './components/IconBadge/index';
export { IconText } from './components/IconText/index';
export { Input, type InputProps } from './components/Input/index';
export { LocationInput, type LocationInputProps } from './components/LocationInput/index';
export { ProgressBar, type ProgressBarProps } from './components/ProgressBar/index';
export { RefreshButton } from './components/RefreshButton/index';
export { SectionHeader } from './components/SectionHeader/index';
export { SectionLabel } from './components/SectionLabel/index';
export { SelectDropdown } from './components/SelectDropdown/index';
export { SourceBadge, type SourceBadgeProps } from './components/SourceBadge/index';
export { TextArea, type TextAreaProps } from './components/TextArea/index';

// ── Overlays & Modals ─────────────────────────────────────────────────────
export { ConfirmModal } from './components/ConfirmModal/index';
export { GlassOverlay } from './components/GlassOverlay/index';
export { ModalShell, type ModalShellProps } from './components/ModalShell/index';
export {
  type NotificationItem,
  NotificationProvider,
  type NotificationVariant,
  useNotification,
} from './components/Notification/index';
export {
  type ToastItem,
  ToastProvider,
  type ToastVariant,
  useToast,
} from './components/Toast/index';

// ── Feedback States ───────────────────────────────────────────────────────
export { EmptyState } from './components/EmptyState/index';
export { ErrorBoundary } from './components/ErrorBoundary/index';
export { ErrorState } from './components/ErrorState/index';
export { CardSkeleton, RowSkeleton, Skeleton } from './components/LoadingSkeleton/index';

// ── Content Rendering ─────────────────────────────────────────────────────
export { MarkdownMessage } from './components/MarkdownMessage/index';
export { StreamingText } from './components/StreamingText/index';

// ── Composition Helpers ───────────────────────────────────────────────────
export { FloatingIcon } from './components/FloatingIcon/index';
export { OptionTile } from './components/OptionTile/index';
export { SettingsSection } from './components/SettingsSection/index';
export { StepDots } from './components/StepDots/index';

// ── Hooks ─────────────────────────────────────────────────────────────────
export { useFocusTrap } from './hooks/use-focus-trap';
