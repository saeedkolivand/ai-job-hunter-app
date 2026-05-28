export interface JobRecord {
  id: string;
  kind: string;
  status: 'queued' | 'running' | 'streaming' | 'completed' | 'failed' | 'cancelled';
  progress: number;
  createdAt: number;
  updatedAt: number;
  finishedAt?: number;
}

export interface ActivityItem {
  id: string;
  time: number;
  text: string;
  tone: 'violet' | 'indigo' | 'blue' | 'emerald' | 'amber';
}
