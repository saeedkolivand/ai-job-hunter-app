/**
 * The per-stage + advisor strings RESOLVE — executed against the real i18next
 * instance, in both locales.
 *
 * A missing key renders as the key itself (i18next's fallback), which no
 * component test with a stubbed `t` can catch: those assert on the key string
 * precisely because it is the key. This runs the real thing, including the
 * plural forms, whose `_one`/`_other` suffixes are resolved by the plural rule
 * rather than by a literal lookup.
 */
import { beforeAll, describe, expect, it } from 'vitest';

import i18n from '@ajh/translations';

import { OVERRIDABLE_PIPELINE_STAGES } from './stage-routing';

const LOCALES = ['en', 'de'] as const;

/** Every key the new surfaces render, dynamic ones expanded. */
const KEYS = [
  'settings.ai.stages.title',
  'settings.ai.stages.description',
  'settings.ai.stages.defaultModel',
  'settings.ai.stages.defaultNoModel',
  'settings.ai.stages.cliDefaultModel',
  'settings.ai.stages.change',
  'settings.ai.stages.changeClose',
  'settings.ai.stages.reset',
  'settings.ai.stages.noProvider',
  'settings.ai.stages.warnActiveUnconfigured',
  'settings.ai.stages.warnActiveNoModel',
  'settings.ai.stages.warnUnconfigured',
  'settings.ai.stages.warnNoModel',
  'settings.ai.stages.saveFailed',
  'settings.ai.stages.clearFailed',
  'settings.ai.stages.suggest.title',
  'settings.ai.stages.suggest.body',
  'settings.ai.stages.suggest.appliesTo',
  'settings.ai.stages.suggest.apply',
  'settings.ai.stages.suggest.dismiss',
  'settings.ai.stages.suggest.partial',
  'settings.ai.stages.editor.provider',
  'settings.ai.stages.editor.model',
  'settings.ai.stages.editor.modelPlaceholder',
  'settings.ai.stages.editor.modelCliDefault',
  'settings.ai.stages.editor.providerUnconfigured',
  'settings.ai.stages.editor.setWindow',
  'settings.ai.stages.editor.contextWindow',
  'settings.ai.stages.editor.windowDefault',
  'settings.ai.stages.editor.save',
  'settings.ai.stages.editor.cancel',
  'settings.ai.advisor.open',
  'settings.ai.advisor.title',
  'settings.ai.advisor.stepCounter',
  'settings.ai.advisor.close',
  'settings.ai.advisor.back',
  'settings.ai.advisor.next',
  'settings.ai.advisor.done',
  'settings.ai.advisor.models.title',
  'settings.ai.advisor.models.intro',
  'settings.ai.advisor.models.window',
  'settings.ai.advisor.models.active',
  'settings.ai.advisor.models.notMeasured',
  'settings.ai.advisor.models.emptyTitle',
  'settings.ai.advisor.models.emptyDescription',
  'settings.ai.advisor.fit.title',
  'settings.ai.advisor.fit.intro',
  'settings.ai.advisor.fit.usable',
  'settings.ai.advisor.fit.unknownHint',
  'settings.ai.advisor.fit.overflow',
  'settings.ai.advisor.fit.truncationNote',
  'settings.ai.advisor.fit.emptyTitle',
  'settings.ai.advisor.fit.emptyDescription',
  'settings.ai.advisor.fit.verdict.fits',
  'settings.ai.advisor.fit.verdict.tight',
  'settings.ai.advisor.fit.verdict.too-small',
  'settings.ai.advisor.fit.verdict.unknown',
  'settings.ai.advisor.recommend.title',
  'settings.ai.advisor.recommend.stagesHeading',
  'settings.ai.advisor.recommend.stagesBody',
  'settings.ai.advisor.recommend.stagesNothing',
  'settings.ai.advisor.recommend.appHeading',
  'settings.ai.advisor.recommend.appBody',
  'settings.ai.advisor.recommend.serverHeading',
  'settings.ai.advisor.recommend.serverBody',
  'settings.ai.advisor.recommend.serverDisclaimer',
  'settings.ai.advisor.risk.title',
  'settings.ai.advisor.risk.intro',
  'settings.ai.advisor.risk.badCombo',
  'settings.ai.advisor.risk.measured',
  'settings.ai.advisor.risk.measuredNoOutput',
  'settings.ai.advisor.risk.measuredLight',
  'settings.ai.advisor.risk.reasoningHeavy',
  'settings.ai.advisor.risk.unmeasured',
  'settings.ai.advisor.risk.emptyTitle',
  'settings.ai.advisor.risk.emptyDescription',
  'settings.ai.advisor.risk.level.known-bad-combo',
  'settings.ai.advisor.risk.level.measured-heavy',
  'settings.ai.advisor.risk.level.measured-light',
  'settings.ai.advisor.risk.level.reasoning-heavy',
  'settings.ai.advisor.risk.level.unmeasured',
  'settings.ai.advisor.risk.coverageNote',
  ...OVERRIDABLE_PIPELINE_STAGES.map((s) => `settings.ai.stages.names.${s}`),
  ...OVERRIDABLE_PIPELINE_STAGES.map((s) => `settings.ai.stages.descriptions.${s}`),
];

describe.each(LOCALES)('per-stage + advisor strings — %s', (locale) => {
  beforeAll(async () => {
    await i18n.changeLanguage(locale);
  });

  it('resolves every key to real copy', () => {
    const missing = KEYS.filter((key) => i18n.t(key) === key || i18n.t(key) === '');
    expect(missing).toEqual([]);
  });

  it('resolves both plural forms of the applied message', () => {
    const one = i18n.t('settings.ai.stages.suggest.applied', { count: 1, model: 'm' });
    const many = i18n.t('settings.ai.stages.suggest.applied', { count: 3, model: 'm' });

    expect(one).not.toContain('applied');
    expect(many).not.toContain('applied');
    // The forms must actually DIFFER — a single string for both is the bug the
    // plural keys exist to prevent.
    expect(one).not.toBe(many);
    expect(many).toContain('3');
  });

  it('interpolates rather than leaving placeholders behind', () => {
    const rendered = i18n.t('settings.ai.stages.suggest.partial', {
      done: 1,
      total: 3,
      reason: 'nope',
    });

    expect(rendered).not.toContain('{{');
    expect(rendered).toContain('nope');
  });
});
