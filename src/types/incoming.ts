

import type { AutoExtractMode } from "./settings";

export const INCOMING_TRANSFER_REQUEST = "zw://transfer/incoming-request";
export const INCOMING_TRANSFER_EXPIRED = "zw://transfer/incoming-expired";
export const TRANSFER_REJECTED = "zw://transfer/rejected";

export interface IncomingFileInfo {
  name: string;
  size: number;
}

export interface IncomingTransferRequestPayload {
  request_id: string;
  device_key: string;
  display_name: string;
  file_names: string[];
  files: IncomingFileInfo[];
  total_size: number;
  default_save_dir: string;
  folder_archive_name: string | null;
  default_extract_mode: AutoExtractMode;
}
