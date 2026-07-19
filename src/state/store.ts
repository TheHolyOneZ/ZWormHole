

import { create } from "zustand";
import {
  becomeVisible,
  blockDevice,
  listActivePeers,
  listBlockedDevices,
  listNearbyDevices,
  listProfiles,
  listTransfers,
  listTrustedDevices,
  revealPath,
  sendConnectRequest,
  startBrowsing,
  stopVisible,
  unblockDevice,
} from "../ipc/commands";
import { formatDeviceName } from "../lib/format";
import {
  onDeviceAppeared,
  onDeviceLost,
  onDeviceUpdated,
  onIncomingTransferExpired,
  onIncomingTransferRequest,
  onPairingFailed,
  onPairingRequested,
  onPairingSucceeded,
  onConnectionTestExpired,
  onConnectionTestRequested,
  onPeerDisconnected,
  onProfileUpdated,
  onReceiveCompleted,
  onReceiveProgress,
  onTextReceived,
  onTransferArchiving,
  onTransferCancelled,
  onTransferCompleted,
  onTransferFailed,
  onTransferProgress,
  onTransferRejected,
  onTransferStarted,
} from "../ipc/events";
import type { NearbyDevicePayload } from "../types/discovery";
import type { IncomingTransferRequestPayload } from "../types/incoming";
import type { ConnectionTestRequestPayload } from "../types/conntest";
import type { TextReceivedPayload } from "../types/text";
import type { FolderNode } from "../types/tree";

export interface FolderPick {
  deviceKey: string;
  displayName: string;
  folderPath: string;
  tree: FolderNode;


  deviceKeys?: string[];
}
import type { PairingRequestedPayload } from "../types/pairing";
import type { LiveReceive, TransferJob } from "../types/transfer";
import type { TrustedDevicePayload } from "../types/trusted";
import type { BlockedDevice } from "../types/blocked";
import type { DeviceProfilePayload } from "../types/profile";
import type { ActivePeerPayload } from "../types/peers";
import { describeError } from "../lib/errors";

export interface OutgoingPairing {
  fullname: string;
  displayName: string;
  code: string;
}

export interface Toast {
  id: string;
  kind: "success" | "error";
  message: string;

  action?: { label: string; run: () => void };
}

export interface LogEntry {
  id: string;
  at: number;
  kind: "info" | "success" | "error";
  message: string;
}

interface AppStore {
  visible: boolean;
  browsing: boolean;
  nearby: Record<string, NearbyDevicePayload>;
  trusted: TrustedDevicePayload[];

  blocked: BlockedDevice[];

  profiles: Record<string, DeviceProfilePayload>;
  activePeers: ActivePeerPayload[];
  transfers: Record<string, TransferJob>;


  transferRates: Record<string, number>;


  transferRateHistory: Record<string, number[]>;


  receives: Record<string, LiveReceive>;


  recvGraphs: Record<string, number[]>;

  selectedTransferId: string | null;

  benchmarkTarget: { fingerprint: string; displayName: string } | null;

  thisDeviceOpen: boolean;


  viewMode: "dashboard" | "focus";

  focusDevice: string | null;

  folderPick: FolderPick | null;


  dropHoverKey: string | null;


  dashboardSection: string;

  incomingPairing: PairingRequestedPayload | null;


  pairingQueue: PairingRequestedPayload[];
  outgoingPairing: OutgoingPairing | null;
  incomingTransfer: IncomingTransferRequestPayload | null;
  incomingConnectionTest: ConnectionTestRequestPayload | null;
  incomingText: TextReceivedPayload | null;
  toasts: Toast[];


  busy: string | null;


  activityLog: LogEntry[];
  settingsOpen: boolean;

  initialize: () => Promise<void>;
  toggleVisible: () => Promise<void>;
  refreshTrusted: () => Promise<void>;
  refreshBlocked: () => Promise<void>;

  refreshProfiles: () => Promise<void>;

  blockDevice: (fingerprint: string, displayName: string) => Promise<void>;

