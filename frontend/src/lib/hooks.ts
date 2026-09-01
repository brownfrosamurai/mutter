import { useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { commands } from "@/lib/bindings";

/** Shared by the dashboard's Settings panel and the onboarding window's
 * Permissions step (review finding: this query + its window-`focus`
 * refetch were duplicated verbatim between the two). A Grant click (mic's
 * native prompt, or Accessibility/Screen Recording's System Settings
 * deep-link) resolves outside the window's own control — refetching on
 * `focus` catches the user returning from System Settings, or the native
 * prompt's own dialog closing, so a stale one-shot fetch doesn't keep
 * showing "not granted" after they actually granted it. Native window
 * focus, not TanStack Query's `refetchOnWindowFocus` (disabled globally in
 * both windows' `main.tsx` and keyed off document visibility, not the OS
 * window). */
export function usePermissionsQuery() {
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: ["permissions"],
    queryFn: () => commands.getPermissionStatus(),
  });

  useEffect(() => {
    const refetch = () => void queryClient.invalidateQueries({ queryKey: ["permissions"] });
    window.addEventListener("focus", refetch);
    return () => window.removeEventListener("focus", refetch);
  }, [queryClient]);

  return query;
}
