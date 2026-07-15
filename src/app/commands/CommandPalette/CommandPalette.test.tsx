import { render, screen, fireEvent } from "@testing-library/react";
import { FileText } from "lucide-react";
import { createI18nInstance } from "../../../i18n";
import { LocaleProvider } from "../../../i18n/LocaleProvider";
import { renderWithI18n } from "../../../i18n/testing/renderWithI18n";
import { CommandPalette } from "./CommandPalette";
import type { CommandAction } from "../commandTypes";

function makeCommands(overrides: Partial<CommandAction>[] = []): CommandAction[] {
  const base: CommandAction[] = [
    {
      id: "open-project",
      labelKey: "shell:commands.openProject.label",
      keywordsKey: "shell:commands.openProject.keywords",
      icon: FileText,
      run: vi.fn(),
    },
    {
      id: "refresh",
      labelKey: "shell:commands.refresh.label",
      keywordsKey: "shell:commands.refresh.keywords",
      icon: FileText,
      run: vi.fn(),
    },
  ];
  return overrides.length > 0 ? overrides.map((o, i) => ({ ...base[i], ...o })) : base;
}

describe("CommandPalette", () => {
  it("renders translated labels for each command", () => {
    renderWithI18n(
      <CommandPalette open onClose={vi.fn()} commands={makeCommands()} />,
    );
    expect(screen.getByText("Open Project")).toBeDefined();
    expect(screen.getByText("Refresh Project Files")).toBeDefined();
  });

  it("filters commands by translated keyword text", () => {
    renderWithI18n(
      <CommandPalette open onClose={vi.fn()} commands={makeCommands()} />,
    );
    fireEvent.change(screen.getByRole("textbox", { name: "Search commands" }), {
      target: { value: "scan" },
    });
    expect(screen.getByText("Refresh Project Files")).toBeDefined();
    expect(screen.queryByText("Open Project")).toBeNull();
  });

  it("shows the translated empty state when nothing matches", () => {
    renderWithI18n(
      <CommandPalette open onClose={vi.fn()} commands={makeCommands()} />,
    );
    fireEvent.change(screen.getByRole("textbox", { name: "Search commands" }), {
      target: { value: "zzz-no-match" },
    });
    expect(screen.getByText("No matching commands")).toBeDefined();
  });

  it("re-renders a command's visible label from its key when the active locale's resources change, without remounting", () => {
    const i18nInstance = createI18nInstance();
    // Same `commands` array reference across both renders (as `AppShell`'s memoized command
    // list would be across an unrelated re-render) -- only the i18next resources/language
    // change below, so this exercises the memo's explicit `i18n.language` dependency rather
    // than a prop-reference change forcing recomputation anyway.
    const commands = makeCommands();
    const { rerender } = render(
      <LocaleProvider i18nInstance={i18nInstance}>
        <CommandPalette open onClose={vi.fn()} commands={commands} />
      </LocaleProvider>,
    );
    expect(screen.getByText("Open Project")).toBeDefined();

    // Simulate what a locale switch delivers: new resource text for the same stable key. If
    // the component cached rendered labels instead of re-deriving them from `labelKey` on
    // every render, this would still show the old English text.
    i18nInstance.addResourceBundle(
      "en",
      "shell",
      { commands: { openProject: { label: "Open Project (updated)" } } },
      true,
      true,
    );
    void i18nInstance.changeLanguage("en");

    rerender(
      <LocaleProvider i18nInstance={i18nInstance}>
        <CommandPalette open onClose={vi.fn()} commands={commands} />
      </LocaleProvider>,
    );

    expect(screen.getByText("Open Project (updated)")).toBeDefined();
    expect(screen.queryByText("Open Project")).toBeNull();
  });
});
