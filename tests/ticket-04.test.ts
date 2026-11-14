import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  buildDeleteBlockedReason,
  createMastersIpcClient,
  createOfflineMastersIpc,
  formatMasterReferenceSummary,
  getMasterEntityMeta,
  normalizeCasNumber,
  validateMasterDraft,
} from "../src/workspaces/mastersLogic";

describe("Ticket 04 Frontend Master Management & Delete Guards", () => {
  test("slice 4: normalization, validation, reference formatting, and IPC client CRUD + delete guard", async () => {
    // 1. CAS number normalization
    expect(normalizeCasNumber(" 67 - 64 – 1 ")).toBe("67-64-1");
    expect(normalizeCasNumber("  50-00-0  ")).toBe("50-00-0");

    // 2. Entity metadata & reference summaries
    expect(getMasterEntityMeta("domestic_importer").title).toContain("국내 수입사");
    expect(getMasterEntityMeta("overseas_branch").title).toContain("해외지사");
    expect(getMasterEntityMeta("chemical_substance").title).toContain("화학물질");

    const client = createOfflineMastersIpc();

    const importers = await client.listMasterItems("domestic_importer");
    expect(importers.length).toBe(3);
    const daehan = importers.find((i) => i.name === "(주)대한케미칼")!;
    const sejin = importers.find((i) => i.name === "세진소재산업(주)")!;
    expect(daehan.reference_count).toBe(5);
    expect(sejin.reference_count).toBe(0);
    expect(formatMasterReferenceSummary(daehan)).toContain("5건");
    expect(formatMasterReferenceSummary(sejin)).toContain("0건");
    expect(buildDeleteBlockedReason(daehan)).toContain("5건");
    expect(buildDeleteBlockedReason(sejin)).toBeNull();

    // 3. Draft validation: empty, case-insensitive duplicate, valid rename
    const branches = await client.listMasterItems("overseas_branch");
    const eu = branches.find((b) => b.name === "유럽지사 (독일)")!;
    expect(eu.product_count).toBe(2);
    expect(eu.shipment_count).toBe(5);
    expect(eu.reference_count).toBe(7);
    expect(formatMasterReferenceSummary(eu)).toContain("7건");

    expect(
      validateMasterDraft(
        "overseas_branch",
        { name: "   ", casNumber: "", substanceName: "", note: "" },
        branches,
        eu.id,
      ).ok,
    ).toBe(false);

    // Case-insensitive duplicate detection
    const branchesWithAscii = [
      ...branches,
      {
        id: 99,
        kind: "overseas_branch" as const,
        name: "UK Branch",
        cas_number: null,
        substance_name: null,
        note: null,
        reference_count: 0,
        product_count: 0,
        component_count: 0,
        shipment_count: 0,
      },
    ];
    const dupBranch = validateMasterDraft(
      "overseas_branch",
      { name: " uk branch ", casNumber: "", substanceName: "", note: "" },
      branchesWithAscii,
      eu.id,
    );
    expect(dupBranch.ok).toBe(false);

    // 4. Chemical substance draft validation: normalized CAS uniqueness
    const substances = await client.listMasterItems("chemical_substance");
    const poly = substances.find((s) => s.cas_number === "9002-88-4")!;
    const dupCas = validateMasterDraft(
      "chemical_substance",
      {
        name: "",
        casNumber: " 67 - 64 - 1 ",
        substanceName: "아세톤",
        note: "",
      },
      substances,
      poly.id,
    );
    expect(dupCas.ok).toBe(false);

    // 5. Update in place via offline IPC
    const renamed = await client.updateMasterItem({
      id: daehan.id,
      kind: "domestic_importer",
      name: " (주)대한케미칼 코리아 ",
      cas_number: null,
      substance_name: null,
      note: " 수정 메모 ",
    });
    expect(renamed.name).toBe("(주)대한케미칼 코리아");
    expect(renamed.note).toBe("수정 메모");
    expect(renamed.reference_count).toBe(5);

    // 6. Delete blocked when referenced, succeeds when unreferenced
    await expect(
      client.deleteMasterItem("domestic_importer", daehan.id),
    ).rejects.toThrow("5건");

    const deletedImp = await client.deleteMasterItem("domestic_importer", sejin.id);
    expect(deletedImp.deleted_id).toBe(sejin.id);
    expect((await client.listMasterItems("domestic_importer")).length).toBe(2);

    const jp = branches.find((b) => b.name === "일본지사 (도쿄)")!;
    await expect(
      client.deleteMasterItem("overseas_branch", eu.id),
    ).rejects.toThrow("7건");
    const deletedBranch = await client.deleteMasterItem("overseas_branch", jp.id);
    expect(deletedBranch.deleted_id).toBe(jp.id);

    // Unreferenced chemical substance deletion + referenced substance guard
    const customClient = createOfflineMastersIpc([
      ...substances,
      {
        id: 999,
        kind: "chemical_substance",
        name: null,
        cas_number: "50-00-0",
        substance_name: "포름알데히드",
        note: null,
        reference_count: 0,
        product_count: 0,
        component_count: 0,
        shipment_count: 0,
      },
    ]);
    await expect(
      customClient.deleteMasterItem("chemical_substance", poly.id),
    ).rejects.toThrow("1건");
    const deletedSub = await customClient.deleteMasterItem("chemical_substance", 999);
    expect(deletedSub.deleted_label).toBe("50-00-0");

    // 7. Tauri invoke override delegates with expected command names and args
    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
    const tauriClient = createMastersIpcClient(
      async <T>(cmd: string, args?: Record<string, unknown>) => {
        calls.push({ cmd, args });
        return [] as unknown as T;
      },
    );
    await tauriClient.listMasterItems("overseas_branch");
    await tauriClient.deleteMasterItem("chemical_substance", 12);
    expect(calls).toEqual([
      { cmd: "list_master_items", args: { kind: "overseas_branch" } },
      { cmd: "delete_master_item", args: { kind: "chemical_substance", id: 12 } },
    ]);
  });

  test("slice 5: MasterModal and Workspace 1 & 2 wiring compile and expose required controls", async () => {
    const root = join(import.meta.dir, "..");
    const modalTsx = readFileSync(
      join(root, "src/workspaces/MasterModal.tsx"),
      "utf8",
    );
    const productsTsx = readFileSync(
      join(root, "src/workspaces/ProductsWorkspace.tsx"),
      "utf8",
    );
    const shipmentsTsx = readFileSync(
      join(root, "src/workspaces/ShipmentsWorkspace.tsx"),
      "utf8",
    );

    const transpiler = new Bun.Transpiler({ loader: "tsx" });
    expect(transpiler.transformSync(modalTsx).length).toBeGreaterThan(0);
    expect(transpiler.transformSync(productsTsx).length).toBeGreaterThan(0);
    expect(transpiler.transformSync(shipmentsTsx).length).toBeGreaterThan(0);

    // MasterModal must support all 3 kinds, inline editing, reference counts, delete guard, and onChanged
    for (const token of [
      "MasterEntityKind",
      "createMastersIpcClient",
      "validateMasterDraft",
      "formatMasterReferenceSummary",
      "buildDeleteBlockedReason",
      'role="dialog"',
      "aria-modal",
      "수정",
      "삭제",
      "저장",
      "취소",
      "onChanged",
    ]) {
      expect(modalTsx).toContain(token);
    }

    // ProductsWorkspace wires MasterModal directly instead of placeholder text
    expect(productsTsx).toContain('from "./MasterModal"');
    expect(productsTsx).toContain("<MasterModal");
    expect(productsTsx).toContain('handleTriggerMasterModal("overseas_branch")');
    expect(productsTsx).toContain('handleTriggerMasterModal("chemical_substance")');
    expect(productsTsx).not.toContain("연결 지점입니다");

    // ShipmentsWorkspace wires MasterModal with domestic_importer kind & onChanged refresh
    expect(shipmentsTsx).toContain("<MasterModal");
    expect(shipmentsTsx).toContain('kind="domestic_importer"');
    expect(shipmentsTsx).toContain("onChanged");
  });
});
