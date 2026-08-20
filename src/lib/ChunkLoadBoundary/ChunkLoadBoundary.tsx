import {
  Component,
  Suspense,
  lazy,
  useMemo,
  useState,
  type ComponentType,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { RefreshCw } from "lucide-react";
import styles from "./ChunkLoadBoundary.module.css";

interface ChunkErrorBoundaryProps {
  children: ReactNode;
  fallback: (retry: () => void) => ReactNode;
}

interface ChunkErrorBoundaryState {
  hasError: boolean;
}

class ChunkErrorBoundary extends Component<
  ChunkErrorBoundaryProps,
  ChunkErrorBoundaryState
> {
  state: ChunkErrorBoundaryState = { hasError: false };

  static getDerivedStateFromError(): ChunkErrorBoundaryState {
    return { hasError: true };
  }

  componentDidCatch(error: unknown) {
    console.error("Failed to load a deferred UI chunk:", error);
  }

  private retry = () => this.setState({ hasError: false });

  render() {
    if (this.state.hasError) return this.props.fallback(this.retry);
    return this.props.children;
  }
}

export interface ChunkLoadBoundaryProps<P extends object> {
  /** Stable (module-scope) dynamic import factory -- an inline arrow recreated every render
   * would defeat the `attempt`-keyed memoization below and refetch the chunk on every render. */
  factory: () => Promise<{ default: ComponentType<P> }>;
  componentProps: P;
  loadingFallback: ReactNode;
}

/** Caches one `lazy()` wrapper per factory, shared across every `ChunkLoadBoundary` instance
 * using that same factory (e.g. one `XmlRawEditor` boundary per open tab) so `factory()` -- and
 * therefore the underlying `import()` -- runs once rather than once per mounted instance. Each
 * cache entry is tagged with the `attempt` it was created for so a retry on one instance creates
 * a fresh entry without disturbing instances that haven't retried. */
const lazyComponentCache = new WeakMap<
  ChunkLoadBoundaryProps<object>["factory"],
  { attempt: number; component: unknown }
>();

function getOrCreateLazy<P extends object>(
  factory: ChunkLoadBoundaryProps<P>["factory"],
  attempt: number,
): ComponentType<P> {
  const key = factory as ChunkLoadBoundaryProps<object>["factory"];
  const cached = lazyComponentCache.get(key);
  if (cached && cached.attempt === attempt) return cached.component as ComponentType<P>;
  const created = lazy(factory);
  lazyComponentCache.set(key, { attempt, component: created });
  return created;
}

/** Wraps a `React.lazy` component with a Suspense fallback and an error boundary, so a failed
 * dynamic import (e.g. a stale deploy's chunk 404ing) surfaces an inline retry affordance instead
 * of crashing the app or leaving a permanent blank pane. A rejected `lazy()` promise never
 * re-fetches on retry, so retrying recreates the `lazy()` wrapper (and therefore issues a fresh
 * `import()`) via the `attempt` counter rather than reusing the same one. */
export function ChunkLoadBoundary<P extends object>({
  factory,
  componentProps,
  loadingFallback,
}: ChunkLoadBoundaryProps<P>) {
  const { t } = useTranslation("shell");
  const [attempt, setAttempt] = useState(0);
  const LazyComponent = useMemo(() => getOrCreateLazy(factory, attempt), [factory, attempt]);

  return (
    <ChunkErrorBoundary
      fallback={(retryBoundary) => (
        <div className={styles.errorBanner} role="alert">
          <span className={styles.errorMessage}>{t("chunkLoad.failed")}</span>
          <button
            className="icon-btn"
            style={{ width: 20, height: 20, flexShrink: 0 }}
            onClick={() => {
              retryBoundary();
              setAttempt((a) => a + 1);
            }}
            aria-label={t("chunkLoad.retry")}
            title={t("chunkLoad.retry")}
          >
            <RefreshCw size={12} />
          </button>
        </div>
      )}
    >
      <Suspense fallback={loadingFallback}>
        <LazyComponent {...componentProps} />
      </Suspense>
    </ChunkErrorBoundary>
  );
}
