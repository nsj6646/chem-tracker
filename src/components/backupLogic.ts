import type { DbStatus } from "../lib/ipc";

export interface BackupResultPayload {
  target_path: string;
  file_size_bytes: number;
  status: DbStatus;
}

export interface RestoreResultPayload {
  pre_restore_backup_path: string;
  status: DbStatus;
}

export interface BackupIpcAdapter {
  pickSavePath(defaultFilename: string): Promise<string | null>;
  pickRestorePath(): Promise<string | null>;
  backupDatabase(targetPath: string): Promise<BackupResultPayload>;
  restoreDatabase(candidatePath: string): Promise<RestoreResultPayload>;
}

export interface BackupPanelState {
  busyAction: "none" | "backup" | "restore";
  feedbackMessage: string | null;
  errorMessage: string | null;
  lastBackupPath: string | null;
  lastPreRestorePath: string | null;
}

export type BackupAction =
  | { type: "BACKUP_STARTED" }
  | { type: "BACKUP_SUCCEEDED"; result: BackupResultPayload }
  | { type: "BACKUP_FAILED"; message: string }
  | { type: "RESTORE_STARTED" }
  | { type: "RESTORE_SUCCEEDED"; result: RestoreResultPayload }
  | { type: "RESTORE_FAILED"; message: string };

type TauriInvoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

export function formatDefaultBackupFilename(date: Date = new Date()): string {
  const year = String(date.getFullYear()).padStart(4, "0");
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `chem-tracker-backup-${year}${month}${day}.db`;
}

export function createInitialBackupPanelState(): BackupPanelState {
  return {
    busyAction: "none",
    feedbackMessage: null,
    errorMessage: null,
    lastBackupPath: null,
    lastPreRestorePath: null,
  };
}

export function reduceBackupPanelState(
  state: BackupPanelState,
  action: BackupAction,
): BackupPanelState {
  switch (action.type) {
    case "BACKUP_STARTED":
      return {
        ...state,
        busyAction: "backup",
        feedbackMessage: null,
        errorMessage: null,
      };
    case "BACKUP_SUCCEEDED":
      return {
        ...state,
        busyAction: "none",
        lastBackupPath: action.result.target_path,
        feedbackMessage: `백업 저장 완료 (${formatByteSize(action.result.file_size_bytes)}): ${action.result.target_path}`,
        errorMessage: null,
      };
    case "BACKUP_FAILED":
      return {
        ...state,
        busyAction: "none",
        errorMessage: action.message,
      };
    case "RESTORE_STARTED":
      return {
        ...state,
        busyAction: "restore",
        feedbackMessage: null,
        errorMessage: null,
      };
    case "RESTORE_SUCCEEDED":
      return {
        ...state,
        busyAction: "none",
        lastPreRestorePath: action.result.pre_restore_backup_path,
        feedbackMessage: `DB 복원 완료 (안전 백업: ${action.result.pre_restore_backup_path})`,
        errorMessage: null,
      };
    case "RESTORE_FAILED":
      return {
        ...state,
        busyAction: "none",
        errorMessage: action.message,
      };
  }
}

function formatByteSize(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const kb = bytes / 1024;
  if (kb < 1024) {
    return `${kb.toFixed(1)} KB`;
  }
  return `${(kb / 1024).toFixed(2)} MB`;
}

function detectTauriInvoke(): TauriInvoke | null {
  const g = globalThis as {
    __TAURI_INTERNALS__?: { invoke?: TauriInvoke };
  };
  return typeof g.__TAURI_INTERNALS__?.invoke === "function"
    ? g.__TAURI_INTERNALS__.invoke
    : null;
}

