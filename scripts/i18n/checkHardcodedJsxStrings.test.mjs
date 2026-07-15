import { describe, it, expect } from "vitest";
import { containsLetter, findHardcodedJsxStrings } from "./checkHardcodedJsxStrings.mjs";

describe("containsLetter", () => {
  it("is true for text with a letter", () => {
    expect(containsLetter("Hello")).toBe(true);
  });

  it("is false for pure punctuation/whitespace", () => {
    expect(containsLetter(" | ")).toBe(false);
    expect(containsLetter("•")).toBe(false);
    expect(containsLetter("  \n  ")).toBe(false);
  });

  it("is false for pure numbers", () => {
    expect(containsLetter("42")).toBe(false);
  });
});

describe("findHardcodedJsxStrings", () => {
  it("flags bare JSX text", () => {
    const source = `
      export function C() {
        return <div>Hello world</div>;
      }
    `;
    const violations = findHardcodedJsxStrings("fixture.tsx", source);
    expect(violations).toHaveLength(1);
    expect(violations[0].kind).toBe("jsx-text");
    expect(violations[0].text).toBe("Hello world");
  });

  it("does not flag text rendered through t(...)", () => {
    const source = `
      export function C() {
        const { t } = useTranslation();
        return <div>{t("shell:hello")}</div>;
      }
    `;
    expect(findHardcodedJsxStrings("fixture.tsx", source)).toEqual([]);
  });

  it("does not flag whitespace-only or punctuation-only JSX text", () => {
    const source = `
      export function C() {
        return (
          <div>
            <span>{"a"}</span> | <span>{"b"}</span>
          </div>
        );
      }
    `;
    expect(findHardcodedJsxStrings("fixture.tsx", source)).toEqual([]);
  });

  it("flags a bare user-facing attribute string literal", () => {
    const source = `
      export function C() {
        return <button title="Click me" />;
      }
    `;
    const violations = findHardcodedJsxStrings("fixture.tsx", source);
    expect(violations).toHaveLength(1);
    expect(violations[0].kind).toBe("jsx-attribute:title");
    expect(violations[0].text).toBe("Click me");
  });

  it("does not flag technical/non-user-facing attributes", () => {
    const source = `
      export function C() {
        return <div className="Panel" data-testid="Panel" id="Panel" />;
      }
    `;
    expect(findHardcodedJsxStrings("fixture.tsx", source)).toEqual([]);
  });

  it("does not flag a user-facing attribute driven by an expression (t(...) or a variable)", () => {
    const source = `
      export function C() {
        const { t } = useTranslation();
        return <button title={t("shell:openProject")} aria-label={label} />;
      }
    `;
    expect(findHardcodedJsxStrings("fixture.tsx", source)).toEqual([]);
  });

  it("reports 1-indexed line/column positions", () => {
    const source = `export function C() {
  return <div>Hi</div>;
}
`;
    const violations = findHardcodedJsxStrings("fixture.tsx", source);
    expect(violations).toHaveLength(1);
    expect(violations[0].line).toBe(2);
  });
});
