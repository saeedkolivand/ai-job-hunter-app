export type ClusterColor =
  | 'client'
  | 'hooks'
  | 'prompts'
  | 'contract'
  | 'command'
  | 'domain'
  | 'scraper'
  | 'provider'
  | 'data'
  | 'external'
  | 'infra';

export type EdgeKind = 'critical' | 'normal' | 'db' | 'api' | 'mount';

export interface Cluster {
  id: string;
  label: string;
  x: number;
  y: number;
  w: number;
  h: number;
  color: ClusterColor;
}

export interface MapNode {
  id: string;
  cluster: string;
  label: string;
  sub: string;
  x: number;
  y: number;
  w: number;
  h: number;
  color: ClusterColor;
  role: string;
  plain: string;
  path: string;
  notes: string[];
  tag: string[];
  critical?: boolean;
}

export interface MapEdge {
  from: string;
  to: string;
  kind: EdgeKind;
  label?: string;
  tag: string[];
}

export interface Fix {
  n: number;
  t: string;
}

export interface Bug {
  sev: string;
  ref: string;
  t: string;
}
