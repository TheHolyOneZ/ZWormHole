

export interface MyProfilePayload {

  display_name: string | null;

  effective_name: string;

  avatar_data_url: string | null;
}

export interface DeviceProfilePayload {
  fingerprint: string;
  display_name: string;
  avatar_data_url: string | null;
}
