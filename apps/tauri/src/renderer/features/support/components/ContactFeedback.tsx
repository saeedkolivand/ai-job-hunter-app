import { Copy } from 'lucide-react';
import { useState } from 'react';

import { Button, SelectDropdown, TextArea } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import {
  useCopyAppVersion,
  useCopyEnvironmentDetails,
  useCopySystemInfo,
  useExportDiagnostics,
} from '@/services';

import { DiagnosticExportItem } from './DiagnosticExportItem';

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
          {[
            { key: 'systemInformation', included: true },
            { key: 'logs', included: true },
            { key: 'enabledModels', included: true },
            { key: 'configuration', included: true },
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
          onClick={() => void exportDiagnostics.mutateAsync()}
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
            <SelectDropdown
              options={feedbackOptions}
              value={feedbackType}
              onChange={setFeedbackType}
            />
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
            onClick={() => void copyEnvDetails.mutateAsync()}
          >
            <Copy size={14} className="mr-2" />
            {t('support.contact.copyEnvironmentDetails')}
          </Button>
          <Button
            size="md"
            variant="ghost"
            className="w-full justify-start"
            loading={copyAppVersion.isPending}
            onClick={() => void copyAppVersion.mutateAsync()}
          >
            <Copy size={14} className="mr-2" />
            {t('support.contact.copyAppVersion')}
          </Button>
          <Button
            size="md"
            variant="ghost"
            className="w-full justify-start"
            loading={copySystemInfo.isPending}
            onClick={() => void copySystemInfo.mutateAsync()}
          >
            <Copy size={14} className="mr-2" />
            {t('support.contact.copySystemInfo')}
          </Button>
        </div>
      </div>
    </div>
  );
}
