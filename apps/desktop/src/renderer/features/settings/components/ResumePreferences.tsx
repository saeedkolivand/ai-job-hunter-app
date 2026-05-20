import { useState, useRef } from 'react';
import { motion } from 'motion/react';
import { FileText, Upload, Check, AlertCircle, Sparkles, Trash2 } from 'lucide-react';
import { GlassCard } from '@/components/ui/GlassCard';
import { Button } from '@/components/ui/Button';
import { usePreferencesStore, useResume } from '@/store/preferences-store';
import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';

interface Resume {
  id: string;
  name: string;
  uploadedAt: string;
  size: number;
  indexed: boolean;
  skillsExtracted: string[];
  atsScore?: number;
}

export function ResumePreferences() {
  const { t } = useTranslation();
  const resume = useResume();
  const setResume = usePreferencesStore((state) => state.setResume);
  const [resumes, setResumes] = useState<Resume[]>([]);
  const [uploading, setUploading] = useState(false);
  const [dragActive, setDragActive] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleFileUpload = async (file: File) => {
    setUploading(true);
    // Simulate upload and processing
    await new Promise((resolve) => setTimeout(resolve, 1500));

    const newResume: Resume = {
      id: Date.now().toString(),
      name: file.name,
      uploadedAt: new Date().toISOString(),
      size: file.size,
      indexed: true,
      skillsExtracted: ['JavaScript', 'TypeScript', 'React', 'Node.js'],
      atsScore: 85,
    };

    setResumes([...resumes, newResume]);
    if (!resume?.defaultId) {
      setResume({ defaultId: newResume.id, autoIndex: true, autoParse: true });
    }
    setUploading(false);
  };

  const handleDrag = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.type === 'dragenter' || e.type === 'dragover') {
      setDragActive(true);
    } else if (e.type === 'dragleave') {
      setDragActive(false);
    }
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setDragActive(false);

    if (e.dataTransfer.files && e.dataTransfer.files[0]) {
      handleFileUpload(e.dataTransfer.files[0]);
    }
  };

  const handleSetDefault = (id: string) => {
    setResume({
      defaultId: id,
      autoIndex: resume?.autoIndex ?? true,
      autoParse: resume?.autoParse ?? true,
    });
  };

  const handleDelete = (id: string) => {
    setResumes(resumes.filter((r) => r.id !== id));
    if (resume?.defaultId === id) {
      setResume({
        defaultId: resumes.find((r) => r.id !== id)?.id,
        autoIndex: resume?.autoIndex ?? true,
        autoParse: resume?.autoParse ?? true,
      });
    }
  };

  const formatFileSize = (bytes: number) => {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
    return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
  };

  return (
    <GlassCard>
      <div className="mb-4 flex items-center justify-between">
        <div className="text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
          {t('settings.resume.title')}
        </div>
      </div>

      <p className="mb-4 text-sm text-foreground/55">{t('settings.resume.description')}</p>

      {/* Upload Area */}
      <div
        onDragEnter={handleDrag}
        onDragLeave={handleDrag}
        onDragOver={handleDrag}
        onDrop={handleDrop}
        onClick={() => fileInputRef.current?.click()}
        className={cn(
          'relative mb-6 flex cursor-pointer flex-col items-center justify-center rounded-xl border-2 border-dashed p-8 transition-all',
          dragActive
            ? 'border-brand-soft/50 bg-brand-soft/5'
            : 'border-white/10 bg-white/5 hover:border-white/20 hover:bg-white/10'
        )}
      >
        <input
          ref={fileInputRef}
          type="file"
          accept=".pdf,.doc,.docx"
          className="hidden"
          onChange={(e) => e.target.files?.[0] && handleFileUpload(e.target.files[0])}
        />

        {uploading ? (
          <div className="flex flex-col items-center gap-3">
            <motion.div animate={{ rotate: 360 }} transition={transition.spin}>
              <Upload size={32} className="text-brand-soft" />
            </motion.div>
            <div className="text-sm text-foreground/60">{t('settings.resume.uploading')}</div>
          </div>
        ) : (
          <div className="flex flex-col items-center gap-3">
            <div
              className={cn(
                'rounded-full p-3 transition-colors',
                dragActive ? 'bg-brand-soft/20' : 'bg-white/5'
              )}
            >
              <Upload
                size={24}
                className={cn(
                  'transition-colors',
                  dragActive ? 'text-brand-soft' : 'text-foreground/40'
                )}
              />
            </div>
            <div className="text-sm text-foreground/70">{t('settings.resume.dragDrop')}</div>
            <div className="text-xs text-foreground/40">
              {t('settings.resume.orClick')} ({t('settings.resume.fileTypes')})
            </div>
          </div>
        )}
      </div>

      {/* Resume List */}
      <div className="space-y-3">
        {resumes.map((resumeItem) => {
          const isDefault = resume?.defaultId === resumeItem.id;

          return (
            <motion.div
              key={resumeItem.id}
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              className={cn(
                'flex items-start gap-4 rounded-xl border p-4 transition-all',
                isDefault
                  ? 'border-brand-soft/50 bg-brand-soft/10 glow-subtle'
                  : 'border-white/10 bg-white/5'
              )}
            >
              <div
                className={cn(
                  'rounded-lg p-2 transition-colors',
                  isDefault ? 'bg-brand-soft/20' : 'bg-white/5'
                )}
              >
                <FileText
                  size={20}
                  className={cn(
                    'transition-colors',
                    isDefault ? 'text-brand-soft' : 'text-foreground/40'
                  )}
                />
              </div>

              <div className="flex-1 min-w-0">
                <div className="flex items-start justify-between gap-2 mb-2">
                  <div className="flex-1 min-w-0">
                    <div
                      className={cn(
                        'text-sm font-medium truncate transition-colors',
                        isDefault ? 'text-foreground' : 'text-foreground/70'
                      )}
                    >
                      {resumeItem.name}
                    </div>
                    <div className="text-xs text-foreground/40 mt-0.5">
                      {formatFileSize(resumeItem.size)} •{' '}
                      {new Date(resumeItem.uploadedAt).toLocaleDateString()}
                    </div>
                  </div>

                  <div className="flex items-center gap-1">
                    {resumeItem.indexed && (
                      <div className="flex items-center gap-1 rounded-full bg-green-500/20 px-2 py-0.5 text-xs text-green-400">
                        <Check size={10} />
                        {t('settings.resume.indexed')}
                      </div>
                    )}
                    {isDefault && (
                      <div className="flex items-center gap-1 rounded-full bg-brand-soft/20 px-2 py-0.5 text-xs text-brand-soft">
                        <Sparkles size={10} />
                        {t('settings.resume.default')}
                      </div>
                    )}
                  </div>
                </div>

                {/* Skills Preview */}
                {resumeItem.skillsExtracted.length > 0 && (
                  <div className="mb-2">
                    <div className="text-xs font-medium uppercase tracking-[0.16em] text-foreground/40 mb-1.5">
                      {t('settings.resume.extractedSkills')}
                    </div>
                    <div className="flex flex-wrap gap-1.5">
                      {resumeItem.skillsExtracted.slice(0, 4).map((skill) => (
                        <span
                          key={skill}
                          className="rounded-full bg-white/5 px-2 py-0.5 text-xs text-foreground/60"
                        >
                          {skill}
                        </span>
                      ))}
                      {resumeItem.skillsExtracted.length > 4 && (
                        <span className="rounded-full bg-white/5 px-2 py-0.5 text-xs text-foreground/40">
                          +{resumeItem.skillsExtracted.length - 4} {t('settings.resume.more')}
                        </span>
                      )}
                    </div>
                  </div>
                )}

                {/* ATS Score */}
                {resumeItem.atsScore && (
                  <div className="flex items-center gap-2">
                    <div className="text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
                      {t('settings.resume.atsScore')}
                    </div>
                    <div
                      className={cn(
                        'text-xs font-medium',
                        resumeItem.atsScore >= 80
                          ? 'text-green-400'
                          : resumeItem.atsScore >= 60
                            ? 'text-yellow-400'
                            : 'text-red-400'
                      )}
                    >
                      {resumeItem.atsScore}%
                    </div>
                  </div>
                )}
              </div>

              <div className="flex items-center gap-2">
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => handleSetDefault(resumeItem.id)}
                  className="!bg-transparent hover:bg-white/5"
                  disabled={isDefault}
                >
                  <Sparkles size={14} />
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => handleDelete(resumeItem.id)}
                  className="!bg-transparent hover:bg-red-500/10 hover:text-red-400"
                >
                  <Trash2 size={14} />
                </Button>
              </div>
            </motion.div>
          );
        })}

        {resumes.length === 0 && (
          <div className="flex flex-col items-center gap-3 py-8 text-center">
            <AlertCircle size={32} className="text-foreground/20" />
            <div className="text-sm text-foreground/40">{t('settings.resume.noResumes')}</div>
          </div>
        )}
      </div>
    </GlassCard>
  );
}
