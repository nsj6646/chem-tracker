export interface DbStatus {
  db_path: string;
  journal_mode: string;
  integrity_ok: boolean;
  domestic_importers_count: number;
  overseas_branches_count: number;
  chemical_substances_count: number;
  imported_products_count: number;
  product_components_count: number;
  import_shipments_count: number;
}

export interface SeedResult {
  inserted_importers: number;
  inserted_branches: number;
  inserted_substances: number;
  inserted_products: number;
  inserted_shipments: number;
  already_seeded: boolean;
  status: DbStatus;
}

export interface IpcClient {
  getDbStatus(): Promise<DbStatus>;
  seedSampleData(): Promise<SeedResult>;
}

export const DB_RESTORED_EVENT = "chem-tracker:db-restored";

export function subscribeDbRestoredEvent(onRestored: () => void): () => void {
  if (typeof window === "undefined" || typeof window.addEventListener !== "function") {
    return () => {};
  }
  const handler = () => {
    onRestored();
  };
  window.addEventListener(DB_RESTORED_EVENT, handler);
  return () => {
    window.removeEventListener(DB_RESTORED_EVENT, handler);
  };
}

export type TauriInvoke = <T>(
  cmd: string,
  args?: Record<string, unknown>,
) => Promise<T>;

export function detectTauriInvoke(): TauriInvoke | null {
  const g = globalThis as {
    __TAURI_INTERNALS__?: { invoke?: TauriInvoke };
  };
  return typeof g.__TAURI_INTERNALS__?.invoke === "function"
    ? g.__TAURI_INTERNALS__.invoke
    : null;
}

/**
 * Creates a typed IPC client.
 * Uses Tauri v2 invoke when running inside the desktop shell, and provides
 * an isolated in-memory fallback when running in standalone browser/unit tests.
 */
export function createIpcClient(invokeOverride?: TauriInvoke): IpcClient {
  const invoke = invokeOverride ?? detectTauriInvoke();
  if (invoke) {
    return {
      getDbStatus: () => invoke<DbStatus>("get_db_status"),
      seedSampleData: () => invoke<SeedResult>("seed_sample_data"),
    };
  }

  // Standalone browser / offline test fallback state
  let seeded = false;
  let status: DbStatus = {
    db_path: "%LOCALAPPDATA%\\com.koreabranch.chemtracker\\data.db",
    journal_mode: "wal",
    integrity_ok: true,
    domestic_importers_count: 0,
    overseas_branches_count: 0,
    chemical_substances_count: 0,
    imported_products_count: 0,
    product_components_count: 0,
    import_shipments_count: 0,
  };

  return {
    async getDbStatus(): Promise<DbStatus> {
      return { ...status };
    },
    async seedSampleData(): Promise<SeedResult> {
      if (seeded) {
        return {
          inserted_importers: 0,
          inserted_branches: 0,
          inserted_substances: 0,
          inserted_products: 0,
          inserted_shipments: 0,
          already_seeded: true,
          status: { ...status },
        };
      }
      seeded = true;
      status = {
        ...status,
        domestic_importers_count: 3,
        overseas_branches_count: 3,
        chemical_substances_count: 7,
        imported_products_count: 3,
        product_components_count: 9,
        import_shipments_count: 6,
      };
      return {
        inserted_importers: 3,
        inserted_branches: 3,
        inserted_substances: 7,
        inserted_products: 3,
        inserted_shipments: 6,
        already_seeded: false,
        status: { ...status },
      };
    },
  };
}
