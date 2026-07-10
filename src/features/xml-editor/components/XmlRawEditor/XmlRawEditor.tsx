import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
} from "react";
import { basicSetup } from "codemirror";
import { Annotation, Compartment, EditorState, Prec } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { xml } from "@codemirror/lang-xml";
import {
  rimEditCodeMirrorTheme,
  rimEditXmlHighlighting,
} from "../../lib/codeMirrorXmlTheme";
import styles from "./XmlRawEditor.module.css";

interface Props {
  value: string;
  onChange: (xml: string) => void;
  readOnly?: boolean;
  onShortcut?: (shortcut: "undo" | "redo" | "save" | "close") => boolean | void;
}

export interface XmlRawEditorHandle {
  readonly view: EditorView | null;
}

// Marks transactions that originate from controlled prop sync, not from the user.
// The update listener skips these so they never reach onChange / updateRawXml.
const syncAnnotation = Annotation.define<true>();

const readOnlyCompartment = new Compartment();

export const XmlRawEditor = forwardRef<XmlRawEditorHandle, Props>(
  function XmlRawEditor(
    { value, onChange, readOnly = false, onShortcut },
    ref,
  ) {
    const hostRef = useRef<HTMLDivElement | null>(null);
    const viewRef = useRef<EditorView | null>(null);
    const onChangeRef = useRef(onChange);
    const valueRef = useRef(value);
    const onShortcutRef = useRef(onShortcut);

    onChangeRef.current = onChange;
    valueRef.current = value;
    onShortcutRef.current = onShortcut;

    useImperativeHandle(
      ref,
      () => ({
        get view() {
          return viewRef.current;
        },
      }),
      [],
    );

    const extensions = useMemo(
      () => [
        // High-precedence keymap must come before basicSetup so our handlers
        // win over CodeMirror's built-in history (undo/redo) and other commands.
        Prec.highest(
          keymap.of([
            {
              key: "Mod-z",
              run: () => Boolean(onShortcutRef.current?.("undo")),
            },
            {
              key: "Mod-y",
              run: () => Boolean(onShortcutRef.current?.("redo")),
            },
            {
              key: "Mod-Shift-z",
              run: () => Boolean(onShortcutRef.current?.("redo")),
            },
            {
              key: "Mod-s",
              run: () => Boolean(onShortcutRef.current?.("save")),
            },
            {
              key: "Mod-w",
              run: () => Boolean(onShortcutRef.current?.("close")),
            },
          ]),
        ),
        basicSetup,
        xml(),
        rimEditCodeMirrorTheme,
        rimEditXmlHighlighting,
        readOnlyCompartment.of([
          EditorState.readOnly.of(readOnly),
          EditorView.editable.of(!readOnly),
        ]),
        EditorView.updateListener.of((update) => {
          if (!update.docChanged) return;
          // Ignore programmatic syncs - only report genuine user edits.
          if (update.transactions.some((tr) => tr.annotation(syncAnnotation)))
            return;
          const nextValue = update.state.doc.toString();
          valueRef.current = nextValue;
          onChangeRef.current(nextValue);
        }),
        EditorView.contentAttributes.of({
          "aria-label": "Raw XML editor",
          spellcheck: "false",
        }),
      ],
      // extensions are stable by design; readOnly changes go through the compartment
      // eslint-disable-next-line react-hooks/exhaustive-deps
      [],
    );

    // Mount the editor once and destroy it on unmount.
    useEffect(() => {
      if (!hostRef.current) return;

      const state = EditorState.create({ doc: value, extensions });
      const view = new EditorView({ state, parent: hostRef.current });
      viewRef.current = view;

      return () => {
        view.destroy();
        viewRef.current = null;
      };
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [extensions]);

    // Sync external value changes (undo, redo, form edits) into CodeMirror.
    // Tagged with syncAnnotation so the update listener does not treat this as
    // a user edit, preventing spurious onChange calls that would corrupt redo
    // history and trigger unnecessary parse debounces.
    useEffect(() => {
      const view = viewRef.current;
      if (!view) return;
      const current = view.state.doc.toString();
      if (current === value) return;
      view.dispatch({
        changes: { from: 0, to: current.length, insert: value },
        annotations: syncAnnotation.of(true),
      });
    }, [value]);

    // Reconfigure read-only without recreating the editor.
    useEffect(() => {
      const view = viewRef.current;
      if (!view) return;
      view.dispatch({
        effects: readOnlyCompartment.reconfigure([
          EditorState.readOnly.of(readOnly),
          EditorView.editable.of(!readOnly),
        ]),
      });
    }, [readOnly]);

    return <div ref={hostRef} className={styles.editor} />;
  },
);
