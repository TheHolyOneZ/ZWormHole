

export interface HistoryEntry {
  id: string;
  device_key: string;
  display_name: string;
  file_names: string[];

  direction: string;
  total_bytes: number;

  at_unix: number;

  paths: string[];

  rate_samples: number[];
}
