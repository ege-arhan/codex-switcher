// Translate known product messages while preserving unknown diagnostic details.
export function localizeProductMessage(message: string, t: (source: string, params?: Record<string, string | number>) => string): string {
  const patterns: Array<[RegExp, (match: RegExpMatchArray) => string]> = [
    [/^Cannot switch accounts while (\d+) Codex process(?: is|es are) running$/, m => t("Cannot switch accounts while {count} Codex process(es) are running. Close Codex first.", { count: m[1] })],
    [/^Switched to '(.*)' and started continuations in (\d+) desktop sessions$/, m => t("Switched to '{name}' and started continuations in {count} desktop sessions", { name: m[1], count: m[2] })],
    [/^Switched to account '(.*)' and (resumed session in-place|relaunched session) with '(.*)'$/, m => t(m[2] === "resumed session in-place" ? "Switched to account '{name}' and resumed session in-place with '{phrase}'" : "Switched to account '{name}' and relaunched session with '{phrase}'", { name: m[1], phrase: m[3] })],
    [/^(Resumed session in-place|Relaunched session) with '(.*)' (after reset credit|on newly active account) \(switched (\d+)s ago\)$/, m => t(m[1] === "Resumed session in-place" ? "Resumed session in-place with '{phrase}' {reason} (switched {seconds}s ago)" : "Relaunched session with '{phrase}' {reason} (switched {seconds}s ago)", { phrase: m[2], reason: t(m[3]), seconds: m[4] })],
    [/^Auto-redeemed reset credit on active account '(.*)' \(limits refreshed\). (Resumed session in-place|Relaunched session) with '(.*)'\.$/, m => t(m[2] === "Resumed session in-place" ? "Auto-redeemed reset credit on active account '{name}' (limits refreshed). Resumed session in-place with '{phrase}'." : "Auto-redeemed reset credit on active account '{name}' (limits refreshed). Relaunched session with '{phrase}'.", { name: m[1], phrase: m[3] })],
    [/^Server overloaded\. Sent automatic retry \(attempt (\d+)\/(\d+)\) with '(.*)'$/, m => t("Server overloaded. Sent automatic retry (attempt {attempt}/{max}) with '{phrase}'", { attempt: m[1], max: m[2], phrase: m[3] })],
    [/^Automatic recovery failed: (.*)$/s, m => t("Automatic recovery failed: {error}", { error: m[1] })],
    [/^Claude usage check failed \(HTTP (\d+)\)$/, m => t("Claude usage check failed (HTTP {status})", { status: m[1] })],
    [/^Cursor request returned HTTP (\d+)$/, m => t("Cursor request returned HTTP {status}", { status: m[1] })],
    [/^Cursor login returned HTTP (\d+)$/, m => t("Cursor login returned HTTP {status}", { status: m[1] })],
  ];
  for (const [pattern, format] of patterns) { const match = message.match(pattern); if (match) return format(match); }
  return t(message);
}
