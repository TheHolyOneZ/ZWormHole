

export const PAIRING_REQUESTED = "zw://pairing/requested";
export const PAIRING_SUCCEEDED = "zw://pairing/succeeded";
export const PAIRING_FAILED = "zw://pairing/failed";
export const PEER_DISCONNECTED = "zw://pairing/peer-disconnected";

export interface PairingRequestedPayload {
  request_id: string;
  requester_name: string;
  code: string;
  requester_fingerprint: string;
}

export interface PairingSucceededPayload {
  device_key: string;
  display_name: string;
}

export interface PairingFailedPayload {
  device_key: string;
  reason: string;
}
