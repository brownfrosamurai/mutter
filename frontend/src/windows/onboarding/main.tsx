import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Onboarding } from "./Onboarding";
import "@/styles/globals.css";

const queryClient = new QueryClient({
  defaultOptions: { queries: { refetchOnWindowFocus: false } },
});

// Deliberately NOT wrapped in <StrictMode> — see pill/main.tsx's
// precedent (pre-landing review, 2026-09-01, red-team finding): Ready.tsx's
// permission-request effect has the same async-effect-plus-cancellation
// hazard StrictMode's dev-mode double-invoke breaks. The double-invoke's
// synchronous mount→cleanup→mount left `startedRef` already set on the
// second mount (so no new request sequence starts) while the first mount's
// in-flight promise resolved into a `cancelled` closure that skips
// `setPhase("resolved")`/`onBusyChange(false)` — the Ready screen got
// permanently stuck on "Setting things up" in every dev build.
// eslint-disable-next-line @typescript-eslint/no-non-null-assertion
createRoot(document.getElementById("root")!).render(
  <QueryClientProvider client={queryClient}>
    <Onboarding />
  </QueryClientProvider>,
);
