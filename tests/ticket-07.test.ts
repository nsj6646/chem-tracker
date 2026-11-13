import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { DbStatus } from "../src/lib/ipc";
import {
  type BackupAction,
  type BackupIpcAdapter,
  type BackupPanelState,
  createBackupIpcAdapter,
  createInitialBackupPanelState,
  executeBackupRestore,
  executeBackupSave,
  formatDefaultBackupFilename,
  reduceBackupPanelState,
} from "../src/components/backupLogic";

const sampleStatus: DbStatus = {
  db_path: "data.db",
  journal_mode: "wal",
  integrity_ok: true,
  domestic_importers_count: 3,
  overseas_branches_count: 3,
  chemical_substances_count: 7,
  imported_products_count: 3,
  product_components_count: 9,
  import_shipments_count: 6,
};

describe("Ticket 07 Frontend Backup & Restore Logic", () => {
  test("slice6: formatDefaultBackupFilename formats YYYYMMDD filename", () => {
    const dt = new Date(2026, 10, 14); // Nov 14, 2026 local
    expect(formatDefaultBackupFilename(dt)).toBe("chem-tracker-backup-20261114.db");
    const jan = new Date(2026, 0, 5); // Jan 5, 2026 local
    expect(formatDefaultBackupFilename(jan)).toBe("chem-tracker-backup-20260105.db");
  });

  test("slice7: executeBackupSave handles dialog pick, progress transitions, and cancellation", async () => {
    const actions: BackupAction[] = [];
    let state: BackupPanelState = createInitialBackupPanelState();
    const dispatch = (action: BackupAction) => {
      actions.push(action);
      state = reduceBackupPanelState(state, action);
    };

    const adapter: BackupIpcAdapter = {
      async pickSavePath(defaultFilename) {
        expect(defaultFilename).toBe("chem-tracker-backup-20261114.db");
        return "C:\\Backups\\chem-tracker-backup-20261114.db";
      },
      async pickRestorePath() {
        return null;
      },
      async backupDatabase(targetPath) {
        expect(targetPath).toBe("C:\\Backups\\chem-tracker-backup-20261114.db");
        return {
          target_path: targetPath,
          file_size_bytes: 32768,
          status: sampleStatus,
        };
      },
      async restoreDatabase() {
        throw new Error("unexpected");
      },
    };

    const saved = await executeBackupSave(adapter, dispatch, {
      date: new Date(2026, 10, 14),
    });
    expect(saved).toBe(true);
    expect(actions.map((a) => a.type)).toEqual(["BACKUP_STARTED", "BACKUP_SUCCEEDED"]);
    expect(state.busyAction).toBe("none");
    expect(state.errorMessage).toBeNull();
    expect(state.feedbackMessage).toContain("백업 저장 완료");
    expect(state.lastBackupPath).toBe("C:\\Backups\\chem-tracker-backup-20261114.db");

    // Cancelled save dialog produces no error and no IPC backup call
    const cancelAdapter: BackupIpcAdapter = {
      ...adapter,
      async pickSavePath() {
        return null;
      },
      async backupDatabase() {
        throw new Error("should not run when cancelled");
      },
    };
    const cancelled = await executeBackupSave(cancelAdapter, dispatch, {
      date: new Date(2026, 10, 14),
    });
    expect(cancelled).toBe(false);
    expect(state.busyAction).toBe("none");
    expect(state.errorMessage).toBeNull();
  });

  test("slice8: executeBackupRestore validates errors and invokes post-restore refresh callback", async () => {
    let state: BackupPanelState = createInitialBackupPanelState();
    const dispatch = (action: BackupAction) => {
      state = reduceBackupPanelState(state, action);
    };

    let refreshedStatus: DbStatus | null = null;

    // 1. Corrupt / schema-incompatible file rejection surfaces Korean error and skips refresh
    const failingAdapter: BackupIpcAdapter = {
      async pickSavePath() {
        return null;
      },
      async pickRestorePath() {
        return "C:\\Backups\\corrupt.db";
      },
      async backupDatabase() {
        throw new Error("unexpected");
      },
      async restoreDatabase() {
        throw new Error(
          "백업 파일 무결성 검사(PRAGMA integrity_check)에 실패했습니다: malformed",
        );
      },
    };

    const failOk = await executeBackupRestore(failingAdapter, dispatch, {
      onRestored: (status) => {
        refreshedStatus = status;
      },
    });
    expect(failOk).toBe(false);
    expect(state.busyAction).toBe("none");
    expect(state.errorMessage).toContain("PRAGMA integrity_check");
    expect(refreshedStatus).toBeNull();

    // 2. Successful restore invokes onRestored callback and records pre-restore safety path
    const okAdapter: BackupIpcAdapter = {
      ...failingAdapter,
      async pickRestorePath() {
        return "C:\\Backups\\chem-tracker-backup-20261114.db";
      },
      async restoreDatabase(candidatePath) {
        expect(candidatePath).toBe("C:\\Backups\\chem-tracker-backup-20261114.db");
        return {
          pre_restore_backup_path: "C:\\Data\\data.pre-restore.db",
          status: sampleStatus,
        };
      },
    };

    const ok = await executeBackupRestore(okAdapter, dispatch, {
      onRestored: (status) => {
        refreshedStatus = status;
      },
    });
    expect(ok).toBe(true);
    expect(state.busyAction).toBe("none");
    expect(state.errorMessage).toBeNull();
    expect(state.feedbackMessage).toContain("복원 완료");
    expect(state.lastPreRestorePath).toBe("C:\\Data\\data.pre-restore.db");
    expect(refreshedStatus).toEqual(sampleStatus);
  });

  test("slice9: createBackupIpcAdapter wires Tauri commands & DatabaseFooterPanel compiles cleanly", async () => {
    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
    const tauriAdapter = createBackupIpcAdapter(async <T,>(cmd: string, args?: Record<string, unknown>) => {
      calls.push({ cmd, args });
      if (cmd === "plugin:dialog|save") {
        return "C:\\temp\\backup.db" as T;
      }
      if (cmd === "plugin:dialog|open") {
        return "C:\\temp\\restore.db" as T;
      }
      if (cmd === "backup_database") {
        return {
          target_path: String(args?.targetPath ?? ""),
          file_size_bytes: 4096,
          status: sampleStatus,
        } as T;
      }
      if (cmd === "restore_database") {
        return {
          pre_restore_backup_path: "C:\\temp\\data.pre-restore.db",
          status: sampleStatus,
        } as T;
      }
      throw new Error(`unexpected cmd: ${cmd}`);
    });

    const savePath = await tauriAdapter.pickSavePath("chem-tracker-backup-20261114.db");
    expect(savePath).toBe("C:\\temp\\backup.db");
    await tauriAdapter.backupDatabase(savePath!);
    const openPath = await tauriAdapter.pickRestorePath();
    expect(openPath).toBe("C:\\temp\\restore.db");
    await tauriAdapter.restoreDatabase(openPath!);

    expect(calls.map((c) => c.cmd)).toEqual([
      "plugin:dialog|save",
      "backup_database",
      "plugin:dialog|open",
      "restore_database",
    ]);

    const root = join(import.meta.dir, "..");
    const footerTsx = readFileSync(join(root, "src/components/DatabaseFooterPanel.tsx"), "utf8");
    expect(footerTsx).toContain("executeBackupSave");
    expect(footerTsx).toContain("executeBackupRestore");
    expect(footerTsx).toContain("DB 백업 저장 (.db)");
    expect(footerTsx).toContain("DB 백업 불러오기 (복원)");

    const transpiler = new Bun.Transpiler({ loader: "tsx" });
    expect(transpiler.transformSync(footerTsx).length).toBeGreaterThan(0);
  });
});
