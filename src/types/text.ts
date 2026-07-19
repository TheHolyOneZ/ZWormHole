

export const TEXT_RECEIVED = "zw://text/received";

export interface TextReceivedPayload {
  device_key: string;
  display_name: string;
  text: string;
}
