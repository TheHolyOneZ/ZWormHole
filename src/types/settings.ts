

export type AutoExtractMode = "off" | "extract_and_delete" | "extract_and_keep";

export interface TransferSettingsPayload {
  auto_accept: boolean;
  save_dir: string | null;
  default_save_dir: string | null;
  auto_extract: AutoExtractMode;
  max_send_mb_s: number;
  max_concurrent_sends: number;

  quic_port: number;
}
