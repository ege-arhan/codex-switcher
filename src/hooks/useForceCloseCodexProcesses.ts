import { t } from "../lib/i18n";
import { useCallback, useState } from "react";
import type { CodexProcessInfo } from "../types";
import { invokeBackend } from "../lib/platform";

interface KillCodexProcessesResult {
  targeted_count: number;
  killed_pids: number[];
  failed_pids: number[];
  reopen_token?: string | null;
}

interface UseForceCloseCodexProcessesOptions {
  processCount: number;
  checkProcesses: () => Promise<CodexProcessInfo | null>;
  showToast: (message: string, isError?: boolean) => void;
  formatError: (err: unknown) => string;
}

export function useForceCloseCodexProcesses({
  processCount,
  checkProcesses,
  showToast,
  formatError,
}: UseForceCloseCodexProcessesOptions) {
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [isForceClosing, setIsForceClosing] = useState(false);

  const closeCodexProcesses = useCallback(async (reopenDesktop = false, forceClose = false) => {
    try {
      setIsForceClosing(true);

      const result = await invokeBackend<KillCodexProcessesResult>(
        "kill_codex_processes",
        { reopenDesktop, forceClose }
      );
      const latestProcessInfo = await checkProcesses();
      const remainingCount = latestProcessInfo?.count ?? processCount;
      const closedCount = Math.max(0, processCount - remainingCount);

      if (!latestProcessInfo) {
        showToast(t("Could not verify that Codex closed. Account switching and reopening were skipped."), true);
      } else if (result.targeted_count === 0) {
        showToast(t("No running Codex processes found."));
      } else if (remainingCount === 0) {
        showToast(
          t(forceClose ? "Force closed {count} Codex sessions." : "Closed {count} Codex sessions.", { count: processCount })
        );
      } else if (closedCount > 0) {
        showToast(
          t(forceClose ? "Force closed {closed}/{total} Codex sessions. {remaining} still running." : "Closed {closed}/{total} Codex sessions. {remaining} still running.", { closed: closedCount, total: processCount, remaining: remainingCount }),
          true
        );
      } else {
        showToast(
          t(forceClose ? "Could not force close {count} Codex sessions." : "Could not gracefully close {count} Codex sessions.", { count: remainingCount }),
          true
        );
      }

      return { processInfo: latestProcessInfo, reopenToken: result.reopen_token ?? null };
    } catch (err) {
      console.error("Failed to close Codex processes:", err);
      showToast(t("Close failed: {error}", { error: formatError(err) }), true);
      return null;
    } finally {
      setConfirmOpen(false);
      setIsForceClosing(false);
    }
  }, [checkProcesses, formatError, processCount, showToast]);

  return {
    forceCloseConfirmOpen: confirmOpen,
    setForceCloseConfirmOpen: setConfirmOpen,
    isForceClosingCodex: isForceClosing,
    closeCodexProcesses,
  };
}
