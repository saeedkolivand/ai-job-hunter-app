export interface Posting {
  id: string;
  source: string;
  externalId: string;
  url: string;
  title: string;
  company: string;
  location?: string;
  remote?: boolean;
  description: string;
  postedAt?: number;
  capturedAt: number;
  interactions?: { interactionType: string }[];
}

export interface ApplyStep {
  ts: number;
  stage: string;
  ok: boolean;
  note?: string;
  kind: 'step' | 'progress';
  p?: number;
}