  unblockDevice: (fingerprint: string) => Promise<void>;
  refreshActivePeers: () => Promise<void>;
  refreshTransfers: () => Promise<void>;
  setOutgoingPairing: (p: OutgoingPairing | null) => void;
  setSelectedTransfer: (id: string | null) => void;
  setBenchmarkTarget: (t: { fingerprint: string; displayName: string } | null) => void;
  setDashboardSection: (s: string) => void;
  setThisDeviceOpen: (open: boolean) => void;
  setViewMode: (mode: "dashboard" | "focus") => void;
  setFocusDevice: (deviceKey: string | null) => void;
  setFolderPick: (p: FolderPick | null) => void;
  setDropHoverKey: (key: string | null) => void;
  setBusy: (message: string | null) => void;
  clearIncomingPairing: () => void;
  clearIncomingTransfer: () => void;
  clearIncomingConnectionTest: () => void;
  clearIncomingText: () => void;
  setSettingsOpen: (open: boolean) => void;
  pushToast: (kind: Toast["kind"], message: string, action?: Toast["action"]) => void;
  dismissToast: (id: string) => void;
  addLog: (kind: LogEntry["kind"], message: string) => void;
  clearActivityLog: () => void;
}

let initialized = false;


const autoReconnectInFlight = new Set<string>();


const rateSamples = new Map<string, { at: number; bytes: number }>();


const RATE_SMOOTHING = 0.4;

function isSessionValid(expiresAt: number | null): boolean {
  if (expiresAt === null) return true;
  return expiresAt > Math.floor(Date.now() / 1000);
}


function maybeAutoReconnect(
  device: NearbyDevicePayload,
  get: () => AppStore,
) {
  const { trusted, activePeers } = get();
  const match = trusted.find((t) => t.fingerprint === device.fingerprint);
  if (!match || !isSessionValid(match.expires_at)) return;
  if (activePeers.some((p) => p.device_key === device.fingerprint)) return;
  if (autoReconnectInFlight.has(device.fingerprint)) return;

  autoReconnectInFlight.add(device.fingerprint);
  void sendConnectRequest(device.fullname, null)
    .then((code) => {
      if (code === null) {
        void get().refreshActivePeers();
      }


    })
    .catch(() => {})
    .finally(() => autoReconnectInFlight.delete(device.fingerprint));
}

