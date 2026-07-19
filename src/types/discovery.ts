

export const DEVICE_APPEARED = "zw://discovery/device-appeared";
export const DEVICE_UPDATED = "zw://discovery/device-updated";
export const DEVICE_LOST = "zw://discovery/device-lost";

export interface NearbyDevicePayload {
  fullname: string;
  hostname: string;
  session_id: string;
  fingerprint: string;
  protocol_version: string;
  os: string;
  addresses: string[];
  port: number;
}

export interface DeviceLostPayload {
  fullname: string;
}
