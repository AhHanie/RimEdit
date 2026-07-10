export interface InstrumentationConfig {
  enabled: boolean;
}

export interface DeriveConfigParams {
  dev: boolean;
  envEnabled: string | undefined;
  localStorageEnabled: string | null;
}

export function deriveInstrumentationConfig(params: DeriveConfigParams): InstrumentationConfig {
  if (!params.dev) {
    return { enabled: false };
  }
  if (params.localStorageEnabled !== null) {
    return {
      enabled: params.localStorageEnabled === "true" || params.localStorageEnabled === "1",
    };
  }
  return { enabled: params.envEnabled === "1" };
}
