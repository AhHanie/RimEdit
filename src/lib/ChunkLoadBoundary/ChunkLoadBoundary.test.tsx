import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderWithI18n as render } from "../../i18n/testing/renderWithI18n";
import { ChunkLoadBoundary } from "./ChunkLoadBoundary";

interface LoadedProps {
  label: string;
}

function Loaded({ label }: LoadedProps) {
  return <div data-testid="loaded">{label}</div>;
}

describe("ChunkLoadBoundary", () => {
  it("shows the loading fallback, then the resolved component", async () => {
    const factory = () => Promise.resolve({ default: Loaded });
    render(
      <ChunkLoadBoundary
        factory={factory}
        componentProps={{ label: "hello" }}
        loadingFallback={<div data-testid="fallback" />}
      />,
    );

    expect(screen.getByTestId("fallback")).toBeDefined();
    expect((await screen.findByTestId("loaded")).textContent).toBe("hello");
  });

  it("shows an error banner with a retry affordance when the dynamic import rejects", async () => {
    const user = userEvent.setup();
    let attempts = 0;
    const factory = () => {
      attempts++;
      if (attempts === 1) return Promise.reject(new Error("boom"));
      return Promise.resolve({ default: Loaded });
    };

    render(
      <ChunkLoadBoundary
        factory={factory}
        componentProps={{ label: "recovered" }}
        loadingFallback={<div data-testid="fallback" />}
      />,
    );

    expect(await screen.findByRole("alert")).toBeDefined();
    expect(screen.queryByTestId("loaded")).toBeNull();

    await user.click(screen.getByRole("button", { name: "Retry" }));

    expect((await screen.findByTestId("loaded")).textContent).toBe("recovered");
    expect(attempts).toBe(2);
  });

  it("calls a shared factory only once for two instances mounted concurrently with the same factory", async () => {
    let calls = 0;
    const factory = () => {
      calls++;
      return Promise.resolve({ default: Loaded });
    };
    const fallback = <div data-testid="fallback" />;

    render(
      <>
        <ChunkLoadBoundary factory={factory} componentProps={{ label: "a" }} loadingFallback={fallback} />
        <ChunkLoadBoundary factory={factory} componentProps={{ label: "b" }} loadingFallback={fallback} />
      </>,
    );

    const loaded = await screen.findAllByTestId("loaded");
    expect(loaded.map((el) => el.textContent).sort()).toEqual(["a", "b"]);
    expect(calls).toBe(1);
  });

  it("still resolves a freshly mounted sibling instance after another instance using the same factory has retried", async () => {
    const user = userEvent.setup();
    let attempts = 0;
    const factory = () => {
      attempts++;
      // Only the very first call (instance A's initial attempt) fails; every later call
      // (A's retry, and B's fresh mount) succeeds.
      if (attempts === 1) return Promise.reject(new Error("boom"));
      return Promise.resolve({ default: Loaded });
    };
    const fallback = <div data-testid="fallback" />;

    const { unmount } = render(
      <ChunkLoadBoundary factory={factory} componentProps={{ label: "a" }} loadingFallback={fallback} />,
    );
    expect(await screen.findByRole("alert")).toBeDefined();
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect((await screen.findByTestId("loaded")).textContent).toBe("a");
    unmount();

    // A fresh instance B mounts after A already bumped the shared cache entry to a later
    // `attempt` -- B's own local `attempt` starts back at 0, so it must not reuse A's
    // (already-settled, but keyed to a different attempt) lazy wrapper; it should get its own
    // working component rather than getting stuck.
    render(
      <ChunkLoadBoundary factory={factory} componentProps={{ label: "b" }} loadingFallback={fallback} />,
    );
    expect((await screen.findByTestId("loaded")).textContent).toBe("b");
    expect(attempts).toBe(3);
  });
});