export function createBackupIpcAdapter(invokeOverride?: TauriInvoke): BackupIpcAdapter {
  const invoke = invokeOverride ?? detectTauriInvoke();
  if (invoke) {
    const dbFilters = [{ name: "SQLite Database", extensions: ["db"] }];
    return {
      async pickSavePath(defaultFilename: string): Promise<string | null> {
        const picked = await invoke<string | null>("plugin:dialog|save", {
          options: {
            title: "SQLite 데이터베이스 백업 저장",
            defaultPath: defaultFilename,
            filters: dbFilters,
          },
        });
        return picked && picked.trim().length > 0 ? picked : null;
      },
      async pickRestorePath(): Promise<string | null> {
        const picked = await invoke<string | string[] | null>("plugin:dialog|open", {
          options: {
            title: "복원할 SQLite 백업 파일 선택 (.db)",
            multiple: false,
            directory: false,
            filters: dbFilters,
          },
        });
        if (Array.isArray(picked)) {
          return picked[0] ?? null;
        }
        return picked && picked.trim().length > 0 ? picked : null;
      },
      backupDatabase(targetPath: string): Promise<BackupResultPayload> {
        return invoke<BackupResultPayload>("backup_database", { targetPath });
      },
      restoreDatabase(candidatePath: string): Promise<RestoreResultPayload> {
        return invoke<RestoreResultPayload>("restore_database", { candidatePath });
      },
    };
  }

  // Standalone browser / offline preview fallback
  const fallbackStatus: DbStatus = {
    db_path: "%LOCALAPPDATA%\\com.koreabranch.chemtracker\\data.db",
    journal_mode: "wal",
    integrity_ok: true,
    domestic_importers_count: 3,
    overseas_branches_count: 3,
    chemical_substances_count: 7,
    imported_products_count: 3,
    product_components_count: 9,
    import_shipments_count: 6,
  };

  return {
    async pickSavePath(defaultFilename: string): Promise<string | null> {
      return defaultFilename;
    },
    async pickRestorePath(): Promise<string | null> {
      return formatDefaultBackupFilename();
    },
    async backupDatabase(targetPath: string): Promise<BackupResultPayload> {
      return {
        target_path: targetPath,
        file_size_bytes: 32768,
        status: { ...fallbackStatus },
      };
    },
    async restoreDatabase(): Promise<RestoreResultPayload> {
      return {
        pre_restore_backup_path: "data.pre-restore.db",
        status: { ...fallbackStatus },
      };
    },
  };
}

function formatError(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

export async function executeBackupSave(
  adapter: BackupIpcAdapter,
  dispatch: (action: BackupAction) => void,
  options?: { date?: Date },
): Promise<boolean> {
  const defaultName = formatDefaultBackupFilename(options?.date);
  let targetPath: string | null;
  try {
    targetPath = await adapter.pickSavePath(defaultName);
  } catch (err) {
    dispatch({ type: "BACKUP_FAILED", message: formatError(err) });
    return false;
  }

  if (!targetPath) {
    return false;
  }

  dispatch({ type: "BACKUP_STARTED" });
  try {
    const result = await adapter.backupDatabase(targetPath);
    dispatch({ type: "BACKUP_SUCCEEDED", result });
    return true;
  } catch (err) {
    dispatch({ type: "BACKUP_FAILED", message: formatError(err) });
    return false;
  }
}

export async function executeBackupRestore(
  adapter: BackupIpcAdapter,
  dispatch: (action: BackupAction) => void,
  callbacks?: { onRestored?: (status: DbStatus) => void },
): Promise<boolean> {
  let candidatePath: string | null;
  try {
    candidatePath = await adapter.pickRestorePath();
  } catch (err) {
    dispatch({ type: "RESTORE_FAILED", message: formatError(err) });
    return false;
  }

  if (!candidatePath) {
    return false;
  }

  dispatch({ type: "RESTORE_STARTED" });
  try {
    const result = await adapter.restoreDatabase(candidatePath);
    dispatch({ type: "RESTORE_SUCCEEDED", result });
    callbacks?.onRestored?.(result.status);
    if (typeof window !== "undefined" && typeof window.dispatchEvent === "function") {
      window.dispatchEvent(
        new CustomEvent("chem-tracker:db-restored", { detail: result.status }),
      );
    }
    return true;
  } catch (err) {
    dispatch({ type: "RESTORE_FAILED", message: formatError(err) });
    return false;
  }
}
