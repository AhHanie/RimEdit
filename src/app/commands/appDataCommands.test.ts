import { invoke } from "@tauri-apps/api/core";
import { openAppDataFolder } from "./appDataCommands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

afterEach(() => {
  vi.clearAllMocks();
});

describe("openAppDataFolder", () => {
  it("invokes the open_app_data_folder command with no arguments", async () => {
    invokeMock.mockResolvedValue(undefined);

    await openAppDataFolder();

    expect(invokeMock).toHaveBeenCalledOnce();
    expect(invokeMock).toHaveBeenCalledWith("open_app_data_folder");
  });

  it("propagates a rejected invoke call to the caller", async () => {
    const error = new Error("boom");
    invokeMock.mockRejectedValue(error);

    await expect(openAppDataFolder()).rejects.toThrow("boom");
  });
});
