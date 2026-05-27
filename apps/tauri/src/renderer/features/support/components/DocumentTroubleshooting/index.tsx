import { useTranslation } from '@/lib/i18n';

import { DiagnosticItem } from './DiagnosticItem';
import { DocumentIssueCard } from './DocumentIssueCard';
import { IssueCard } from './IssueCard';

export function DocumentTroubleshooting() {
  const { t } = useTranslation();
  return (
    <div className="space-y-6">
      <div className="glass-card rounded-2xl p-6">
        <h2 className="text-lg font-semibold mb-4">
          {t('support.documentParsing.documentParsingStatus')}
        </h2>
        <div className="space-y-4">
          <DiagnosticItem
            name={t('support.documentParsing.pdfParser')}
            status="healthy"
            description={t('support.documentParsing.pdfParserDesc')}
          />
          <DiagnosticItem
            name={t('support.documentParsing.docxParser')}
            status="healthy"
            description={t('support.documentParsing.docxParserDesc')}
          />
          <DiagnosticItem
            name={t('support.documentParsing.ocrEngine')}
            status="healthy"
            description={t('support.documentParsing.ocrEngineDesc')}
          />
          <DiagnosticItem
            name={t('support.documentParsing.textExtraction')}
            status="healthy"
            description={t('support.documentParsing.textExtractionDesc')}
          />
        </div>
      </div>

      <div className="glass-card rounded-2xl p-6">
        <h2 className="text-lg font-semibold mb-4">
          {t('support.documentParsing.recentDocumentIssues')}
        </h2>
        <div className="space-y-3">
          <DocumentIssueCard
            filename="resume_2024.pdf"
            issue={t('support.documentParsing.ocrConfidenceLow')}
            status="warning"
            actions={[
              t('support.documentParsing.forceReOcr'),
              t('support.documentParsing.manualEdit'),
            ]}
          />
          <DocumentIssueCard
            filename="cover_letter.docx"
            issue={t('support.documentParsing.emptyExtraction')}
            status="error"
            actions={[
              t('support.documentParsing.retryParsing'),
              t('support.documentParsing.checkFileCorruption'),
            ]}
          />
          <DocumentIssueCard
            filename="job_post.pdf"
            issue={t('support.documentParsing.encodingDetectedAsBinary')}
            status="warning"
            actions={[
              t('support.documentParsing.reencodeAsUtf8'),
              t('support.documentParsing.useOcr'),
            ]}
          />
        </div>
      </div>

      <div className="glass-card rounded-2xl p-6">
        <h2 className="text-lg font-semibold mb-4">
          {t('support.documentParsing.commonDocumentIssues')}
        </h2>
        <div className="space-y-3">
          <IssueCard
            title={t('support.documentParsing.pdfParsingFailed')}
            solutions={[
              t('support.documentParsing.pdfParsingFailedSolution1'),
              t('support.documentParsing.pdfParsingFailedSolution2'),
              t('support.documentParsing.pdfParsingFailedSolution3'),
              t('support.documentParsing.pdfParsingFailedSolution4'),
            ]}
          />
          <IssueCard
            title={t('support.documentParsing.ocrConfidenceTooLow')}
            solutions={[
              t('support.documentParsing.ocrConfidenceTooLowSolution1'),
              t('support.documentParsing.ocrConfidenceTooLowSolution2'),
              t('support.documentParsing.ocrConfidenceTooLowSolution3'),
              t('support.documentParsing.ocrConfidenceTooLowSolution4'),
            ]}
          />
          <IssueCard
            title={t('support.documentParsing.emptyExtractionFromDocument')}
            solutions={[
              t('support.documentParsing.emptyExtractionFromDocumentSolution1'),
              t('support.documentParsing.emptyExtractionFromDocumentSolution2'),
              t('support.documentParsing.emptyExtractionFromDocumentSolution3'),
              t('support.documentParsing.emptyExtractionFromDocumentSolution4'),
            ]}
          />
        </div>
      </div>
    </div>
  );
}
