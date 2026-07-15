import { AppShell } from "./shell/AppShell/AppShell";
import type { ProjectSettingsLoadResult } from "../features/project-settings";

interface AppProps {
  /** Forwarded to `AppShell` -- see `main.tsx` and `AppShell`'s `initialProjectSettingsPromise`
   * doc comment. */
  initialProjectSettingsPromise?: Promise<ProjectSettingsLoadResult>;
}

function App({ initialProjectSettingsPromise }: AppProps = {}) {
  return (
    <AppShell initialProjectSettingsPromise={initialProjectSettingsPromise} />
  );
}

export default App;
