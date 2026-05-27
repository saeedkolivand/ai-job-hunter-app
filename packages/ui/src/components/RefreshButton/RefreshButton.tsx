import { RefreshCw } from 'lucide-react';
import { useState } from 'react';

import { Button } from '../Button';

interface RefreshButtonProps {
  onRefresh: () => void | Promise<void>;
  size?: number;
  variant?: 'ghost' | 'glass';
  disabled?: boolean;
  className?: string;
  title?: string;
  children?: React.ReactNode;
}

/**
 * Reusable refresh button with spinning animation during async operations.
 * Handles local loading state and disables during refresh.
 */
export function RefreshButton({
  onRefresh,
  size = 12,
  variant = 'ghost',
  disabled,
  className,
  title,
  children,
}: RefreshButtonProps) {
  const [refreshing, setRefreshing] = useState(false);

  const handleRefresh = async () => {
    if (refreshing || disabled) return;
    setRefreshing(true);
    try {
      await onRefresh();
    } finally {
      setRefreshing(false);
    }
  };

  return (
    <Button
      variant={variant}
      size="sm"
      onClick={handleRefresh}
      disabled={disabled || refreshing}
      className={className}
      title={title}
    >
      <RefreshCw size={size} className={refreshing ? 'animate-spin' : ''} />
      {children}
    </Button>
  );
}