export const useAppStore = create<AppStore>((set, get) => ({
  visible: false,
  browsing: false,
  nearby: {},
  trusted: [],
  blocked: [],
  profiles: {},
  activePeers: [],
  transfers: {},
  transferRates: {},
  transferRateHistory: {},
  receives: {},
  recvGraphs: {},
  selectedTransferId: null,
  benchmarkTarget: null,
  dashboardSection: "home",
  thisDeviceOpen: false,
  viewMode: "dashboard",
  focusDevice: null,
  folderPick: null,
  dropHoverKey: null,
  incomingPairing: null,
  pairingQueue: [],
  outgoingPairing: null,
  incomingTransfer: null,
  incomingConnectionTest: null,
  incomingText: null,
  toasts: [],
  busy: null,
  activityLog: [],
  settingsOpen: false,

  initialize: async () => {
    if (initialized) return;
    initialized = true;

    const [nearby, trusted, blocked, profiles, activePeers, transfers] = await Promise.all([
      listNearbyDevices(),
      listTrustedDevices(),
      listBlockedDevices(),
      listProfiles(),
      listActivePeers(),
      listTransfers(),
    ]);
    set({
      nearby: Object.fromEntries(nearby.map((d) => [d.fullname, d])),
      trusted,
      blocked,
      profiles: Object.fromEntries(profiles.map((p) => [p.fingerprint, p])),
      activePeers,
      transfers: Object.fromEntries(transfers.map((t) => [t.id, t])),
    });
    for (const d of nearby) maybeAutoReconnect(d, get);


    try {
      await startBrowsing();
      set({ browsing: true });
    } catch (e) {
      get().pushToast("error", describeError(e));
    }


    try {
      await becomeVisible();
      set({ visible: true });
    } catch {

    }

    onDeviceAppeared((d) => {
      set((s) => ({ nearby: { ...s.nearby, [d.fullname]: d } }));
      maybeAutoReconnect(d, get);
    });
    onDeviceUpdated((d) => {
      set((s) => ({ nearby: { ...s.nearby, [d.fullname]: d } }));
      maybeAutoReconnect(d, get);
    });
    onDeviceLost(({ fullname }) =>
      set((s) => {
        const next = { ...s.nearby };
        delete next[fullname];
        return { nearby: next };
      }),
    );

    onPairingRequested((payload) =>
      set((s) => {


        const known =
          s.incomingPairing?.request_id === payload.request_id ||
          s.pairingQueue.some((p) => p.request_id === payload.request_id);
        if (known) return {};
        if (s.incomingPairing) return { pairingQueue: [...s.pairingQueue, payload] };
        return { incomingPairing: payload };
      }),
    );

    onPairingSucceeded((payload) => {


      set({ outgoingPairing: null });
      get().pushToast("success", `Paired with ${payload.display_name}`);
      get().addLog("success", `Paired with ${payload.display_name}`);
      void get().refreshTrusted();
      void get().refreshActivePeers();
    });

    onPairingFailed((payload) => {
      set({ outgoingPairing: null });
      get().pushToast("error", `Pairing failed: ${payload.reason}`);
      get().addLog("error", `Pairing failed: ${payload.reason}`);
    });

    onPeerDisconnected((deviceKey) => {
      const name = get().activePeers.find((p) => p.device_key === deviceKey)?.display_name;
      get().addLog("info", `${name ?? "A device"} disconnected`);
      set((s) => ({
        activePeers: s.activePeers.filter((p) => p.device_key !== deviceKey),
      }));
    });


    onProfileUpdated(() => {
      void get().refreshProfiles();
    });

    onTransferStarted((jobId) => {
      void get().refreshTransfers();


      if (get().viewMode !== "focus") {
        set({ dashboardSection: "transfers" });
      }
      void jobId;
    });


    onTransferArchiving((jobId) => {
      void get().refreshTransfers();
      if (get().viewMode !== "focus") {
        set({ dashboardSection: "transfers" });
      }
      void jobId;
    });


    onTransferProgress(() => {
      void get().refreshTransfers();
    });
    onTransferCompleted((jobId) => {
      get().pushToast("success", "Transfer complete");
      get().addLog("success", "Transfer complete");
      void get().refreshTransfers();
      void jobId;
    });
    onReceiveProgress((p) => {
      set((s) => {
        const now = Date.now();
        const prev = s.receives[p.id];
        let rate = prev?.rate ?? 0;
        if (prev && now > prev.updated) {
          const dBytes = p.bytes_done - prev.bytes_done;
          const dt = (now - prev.updated) / 1000;
          if (dt > 0 && dBytes >= 0) rate = dBytes / dt;
        }
        const history = [...(prev?.history ?? []), rate].slice(-60);

        const receives: Record<string, LiveReceive> = {};
        for (const [id, r] of Object.entries(s.receives)) {
          if (id === p.id || now - r.updated < 25_000) receives[id] = r;
        }
        receives[p.id] = {
          id: p.id,
          device_key: p.device_key,
          display_name: p.display_name,
          label: p.label,
          bytes_done: p.bytes_done,
          bytes_total: p.bytes_total,
          rate,
          history,
          updated: now,
        };
        return { receives };
      });
    });
    onReceiveCompleted((p) => {


      set((s) => {
        const live = s.receives[p.id];
        const receives = { ...s.receives };
        delete receives[p.id];
        const recvGraphs = { ...s.recvGraphs };
        if (live && live.history.length >= 2 && p.paths.length > 0) {
          recvGraphs[p.paths.join("|")] = live.history;
        }
        return { receives, recvGraphs };
      });
      const n = p.file_names.length;
      const who = formatDeviceName(p.display_name);
      const msg = `Received ${n} file${n === 1 ? "" : "s"} from ${who}`;
      const first = p.paths[0];
      get().pushToast(
        "success",
        msg,
        first ? { label: "Open folder", run: () => void revealPath(first) } : undefined,
      );
      get().addLog("success", msg);
    });
    onTransferCancelled(() => {

      get().addLog("info", "Transfer cancelled");
      void get().refreshTransfers();
    });
    onTransferFailed((message) => {
      get().pushToast("error", `Transfer failed: ${message}`);
      get().addLog("error", `Transfer failed: ${message}`);
      void get().refreshTransfers();
    });
    onTransferRejected(() => {
      get().pushToast("error", "The other device declined the transfer");
      get().addLog("info", "A transfer was declined");
      void get().refreshTransfers();
    });

    onIncomingTransferRequest((payload) => {
      set({ incomingTransfer: payload });
      get().addLog("info", `${payload.display_name} wants to send you files`);
    });
    onIncomingTransferExpired((requestId) =>
      set((s) =>
        s.incomingTransfer?.request_id === requestId
          ? { incomingTransfer: null }
          : {},
      ),
    );

    onTextReceived((payload) => {
      set({ incomingText: payload });
      get().addLog("info", `${payload.display_name} sent you text`);
    });

    onConnectionTestRequested((payload) => {
      set({ incomingConnectionTest: payload });
      const label = payload.kind === "benchmark" ? "a benchmark" : "a connection test";
      get().addLog("info", `${payload.display_name} wants to run ${label}`);
    });
    onConnectionTestExpired((requestId) =>
      set((s) =>
        s.incomingConnectionTest?.request_id === requestId
          ? { incomingConnectionTest: null }
          : {},
      ),
    );
  },

  toggleVisible: async () => {
    const visible = get().visible;
    try {
      if (visible) {
        await stopVisible();
        set({ visible: false });
      } else {
        await becomeVisible();


        if (!get().browsing) {
          try {
            await startBrowsing();
            set({ browsing: true });
          } catch {

          }
        }
        set({ visible: true });
      }
    } catch (e) {
      get().pushToast("error", describeError(e));
    }
  },

  refreshTrusted: async () => {
    const trusted = await listTrustedDevices();
    set({ trusted });
  },

  refreshBlocked: async () => {
    const blocked = await listBlockedDevices();
    set({ blocked });
  },

  refreshProfiles: async () => {
    const list = await listProfiles();
    set({ profiles: Object.fromEntries(list.map((p) => [p.fingerprint, p])) });
  },

  blockDevice: async (fingerprint, displayName) => {
    await blockDevice(fingerprint, displayName);


    set((s) => ({
      nearby: Object.fromEntries(
        Object.entries(s.nearby).filter(([, d]) => d.fingerprint !== fingerprint),
      ),
    }));
    await Promise.all([get().refreshTrusted(), get().refreshBlocked(), get().refreshActivePeers()]);
    get().pushToast("success", `Blocked ${formatDeviceName(displayName)}`);
  },

  unblockDevice: async (fingerprint) => {
    await unblockDevice(fingerprint);
    await get().refreshBlocked();
  },

  refreshActivePeers: async () => {
    const activePeers = await listActivePeers();
    set({ activePeers });
  },

  refreshTransfers: async () => {
    const transfers = await listTransfers();
    const now = Date.now();
    const rates = { ...get().transferRates };
    const history = { ...get().transferRateHistory };
    const byId: Record<string, TransferJob> = {};

    for (const t of transfers) {
      byId[t.id] = t;
      if (t.status !== "in_progress") {


        rateSamples.delete(t.id);
        delete rates[t.id];
        continue;
      }
      const prev = rateSamples.get(t.id);
      if (prev && now > prev.at) {
        const instantaneous = ((t.bytes_done - prev.bytes) * 1000) / (now - prev.at);
        const smoothed =
          rates[t.id] === undefined
            ? instantaneous
            : rates[t.id] * (1 - RATE_SMOOTHING) + instantaneous * RATE_SMOOTHING;
        rates[t.id] = Math.max(0, smoothed);

        history[t.id] = [...(history[t.id] ?? []), rates[t.id]].slice(-60);
      }
      rateSamples.set(t.id, { at: now, bytes: t.bytes_done });
    }


    for (const id of Object.keys(rates)) {
      if (!byId[id]) {
        delete rates[id];
        delete history[id];
        rateSamples.delete(id);
      }
    }

    set({ transfers: byId, transferRates: rates, transferRateHistory: history });
  },

  setOutgoingPairing: (p) => set({ outgoingPairing: p }),
  setSelectedTransfer: (id) => set({ selectedTransferId: id }),
  setBenchmarkTarget: (t) => set({ benchmarkTarget: t }),
  setDashboardSection: (s) => set({ dashboardSection: s }),
  setThisDeviceOpen: (open) => set({ thisDeviceOpen: open }),
  setViewMode: (mode) => set({ viewMode: mode }),
  setFocusDevice: (deviceKey) => set({ focusDevice: deviceKey }),
  setFolderPick: (p) => set({ folderPick: p }),
  setDropHoverKey: (key) => set({ dropHoverKey: key }),
  setBusy: (message) => set({ busy: message }),
  clearIncomingPairing: () =>


    set((s) => {
      const [next, ...rest] = s.pairingQueue;
      return { incomingPairing: next ?? null, pairingQueue: rest };
    }),
  clearIncomingTransfer: () => set({ incomingTransfer: null }),
  clearIncomingConnectionTest: () => set({ incomingConnectionTest: null }),
  clearIncomingText: () => set({ incomingText: null }),
  setSettingsOpen: (open) => set({ settingsOpen: open }),

  pushToast: (kind, message, action) => {
    const id = crypto.randomUUID();
    set((s) => ({ toasts: [...s.toasts, { id, kind, message, action }] }));

    setTimeout(() => get().dismissToast(id), action ? 8000 : 4000);
  },
  dismissToast: (id) =>
    set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),

  addLog: (kind, message) =>
    set((s) => ({

      activityLog: [
        { id: crypto.randomUUID(), at: Date.now(), kind, message },
        ...s.activityLog,
      ].slice(0, 200),
    })),
  clearActivityLog: () => set({ activityLog: [] }),
}));
