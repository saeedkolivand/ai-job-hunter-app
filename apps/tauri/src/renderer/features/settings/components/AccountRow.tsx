import { Check, Lock, Trash2 } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useState } from 'react';

import { Button, Input, useToast } from '@ajh/ui';

import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';
import { transition, variants } from '@/lib/motion';
import { useRemoveCredential, useSetCredential } from '@/services';
import type { CredEntry } from '@/types';

interface AccountRowProps {
  board: { id: string; name: string; hint: string };
  saved?: CredEntry;
  disabled: boolean;
  onSaved: () => void;
  onRemoved: () => void;
}

export function AccountRow({ board, saved, disabled, onSaved, onRemoved }: AccountRowProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const toast = useToast();
  const setCredential = useSetCredential();
  const removeCredential = useRemoveCredential();
  const busy = setCredential.isPending || removeCredential.isPending;

  const save = async () => {
    if (!username.trim() || !password) return;
    try {
      await setCredential.mutateAsync({ boardId: board.id, username: username.trim(), password });
      setPassword('');
      setUsername('');
      setOpen(false);
      onSaved();
      toast(`${board.name} credentials saved.`, 'success');
    } catch (err) {
      toast(
        err instanceof Error ? err.message : `Failed to save ${board.name} credentials.`,
        'error'
      );
    }
  };

  const remove = async () => {
    try {
      await removeCredential.mutateAsync(board.id);
      onRemoved();
      toast(`${board.name} credentials removed.`, 'success');
    } catch (err) {
      toast(
        err instanceof Error ? err.message : `Failed to remove ${board.name} credentials.`,
        'error'
      );
    }
  };

  return (
    <div className="glass-surface rounded-xl p-3">
      <div className="flex items-center gap-3">
        <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-white/5">
          <Lock size={14} className="text-brand-soft" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 text-sm font-medium">
            {board.name}
            {saved && (
              <span className="flex items-center gap-1 rounded-full border border-emerald-400/20 bg-emerald-400/5 px-2 py-0.5 text-[10px] text-emerald-200/90">
                <Check size={10} /> {t('settings.accounts.connected')}
              </span>
            )}
          </div>
          <div className="truncate text-xs text-foreground/45">
            {saved ? saved.username : board.hint}
          </div>
        </div>
        <div className="flex items-center gap-2">
          {saved && (
            <Button
              variant="ghost"
              size="sm"
              loading={busy}
              onClick={() => void remove()}
              className="text-foreground/70 hover:text-red-300"
            >
              <Trash2 size={12} /> {t('settings.accounts.remove')}
            </Button>
          )}
          <Button
            variant={disabled ? 'default' : 'glass'}
            size="sm"
            loading={busy}
            disabled={disabled}
            onClick={() => setOpen((o) => !o)}
          >
            {saved ? t('settings.accounts.update') : t('settings.accounts.connect')}
          </Button>
        </div>
      </div>

      <AnimatePresence>
        {open && (
          <motion.div
            {...variants.expand}
            transition={transition.normal}
            className="overflow-hidden"
          >
            <div className="mt-3 flex flex-col gap-2 border-t border-white/5 px-1 pt-3">
              <Input
                type="email"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder={t('settings.accounts.usernamePlaceholder')}
                autoComplete="off"
              />
              <Input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder={t('settings.accounts.passwordPlaceholder')}
                autoComplete="new-password"
              />
              <div className="flex items-center justify-between">
                <span className="text-[11px] text-foreground/40">
                  {t('settings.accounts.encryptedNote')}
                </span>
                <div className="flex gap-2">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => {
                      setOpen(false);
                      setPassword('');
                    }}
                  >
                    {t('settings.accounts.cancel')}
                  </Button>
                  <Button
                    variant="glass"
                    size="sm"
                    loading={busy}
                    disabled={!username.trim() || !password}
                    onClick={() => void save()}
                    className={cn(!username.trim() || !password ? '' : 'glow-subtle')}
                  >
                    {t('settings.accounts.save')}
                  </Button>
                </div>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
