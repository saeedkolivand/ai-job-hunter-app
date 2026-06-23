// ── Design utilities ──────────────────────────────────────────────────────
export { cn } from './lib/cn';
export * from './lib/motion';
export {
  applyTheme,
  applyThemeAnimated,
  type ColorScheme,
  DEFAULT_THEME_PREFS,
  getResolvedScheme,
  getThemePrefs,
  reapplySystemAccent,
  restoreTheme,
  type TextScale,
  type ThemePrefs,
} from './lib/theme';

// ── Primitives ────────────────────────────────────────────────────────────
export { Accordion, type AccordionProps } from './components/Accordion/index';
export {
  ActionMenu,
  type ActionMenuItem,
  type ActionMenuProps,
} from './components/ActionMenu/index';
export { ActionTile } from './components/ActionTile/index';
export { Alert, type AlertProps, type AlertType } from './components/Alert/index';
export { Button, type ButtonProps } from './components/Button/index';
export {
  CollapsibleFileInput,
  type CollapsibleFileInputProps,
} from './components/CollapsibleFileInput/index';
export { Dropdown, type DropdownOption, type DropdownProps } from './components/Dropdown/index';
export {
  type Control,
  Controller,
  type FieldErrors,
  type FieldValues,
  Form,
  FormField,
  type FormFieldProps,
  type FormProps,
  type Path,
  type RegisterOptions,
  type SubmitHandler,
  useFieldArray,
  type UseFieldArrayReturn,
  useForm,
  useFormContext,
  type UseFormReturn,
  useWatch,
} from './components/Form/index';
export { GlassCard, type GlassCardProps } from './components/GlassCard/index';
export { IconBadge, type IconBadgeProps } from './components/IconBadge/index';
export { IconText } from './components/IconText/index';
export { Image, type ImageProps } from './components/Image/index';
export { Input, type InputProps } from './components/Input/index';
export { LocationInput, type LocationInputProps } from './components/LocationInput/index';
export { NumberField, type NumberFieldProps } from './components/NumberField/index';
export { OptionalHint, type OptionalHintProps } from './components/OptionalHint/index';
export { ProgressBar, type ProgressBarProps } from './components/ProgressBar/index';
export { RefreshButton } from './components/RefreshButton/index';
export { SectionHeader } from './components/SectionHeader/index';
export { SectionLabel } from './components/SectionLabel/index';
export {
  SegmentedControl,
  type SegmentedControlProps,
  type SegmentedOption,
} from './components/SegmentedControl/index';
export { SetupHint, type SetupHintProps } from './components/SetupHint/index';
export { SourceBadge, type SourceBadgeProps } from './components/SourceBadge/index';
export { Switch, type SwitchProps } from './components/Switch/index';
export { type TabItem, Tabs, type TabsProps } from './components/Tabs/index';
export {
  type CheckableTagProps,
  Tag,
  type TagProps,
  type TagStatusColor,
} from './components/Tag/index';
export { TextArea, type TextAreaProps } from './components/TextArea/index';
export {
  Timeline,
  type TimelineColor,
  type TimelineItem,
  type TimelineProps,
} from './components/Timeline/index';

// ── Overlays & Modals ─────────────────────────────────────────────────────
export { ConfirmModal } from './components/ConfirmModal/index';
export { GlassOverlay } from './components/GlassOverlay/index';
export { HoverPopover, type HoverPopoverProps } from './components/HoverPopover/index';
export { ModalShell, type ModalShellProps } from './components/ModalShell/index';
export {
  type NotificationApi,
  type NotificationConfig,
  type NotificationPlacement,
  NotificationProvider,
  type NotificationVariant,
  useNotification,
} from './components/Notification/index';

// ── Feedback States ───────────────────────────────────────────────────────
export { EmptyState } from './components/EmptyState/index';
export { ErrorBoundary } from './components/ErrorBoundary/index';
export { ErrorState } from './components/ErrorState/index';
export { CardSkeleton, RowSkeleton, Skeleton } from './components/LoadingSkeleton/index';

// ── Content Rendering ─────────────────────────────────────────────────────
export { MarkdownMessage } from './components/MarkdownMessage/index';
export {
  ALLOWED_LINK_PROTOCOLS,
  buildEditorExtensions,
  docToMarkdown,
  getEditorSchema,
  isAllowedLinkUrl,
  joinPreserved,
  type LinkSuggestion,
  markdownToDoc,
  RichTextEditor,
  type RichTextEditorHandle,
  type RichTextEditorProps,
  roundTrip,
  type SplitPreserved,
  splitPreserved,
  type ToolbarLabels,
} from './components/RichTextEditor/index';
export { StreamingText } from './components/StreamingText/index';

// ── Composition Helpers ───────────────────────────────────────────────────
export { FloatingIcon } from './components/FloatingIcon/index';
export { NavPill, type NavPillProps } from './components/NavPill/index';
export { OptionTile } from './components/OptionTile/index';
export { SettingsSection } from './components/SettingsSection/index';
export { StepDots } from './components/StepDots/index';

// ── Hooks ─────────────────────────────────────────────────────────────────
export { useFocusTrap } from './hooks/use-focus-trap';
