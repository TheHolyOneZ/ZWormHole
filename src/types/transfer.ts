

export const TRANSFER_STARTED = "zw://transfer/started";
export const TRANSFER_ARCHIVING = "zw://transfer/archiving";
export const TRANSFER_PROGRESS = "zw://transfer/progress";
export const TRANSFER_COMPLETED = "zw://transfer/completed";
export const RECEIVE_COMPLETED = "zw://transfer/received";
export const RECEIVE_PROGRESS = "zw://transfer/receive-progress";
export const TRANSFER_FAILED = "zw://transfer/failed";
export const TRANSFER_CANCELLED = "zw://transfer/cancelled";


export interface ReceiveCompletedPayload {
  id: string;
  device_key: string;
  display_name: string;
  file_names: string[];
  paths: string[];
}


export interface ReceiveProgressPayload {
  id: string;
  device_key: string;
  display_name: string;
  label: string;
  bytes_done: number;
  bytes_total: number;
}


export interface LiveReceive {
  id: string;
  device_key: string;
  display_name: string;
  label: string;
  bytes_done: number;
  bytes_total: number;

  rate: number;

  history: number[];

  updated: number;
}

export type TransferStatus =
  | "queued"
  | "archiving"
  | "in_progress"
  | "paused"
  | "completed"
  | "failed"
  | "cancelled";

export interface TransferJob {
  id: string;
  device_fullname: string;

  label: string;
  status: TransferStatus;
  bytes_total: number;
  bytes_done: number;

  created_at: number;


  local_micros: number;
  encrypt_micros: number;
  socket_micros: number;


  peer_recv_wait_micros: number;
  peer_disk_write_micros: number;
}
