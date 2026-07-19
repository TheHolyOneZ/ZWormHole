

export interface TrustedDevicePayload {
  fingerprint: string;
  display_name: string;
  added_at: string;
  expires_at: number | null;
  nickname: string | null;
  favorite: boolean;
  auto_accept: boolean;
}
