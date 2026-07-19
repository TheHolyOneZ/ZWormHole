

export const CONNECTION_TEST_REQUESTED = "zw://conntest/requested";
export const CONNECTION_TEST_EXPIRED = "zw://conntest/expired";

export interface ConnectionTestRequestPayload {
  request_id: string;
  device_key: string;
  display_name: string;

  kind: string;
  total_bytes: number;
}
