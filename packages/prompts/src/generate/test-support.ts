import type { GenerationMeta } from './index';

export const RESUME_WITH_LINKS = `John Doe
Senior Engineer
Berlin, Germany | john@example.com

PROFESSIONAL SUMMARY
Built lots of things.

---
- [LinkedIn](https://linkedin.com/in/johndoe)
- [GitHub](https://github.com/johndoe)
- [Email](mailto:john@example.com)
- [Personal](https://not-a-profile.example.com)`;

export const META: GenerationMeta = {
  resumeLanguage: 'en',
  jobAdLanguage: 'en',
  mismatch: false,
  candidateName: 'John Doe',
  jobTitle: 'Senior Engineer',
  companyName: 'Acme',
  targetLanguage: 'en',
  topRequirements: ['React', 'TypeScript', 'AWS'],
};

export const RESUME_FOR_GROUNDING = `Jane Dev
Senior Engineer
jane@example.com

PROFESSIONAL SUMMARY
Backend engineer who ships React apps written in TypeScript.

WORK EXPERIENCE
Acme — Engineer (2020 - Present)
Built services in TypeScript and React with PostgreSQL.

SKILLS
React, TypeScript, PostgreSQL`;
