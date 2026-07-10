import { describe, expect, it } from "vitest";
import {
  cloneWithFreshIds,
  createBuiltInOperation,
  nextOperationId,
} from "./patchOperationDefaults";
import type { PatchOperationNode } from "../types/patchFile";

function addOp(id: number, xpath: string): PatchOperationNode {
  return {
    id,
    className: "PatchOperationAdd",
    success: "normal",
    attributes: [],
    kind: { type: "add", data: { xpath, valueXml: null, order: null } },
    span: null,
  };
}

describe("nextOperationId", () => {
  it("returns 0 for an empty tree", () => {
    expect(nextOperationId([])).toBe(0);
  });

  it("returns one past the highest top-level id", () => {
    expect(nextOperationId([addOp(0, "a"), addOp(3, "b")])).toBe(4);
  });

  it("descends into sequence children", () => {
    const sequence: PatchOperationNode = {
      id: 5,
      className: "PatchOperationSequence",
      success: "normal",
      attributes: [],
      kind: { type: "sequence", data: [addOp(0, "a"), addOp(9, "b")] },
      span: null,
    };
    expect(nextOperationId([sequence])).toBe(10);
  });

  it("descends into conditional match/nomatch slots", () => {
    const conditional: PatchOperationNode = {
      id: 2,
      className: "PatchOperationConditional",
      success: "normal",
      attributes: [],
      kind: {
        type: "conditional",
        data: { xpath: "Defs/ThingDef", matchOp: addOp(7, "a"), nomatchOp: null },
      },
      span: null,
    };
    expect(nextOperationId([conditional])).toBe(8);
  });
});

describe("cloneWithFreshIds", () => {
  it("assigns a fresh id to the top-level node and preserves its data", () => {
    const original = addOp(3, "Defs/ThingDef");
    let counter = 100;
    const clone = cloneWithFreshIds(original, () => counter++);
    expect(clone.id).toBe(100);
    expect(clone.kind).toEqual(original.kind);
    expect(clone).not.toBe(original);
  });

  it("assigns fresh ids to every nested sequence child", () => {
    const sequence: PatchOperationNode = {
      id: 1,
      className: "PatchOperationSequence",
      success: "normal",
      attributes: [],
      kind: { type: "sequence", data: [addOp(2, "a"), addOp(3, "b")] },
      span: null,
    };
    let counter = 50;
    const clone = cloneWithFreshIds(sequence, () => counter++);
    expect(clone.id).toBe(50);
    if (clone.kind.type !== "sequence") throw new Error("expected sequence");
    expect(clone.kind.data.map((c) => c.id)).toEqual([51, 52]);
    // Original tree is untouched.
    if (sequence.kind.type !== "sequence") throw new Error("expected sequence");
    expect(sequence.kind.data.map((c) => c.id)).toEqual([2, 3]);
  });
});

describe("createBuiltInOperation", () => {
  it("creates a blank PatchOperationAdd with null fields", () => {
    const node = createBuiltInOperation("PatchOperationAdd", 7);
    expect(node.id).toBe(7);
    expect(node.className).toBe("PatchOperationAdd");
    expect(node.success).toBe("normal");
    expect(node.kind).toEqual({ type: "add", data: { xpath: null, valueXml: null, order: null } });
  });

  it("creates an empty PatchOperationSequence", () => {
    const node = createBuiltInOperation("PatchOperationSequence", 1);
    expect(node.kind).toEqual({ type: "sequence", data: [] });
  });

  it("creates a PatchOperationFindMod with empty mods and no match/nomatch", () => {
    const node = createBuiltInOperation("PatchOperationFindMod", 1);
    expect(node.kind).toEqual({
      type: "findMod",
      data: { mods: [], matchOp: null, nomatchOp: null },
    });
  });
});
