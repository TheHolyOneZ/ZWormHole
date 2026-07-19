

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { PING_PONG, type PongPayload } from "../types/events";
import {
  DEVICE_APPEARED,
  DEVICE_LOST,
  DEVICE_UPDATED,
  type DeviceLostPayload,
  type NearbyDevicePayload,
} from "../types/discovery";
import {
  PAIRING_FAILED,
  PAIRING_REQUESTED,
  PAIRING_SUCCEEDED,
  PEER_DISCONNECTED,
  type PairingFailedPayload,
  type PairingRequestedPayload,
  type PairingSucceededPayload,
} from "../types/pairing";
import {
  RECEIVE_COMPLETED,
  RECEIVE_PROGRESS,
  TRANSFER_ARCHIVING,
  TRANSFER_CANCELLED,
  TRANSFER_COMPLETED,
  TRANSFER_FAILED,
  TRANSFER_PROGRESS,
  TRANSFER_STARTED,
  type ReceiveCompletedPayload,
  type ReceiveProgressPayload,
} from "../types/transfer";
import {
  INCOMING_TRANSFER_EXPIRED,
  INCOMING_TRANSFER_REQUEST,
  TRANSFER_REJECTED,
  type IncomingTransferRequestPayload,
} from "../types/incoming";
import {
  CONNECTION_TEST_EXPIRED,
  CONNECTION_TEST_REQUESTED,
  type ConnectionTestRequestPayload,
} from "../types/conntest";
import { TEXT_RECEIVED, type TextReceivedPayload } from "../types/text";

export function onPong(
  handler: (payload: PongPayload) => void,
): Promise<UnlistenFn> {
  return listen<PongPayload>(PING_PONG, (event) => handler(event.payload));
}

export function onDeviceAppeared(
  handler: (payload: NearbyDevicePayload) => void,
): Promise<UnlistenFn> {
  return listen<NearbyDevicePayload>(DEVICE_APPEARED, (event) =>
    handler(event.payload),
  );
}

export function onDeviceUpdated(
  handler: (payload: NearbyDevicePayload) => void,
): Promise<UnlistenFn> {
  return listen<NearbyDevicePayload>(DEVICE_UPDATED, (event) =>
    handler(event.payload),
  );
}

export function onDeviceLost(
  handler: (payload: DeviceLostPayload) => void,
): Promise<UnlistenFn> {
  return listen<DeviceLostPayload>(DEVICE_LOST, (event) =>
    handler(event.payload),
  );
}

export function onPairingRequested(
  handler: (payload: PairingRequestedPayload) => void,
): Promise<UnlistenFn> {
  return listen<PairingRequestedPayload>(PAIRING_REQUESTED, (event) =>
    handler(event.payload),
  );
}

export function onPairingSucceeded(
  handler: (payload: PairingSucceededPayload) => void,
): Promise<UnlistenFn> {
  return listen<PairingSucceededPayload>(PAIRING_SUCCEEDED, (event) =>
    handler(event.payload),
  );
}

export function onPairingFailed(
  handler: (payload: PairingFailedPayload) => void,
): Promise<UnlistenFn> {
  return listen<PairingFailedPayload>(PAIRING_FAILED, (event) =>
    handler(event.payload),
  );
}

export function onPeerDisconnected(
  handler: (deviceKey: string) => void,
): Promise<UnlistenFn> {
  return listen<string>(PEER_DISCONNECTED, (event) => handler(event.payload));
}


export function onProfileUpdated(handler: (fingerprint: string) => void): Promise<UnlistenFn> {
  return listen<string>("zw://profile/updated", (event) => handler(event.payload));
}

export function onTransferStarted(
  handler: (jobId: string) => void,
): Promise<UnlistenFn> {
  return listen<string>(TRANSFER_STARTED, (event) => handler(event.payload));
}

export function onTransferArchiving(
  handler: (jobId: string) => void,
): Promise<UnlistenFn> {
  return listen<string>(TRANSFER_ARCHIVING, (event) => handler(event.payload));
}

export function onTransferProgress(
  handler: (jobId: string) => void,
): Promise<UnlistenFn> {
  return listen<string>(TRANSFER_PROGRESS, (event) => handler(event.payload));
}

export function onTransferCompleted(
  handler: (jobId: string) => void,
): Promise<UnlistenFn> {
  return listen<string>(TRANSFER_COMPLETED, (event) => handler(event.payload));
}

export function onReceiveCompleted(
  handler: (payload: ReceiveCompletedPayload) => void,
): Promise<UnlistenFn> {
  return listen<ReceiveCompletedPayload>(RECEIVE_COMPLETED, (event) => handler(event.payload));
}

export function onReceiveProgress(
  handler: (payload: ReceiveProgressPayload) => void,
): Promise<UnlistenFn> {
  return listen<ReceiveProgressPayload>(RECEIVE_PROGRESS, (event) => handler(event.payload));
}

export function onTransferCancelled(
  handler: (jobId: string) => void,
): Promise<UnlistenFn> {
  return listen<string>(TRANSFER_CANCELLED, (event) => handler(event.payload));
}

export function onTransferFailed(
  handler: (message: string) => void,
): Promise<UnlistenFn> {
  return listen<string>(TRANSFER_FAILED, (event) => handler(event.payload));
}

export function onTransferRejected(
  handler: (deviceKey: string) => void,
): Promise<UnlistenFn> {
  return listen<string>(TRANSFER_REJECTED, (event) => handler(event.payload));
}

export function onIncomingTransferRequest(
  handler: (payload: IncomingTransferRequestPayload) => void,
): Promise<UnlistenFn> {
  return listen<IncomingTransferRequestPayload>(INCOMING_TRANSFER_REQUEST, (event) =>
    handler(event.payload),
  );
}

export function onIncomingTransferExpired(
  handler: (requestId: string) => void,
): Promise<UnlistenFn> {
  return listen<string>(INCOMING_TRANSFER_EXPIRED, (event) => handler(event.payload));
}

export function onConnectionTestRequested(
  handler: (payload: ConnectionTestRequestPayload) => void,
): Promise<UnlistenFn> {
  return listen<ConnectionTestRequestPayload>(CONNECTION_TEST_REQUESTED, (event) =>
    handler(event.payload),
  );
}

export function onConnectionTestExpired(
  handler: (requestId: string) => void,
): Promise<UnlistenFn> {
  return listen<string>(CONNECTION_TEST_EXPIRED, (event) => handler(event.payload));
}

export function onTextReceived(
  handler: (payload: TextReceivedPayload) => void,
): Promise<UnlistenFn> {
  return listen<TextReceivedPayload>(TEXT_RECEIVED, (event) => handler(event.payload));
}
