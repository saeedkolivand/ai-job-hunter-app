import { Copy } from 'lucide-react';
import { useState } from 'react';
import { save } from '@tauri-apps/plugin-dialog';
import { revealItemInDir } from '@tauri-apps/plugin-opener';

import { useTranslation } from '@ajh/translations';
import { Button, Dropdown, TextArea, useNotification } from '@ajh/ui';

import {
  useCopyAppVersion,
  useCopyEnvironmentDetails,
  useCopySystemInfo,
  useExportDiagnostics,
} from '@/services';

import { DiagnosticExportItem } from '../DiagnosticExportItem';

const FEEDBACK_TYPES = [
  'bugReport',
  'featureRequest',
  'generalFeedback',
  'performanceIssue',
  'other',
] as const;

export function ContactFeedback() {
  const { t } = useTranslation();
  const [feedbackType, setFeedbackType] = useState<string>('bugReport');
  const [description, setDescription] = useState('');

  const notify = useNotification();
  const exportDiagnostics = useExportDiagnostics();
  const copyEnvDetails = useCopyEnvironmentDetails();
  const copyAppVersion = useCopyAppVersion();
  const copySystemInfo = useCopySystemInfo();

  const feedbackOptions = FEEDBACK_TYPES.map((key) => ({
    value: key,
    label: t(`support.contact.${key}`),
  }));

  return (
    <div className="space-y-6">
      <div className="glass-card rounded-2xl p-6">
        <h2 className="mb-4 text-lg font-semibold">
          {t('support.contact.exportDiagnosticsBundle')}
        </h2>
        <p className="mb-4 text-sm text-foreground/55">
          {t('support.contact.exportDiagnosticsBundleDesc')}
        </p>
        <div className="space-y-3">
          {/* SECURITY: Only list items the Rust build_diagnostics_zip command actually emits
              (system-info.txt, crashes.log, logs/). Any entry marked included:true MUST be
              redaction-safe and MUST appear in that allowlist — never add config or API-key
              bearing files here, even as a future convenience. */}
          {[
            { key: 'systemInformation', included: true },
            { key: 'crashLog', included: true },
            { key: 'logs', included: true },
            { key: 'documentContents', included: false },
            { key: 'personalData', included: false },
          ].map(({ key, included }) => (
            <DiagnosticExportItem
              key={key}
              name={t(`support.contact.${key}`)}
              included={included}
              description={t(`support.contact.${key}Desc`)}
            />
          ))}
        </div>
        <Button
          size="md"
          variant="glass"
          className="mt-4"
          loading={exportDiagnostics.isPending}
          onClick={async () => {
            const now = new Date();
            const yyyy = now.getFullYear();
            const mm = String(now.getMonth() + 1).padStart(2, '0');
            const dd = String(now.getDate()).padStart(2, '0');
            const defaultPath = `ajh-diagnostics-${yyyy}-${mm}-${dd}.zip`;
            try {
              const dest = await save({
                defaultPath,
                filters: [{ name: 'Zip archive', extensions: ['zip'] }],
              });
              if (!dest) return;
              const res = await exportDiagnostics.mutateAsync(dest);
              if (res.success) {
                notify.success({ message: t('support.contact.exportBundleSaved') });
                revealItemInDir(dest).catch(() => {});
              } else {
                notify.error({ message: t('support.contact.exportBundleError') });
              }
            } catch (err) {
              console.error(
                'diagnostics export failed:',
                err instanceof Error ? err.name : 'unknown'
              );
              notify.error({ message: t('support.contact.exportBundleError') });
            }
          }}
        >
          {t('support.contact.exportBundle')}
        </Button>
      </div>

      <div className="glass-card rounded-2xl p-6">
        <h2 className="mb-4 text-lg font-semibold">{t('support.contact.submitFeedback')}</h2>
        <div className="space-y-4">
          <div>
            <label className="mb-2 block text-sm font-medium text-foreground/90">
              {t('support.contact.feedbackType')}
            </label>
            <Dropdown options={feedbackOptions} value={feedbackType} onChange={setFeedbackType} />
          </div>
          <div>
            <label className="mb-2 block text-sm font-medium text-foreground/90">
              {t('support.contact.description')}
            </label>
            <TextArea
              variant="glass"
              placeholder={t('support.contact.descriptionPlaceholder')}
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              className="h-32"
            />
          </div>
          <Button size="md" variant="glass">
            {t('support.contact.submit')}
          </Button>
        </div>
      </div>

      <div className="glass-card rounded-2xl p-6">
        <h2 className="mb-4 text-lg font-semibold">{t('support.contact.quickActions')}</h2>
        <div className="space-y-3">
          <Button
            size="md"
            variant="ghost"
            className="w-full justify-start"
            loading={copyEnvDetails.isPending}
            onClick={async () => {
              await copyEnvDetails.mutateAsync();
              notify.success({ message: 'Copied to clipboard.' });
            }}
          >
            <Copy size={14} className="mr-2" />
            {t('support.contact.copyEnvironmentDetails')}
          </Button>
          <Button
            size="md"
            variant="ghost"
            className="w-full justify-start"
            loading={copyAppVersion.isPending}
            onClick={async () => {
              await copyAppVersion.mutateAsync();
              notify.success({ message: 'Copied to clipboard.' });
            }}
          >
            <Copy size={14} className="mr-2" />
            {t('support.contact.copyAppVersion')}
          </Button>
          <Button
            size="md"
            variant="ghost"
            className="w-full justify-start"
            loading={copySystemInfo.isPending}
            onClick={async () => {
              await copySystemInfo.mutateAsync();
              notify.success({ message: 'Copied to clipboard.' });
            }}
          >
            <Copy size={14} className="mr-2" />
            {t('support.contact.copySystemInfo')}
          </Button>
        </div>
      </div>
    </div>
  );
}
