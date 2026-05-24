import { AlertTriangle } from 'lucide-react';
import { Component, type ErrorInfo, type ReactNode } from 'react';

import { RefreshButton } from './RefreshButton';

interface Props {
  children: ReactNode;
  /** Custom fallback — receives error and reset handler */
  fallback?: (error: Error, reset: () => void) => ReactNode;
}

interface State {
  error: Error | null;
}

/**
 * Catches render errors in the subtree and shows a recovery UI.
 * Wrap feature pages or heavy panels individually to isolate failures.
 *
 * Usage:
 *   <ErrorBoundary>
 *     <FeaturePage />
 *   </ErrorBoundary>
 */
export class ErrorBoundary extends Component<Props, State> {
  override state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  override componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('[ErrorBoundary]', error, info.componentStack);
  }

  reset = () => this.setState({ error: null });

  override render() {
    const { error } = this.state;
    if (!error) return this.props.children;

    if (this.props.fallback) return this.props.fallback(error, this.reset);

    return <ErrorBoundaryFallback onReset={this.reset} error={error} />;
  }
}

function ErrorBoundaryFallback({ onReset, error }: { onReset: () => void; error: Error }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-4 p-10 text-center">
      <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-red-500/10 ring-1 ring-red-500/20">
        <AlertTriangle size={24} className="text-red-400/70" />
      </div>
      <div className="space-y-1">
        <p className="text-sm font-medium text-foreground/70">Something went wrong</p>
        <p className="max-w-xs text-xs text-foreground/40">
          {error.message || 'An unexpected error occurred in this panel.'}
        </p>
      </div>
      <RefreshButton onRefresh={onReset} size={13}>
        Try again
      </RefreshButton>
    </div>
  );
}
