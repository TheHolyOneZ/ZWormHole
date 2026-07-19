

export interface BenchSample {
  at_ms: number;
  mbps: number;
}

export interface BenchStats {
  bytes: number;
  elapsed_ms: number;
  min_mbps: number;
  max_mbps: number;
  avg_mbps: number;
  samples: BenchSample[];
}


export interface BenchLeg {
  sender: BenchStats;
  receiver: BenchStats;
}

export interface BenchmarkOutcome {
  total_bytes: number;

  upload: BenchLeg;

  download: BenchLeg;
}


export interface BenchHistoryEntry {
  id: string;
  fingerprint: string;
  display_name: string;
  at_unix: number;
  up_avg_mbps: number;
  down_avg_mbps: number;
}
