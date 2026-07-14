// Shared "request a view switch, prompting if an override is dirty" behavior (Plan.md section 8
// step 6), used identically by `FormViewSelector`'s dropdown/"Show full form" action and
// `FormViewManagerDialog`'s per-row "Use this view" action -- factored out once rather than
// duplicated in both call sites.
import { useState } from "react";
import type { UseFormViewsResult } from "./useFormViews";
import type { SelectedFormViewRef } from "../types/formViews";
import { FormViewSwitchConfirmDialog } from "../components/FormViewSwitchConfirmDialog/FormViewSwitchConfirmDialog";

export interface UseViewSwitchConfirmationResult {
  /** Switches immediately if there's no dirty override, otherwise opens the three-way
   * discard/save-as-custom/cancel confirmation. */
  requestSwitch: (ref: SelectedFormViewRef) => void;
  /** Render this wherever the confirmation should appear -- `null` when no switch is pending. */
  switchConfirmDialog: React.ReactNode;
}

export function useViewSwitchConfirmation(
  controller: UseFormViewsResult,
): UseViewSwitchConfirmationResult {
  const [pendingRef, setPendingRef] = useState<SelectedFormViewRef | null>(null);

  function requestSwitch(ref: SelectedFormViewRef) {
    if (controller.hasDirtyOverride) {
      setPendingRef(ref);
      return;
    }
    controller.selectView(ref);
  }

  const switchConfirmDialog = pendingRef ? (
    <FormViewSwitchConfirmDialog
      hiddenCount={controller.hiddenCount}
      onCancel={() => setPendingRef(null)}
      onDiscardAndSwitch={() => {
        controller.selectView(pendingRef);
        setPendingRef(null);
      }}
      onSaveAsCustom={async (name) => {
        // Captured before the async call -- see `getScopeKey`'s doc comment on
        // `UseFormViewsResult`: if the scope changes (a Def/tab/game-version switch) while this
        // save is in flight, this closure's `controller` is now stale, and calling
        // `controller.selectView(pendingRef)` from it would still fire a real persist attempt
        // scoped to the OLD Def/view -- the hook's own guards stop it from corrupting the NEW
        // scope's in-memory selection, but not from firing (and potentially misattributing a
        // warning) in the first place.
        const startScopeKey = controller.getScopeKey();
        await controller.saveOverrideAsCustomView(name);
        setPendingRef(null);
        if (controller.getScopeKey() !== startScopeKey) {
          // The override WAS saved as a custom view (correctly, under its original scope), but
          // completing "the switch the user actually asked for" no longer means anything: that
          // request was for a view/scope combination that isn't the one active anymore.
          return;
        }
        // `saveOverrideAsCustomView` selects the freshly created custom view as a side effect;
        // complete the switch the user actually asked for.
        controller.selectView(pendingRef);
      }}
    />
  ) : null;

  return { requestSwitch, switchConfirmDialog };
}
