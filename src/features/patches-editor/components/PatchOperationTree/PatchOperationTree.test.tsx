import { useCallback, useState } from "react";
import { act, render, screen } from "@testing-library/react";
import { PatchOperationTree } from "./PatchOperationTree";
import type { PatchOperationNode } from "../../types/patchFile";
import type { PatchOperationNodeRowProps } from "../PatchOperationNodeRow/PatchOperationNodeRow";

// Replaces the real recursive row renderer with a bare-bones stub that stashes the props it was
// last called with, keyed by operation id -- lets this test assert on callback *identity*
// (Plan.md's "operation-id-based dispatcher") without depending on React internals to prove a
// render was skipped.
const capturedProps = new Map<number, PatchOperationNodeRowProps>();
vi.mock("../PatchOperationNodeRow/PatchOperationNodeRow", () => ({
  PatchOperationNodeRow: (props: PatchOperationNodeRowProps) => {
    capturedProps.set(props.node.id, props);
    return <li data-testid={`row-${props.node.id}`}>{props.node.className}</li>;
  },
}));

function op(id: number, xpath: string): PatchOperationNode {
  return {
    id,
    className: "PatchOperationAdd",
    success: "normal",
    attributes: [],
    kind: { type: "add", data: { xpath, valueXml: null, order: null } },
    span: null,
  };
}

function Harness({ initialOps }: { initialOps: PatchOperationNode[] }) {
  const [ops, setOps] = useState(initialOps);
  // Mirrors `usePatchOperationTree.setOperations`'s own stability contract (a `useCallback` with
  // stable deps) -- an inline arrow recreated every render would defeat the very identity
  // stability this test is checking, for a reason unrelated to the row-dispatch logic under test.
  const setOperations = useCallback((updater: (prev: PatchOperationNode[]) => PatchOperationNode[]) => {
    setOps((prev) => updater(prev));
  }, []);
  // Same stability rationale as `setOperations` above -- mirrors `usePatchOperationTree.generateId`.
  const generateId = useCallback(() => 999, []);
  return (
    <PatchOperationTree
      operations={ops}
      catalog={null}
      readOnly={false}
      projectId={null}
      generateId={generateId}
      setOperations={setOperations}
    />
  );
}

describe("PatchOperationTree row isolation", () => {
  beforeEach(() => {
    capturedProps.clear();
  });

  it("keeps sibling rows' node reference and dispatch callbacks stable when only one row is edited", () => {
    render(<Harness initialOps={[op(0, "Defs/A"), op(1, "Defs/B"), op(2, "Defs/C")]} />);

    expect(screen.getByTestId("row-0")).toBeTruthy();
    expect(screen.getByTestId("row-1")).toBeTruthy();
    expect(screen.getByTestId("row-2")).toBeTruthy();

    const before0 = capturedProps.get(0)!;
    const before2 = capturedProps.get(2)!;

    // Edit only row 1 -- via its own captured onChange, mirroring what `PatchPathInput`'s commit
    // boundary would call.
    const row1 = capturedProps.get(1)!;
    act(() => {
      row1.onChange((n) =>
        n.kind.type === "add" ? { ...n, kind: { ...n.kind, data: { ...n.kind.data, xpath: "Defs/B2" } } } : n,
      );
    });

    const after0 = capturedProps.get(0)!;
    const after2 = capturedProps.get(2)!;

    // Untouched rows' `node` objects are the very same reference `replaceAt`/id-map left alone...
    expect(after0.node).toBe(before0.node);
    expect(after2.node).toBe(before2.node);
    // ...and their dispatch callbacks (built from `[setOperations, node.id]` alone) never changed
    // identity either, so a memoized `PatchOperationNodeRow` would see referentially-equal props
    // for every row except the one that actually changed.
    expect(after0.onChange).toBe(before0.onChange);
    expect(after0.onRemove).toBe(before0.onRemove);
    expect(after0.onDuplicate).toBe(before0.onDuplicate);
    expect(after2.onChange).toBe(before2.onChange);
    expect(after2.onRemove).toBe(before2.onRemove);
    expect(after2.onDuplicate).toBe(before2.onDuplicate);
  });

  it("keeps a row's own dispatch callbacks stable across an edit to its own data", () => {
    render(<Harness initialOps={[op(0, "Defs/A"), op(1, "Defs/B")]} />);
    const before = capturedProps.get(0)!;

    act(() => {
      before.onChange((n) =>
        n.kind.type === "add" ? { ...n, kind: { ...n.kind, data: { ...n.kind.data, xpath: "Defs/A2" } } } : n,
      );
    });

    const after = capturedProps.get(0)!;
    expect(after.node).not.toBe(before.node);
    expect(after.onChange).toBe(before.onChange);
    expect(after.onRemove).toBe(before.onRemove);
  });
});
