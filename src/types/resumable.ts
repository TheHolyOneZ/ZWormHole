import type { TransferStatus } from "./transfer";


export type PersistedSource =
  | { kind: "files"; paths: string[] }
  | { kind: "folder"; folder_path: string; selected_files: string[] | null };


export interface PersistedJob {
  id: string;
  device_key: string;
  source: PersistedSource;
  status: TransferStatus;
  bytes_total: number;
  updated_at: number;
}
