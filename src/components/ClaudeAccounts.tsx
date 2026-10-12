import { useCallback, useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { RateLimitBar } from "./UsageBar";
import { useI18n, localizeMessage } from "../lib/i18n";
import { invokeBackend, isTauriRuntime, openExternalUrl } from "../lib/platform";
interface ClaudeUsage {
    five_hour_used: number;
    seven_day_used: number;
    five_hour_resets_at: string | null;
    seven_day_resets_at: string | null;
    checked_at: number;
}
interface ClaudeProfile {
    id: string;
    email: string;
    is_default: boolean;
    logged_in: boolean;
    last_warmup_at: number | null;
    desktop_status: "saved" | "login_required" | "different_account" | "unavailable";
    usage: ClaudeUsage | null;
    error: string | null;
}
interface ClaudeProfiles {
    profiles: ClaudeProfile[];
    active_code_id: string | null;
    active_desktop_id: string | null;
    auto_switch_enabled: boolean;
    auto_warmup_enabled: boolean;
}
interface ClaudeLogin {
    flow_id: string;
    profile_id: string;
    auth_url: string | null;
    status: string;
    error: string | null;
}
const button = "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#d97757] rounded-lg border border-gray-300 px-3 py-2 text-sm font-medium text-gray-800 hover:bg-gray-100 disabled:opacity-50 dark:border-gray-600 dark:text-gray-100 dark:hover:bg-gray-800";
export function ClaudeAccounts() {
    const { t, locale } = useI18n();
    const [returnInput, setReturnInput] = useState("");
    const [store, setStore] = useState<ClaudeProfiles | null>(null);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [email, setEmail] = useState("");
    const [login, setLogin] = useState<ClaudeLogin | null>(null);
    const [adding, setAdding] = useState(false);
    const [desktopTarget, setDesktopTarget] = useState<ClaudeProfile | null>(null);
    const [cwd, setCwd] = useState(() => localStorage.getItem("claude-project-directory") || "");
    const [copied, setCopied] = useState(false);
    const dialog = useRef<HTMLDialogElement>(null);
    const desktop = isTauriRuntime();
    const mac = /Mac/i.test(navigator.userAgent);
    const load = useCallback(async () => {
        try {
            setStore(await invokeBackend<ClaudeProfiles>("list_claude_profiles"));
        }
        catch (e) {
            setError(String(e));
        }
    }, []);
    useEffect(() => {
        if (!desktop)
            return;
        void load();
        const reload = () => { void load(); };
        window.addEventListener("focus", reload);
        return () => window.removeEventListener("focus", reload);
    }, [desktop, load]);
    useEffect(() => {
        if (adding || desktopTarget)
            dialog.current?.showModal();
        else
            dialog.current?.close();
    }, [adding, desktopTarget]);
    useEffect(() => {
        if (login?.status !== "pending")
            return;
        let disposed = false;
        let timer: ReturnType<typeof setTimeout>;
        const poll = async () => {
            try {
                const next = await invokeBackend<ClaudeLogin | null>("get_claude_login");
                if (disposed)
                    return;
                setLogin(next);
                if (next?.status === "complete") {
                    await invokeBackend("refresh_claude_profiles");
                    await load();
                    return;
                }
                if (next?.error)
                    setError(next.error);
                if (next?.status === "pending")
                    timer = setTimeout(poll, 1000);
            }
            catch (e) {
                if (!disposed)
                    setError(String(e));
            }
        };
        timer = setTimeout(poll, 500);
        return () => { disposed = true; clearTimeout(timer); };
    }, [login?.status, load]);
    const action = async (command: string, args?: Record<string, unknown>) => {
        setBusy(true);
        setError(null);
        try {
            await invokeBackend<ClaudeProfiles>(command, args);
            await load();
            return true;
        }
        catch (e) {
            setError(String(e));
            return false;
        }
        finally {
            setBusy(false);
        }
    };
    const startLogin = async () => {
        setBusy(true);
        setError(null);
        setCopied(false);
        setReturnInput("");
        try {
            setLogin(await invokeBackend<ClaudeLogin>("start_claude_login", { email }));
        }
        catch (e) {
            setError(String(e));
        }
        finally {
            setBusy(false);
        }
    };
    const submitReturn = async () => {
        if (!login)
            return;
        setBusy(true);
        setError(null);
        try {
            await invokeBackend("complete_claude_login", { flowId: login.flow_id, input: returnInput });
            setReturnInput("");
        }
        catch (e) {
            setError(String(e));
        }
        finally {
            setBusy(false);
        }
    };
    useEffect(() => {
        if (!desktop)
            return;
        let stop: (() => void) | undefined;
        let disposed = false;
        void import("@tauri-apps/api/event").then(async ({ listen }) => {
            const unlisten = await listen<{
                error?: string;
            }>("claude-warmup-event", event => { void load(); if (event.payload.error)
                setError(event.payload.error); });
            if (disposed)
                unlisten();
            else
                stop = unlisten;
        });
        return () => { disposed = true; stop?.(); };
    }, [desktop, load]);
    const closeDialog = async () => {
        if (login?.status === "pending")
            await invokeBackend("cancel_claude_login").catch((e) => setError(String(e)));
        setLogin(null);
        setAdding(false);
        setDesktopTarget(null);
        setCopied(false);
        setReturnInput("");
        void load();
    };
    const chooseProject = async () => {
        setError(null);
        try {
            const selected = await open({ directory: true, multiple: false, title: t("Choose project folder") });
            if (!selected || Array.isArray(selected))
                return;
            localStorage.setItem("claude-project-directory", selected);
            setCwd(selected);
        }
        catch (e) {
            setError(String(e));
        }
    };
    const openCode = async (p: ClaudeProfile) => {
        if (await action("open_claude_code", { profileId: p.id, cwd: cwd || null })) {
            if (adding)
                void closeDialog();
        }
    };
    const resetTime = (value: string | null) => {
        const seconds = value ? Date.parse(value) / 1000 : NaN;
        return Number.isFinite(seconds) ? seconds : null;
    };
    const loginProfile = store?.profiles.find(p => p.id === login?.profile_id);
    if (!desktop)
        return null;
    return (<section aria-labelledby="claude-accounts-title" className="mb-8 rounded-xl border border-[#d97757]/60 bg-white p-5 dark:border-[#d97757]/70 dark:bg-gray-900">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 id="claude-accounts-title" className="text-lg font-semibold text-[#a54f36] dark:text-[#e6a28a]">{t("Claude accounts")}</h2>
          <p className="text-sm text-gray-600 dark:text-gray-300">{t("Manage Claude Code and Claude Desktop sign-ins.")}</p>
        </div>
        <div className="flex flex-wrap gap-2">
          <button className={button} disabled={busy} onClick={() => void action("import_current_claude_profile")}>{t("Import current account")}</button>
          <button className={button} disabled={busy} onClick={() => void action("refresh_claude_profiles")}>{t("Refresh usage")}</button>
          <button className={button} disabled={busy || !store} onClick={() => { setError(null); setEmail(""); setLogin(null); setAdding(true); }}>{t("Add account")}</button>
        </div>
      </div>
      <label className="mt-4 flex items-center gap-2 text-sm font-medium text-gray-900 dark:text-gray-100">
        <input type="checkbox" checked={store?.auto_switch_enabled ?? false} disabled={busy || !store} onChange={(e) => void action("set_claude_auto_switch", { enabled: e.target.checked })}/>
        {t("Claude Code auto-switch")}
      </label>
      <label className="mt-3 flex items-center gap-2 text-sm font-medium text-gray-900 dark:text-gray-100">
        <input type="checkbox" checked={store?.auto_warmup_enabled ?? false} disabled={busy || !store} onChange={e => void action("set_claude_auto_warmup", { enabled: e.target.checked })}/>
        {t("Claude auto warm-up")}
      </label>
      <p className="mt-1 text-xs text-gray-600 dark:text-gray-300">{t("Warm-up sends a minimal request when an unused usage window resets.")}</p>
      <div className="mt-3 flex flex-wrap items-center gap-2 text-xs text-gray-600 dark:text-gray-300">
        <span className="break-all">{t("Project: {directory}", { directory: cwd || t("Home directory") })}</span>
        <button className={button} disabled={busy} onClick={() => void chooseProject()}>{t("Choose project folder")}</button>
        {cwd && <button className={button} disabled={busy} onClick={() => { localStorage.removeItem("claude-project-directory"); setCwd(""); }}>{t("Home directory")}</button>}
      </div>
      <p className="mt-2 text-xs text-gray-600 dark:text-gray-300">{t("When a managed terminal reaches its limit, switch to an account with verified remaining usage and resume the same local chat. Desktop cloud chats stay with their account.")}</p>
      {error && <p role="alert" className="mt-3 rounded-lg bg-red-50 p-3 text-sm text-red-800 dark:bg-red-950 dark:text-red-200">{localizeMessage(error)}</p>}
      {store?.profiles.length === 0 && <p className="mt-4 text-sm text-gray-600 dark:text-gray-300">{t("Import your current Claude Code account, then generate a login link for another account.")}</p>}
      <div className="mt-4 grid gap-4 sm:grid-cols-2">
        {store?.profiles.map((p) => (<article key={p.id} className={`rounded-xl border bg-white p-5 dark:bg-gray-900 ${store.active_code_id === p.id ? "border-[#d97757]" : "border-gray-200 dark:border-gray-700"}`}>
            <h3 className="break-all font-semibold text-gray-900 dark:text-gray-100">{p.email}</h3>
            <p className="mt-1 text-xs text-gray-600 dark:text-gray-300">
              {p.logged_in ? t("Code sign-in verified") : t("Code sign-in required")}
              {store.active_code_id === p.id && t(" \u00B7 Selected Code account")}
              {store.active_desktop_id === p.id && t(" \u00B7 Selected Desktop profile")}
            </p>
            <p className="mt-1 text-xs text-gray-600 dark:text-gray-300">
              {p.desktop_status === "saved" ? t("Desktop: saved account profile") : p.desktop_status === "different_account" ? t("Desktop: signed in with another account") : t("Desktop: separate sign-in required")}
            </p>
            <div className="mt-3 space-y-2">
              {p.usage ? <>
                <RateLimitBar label={t("Session limit")} windowMinutes={300} usedPercent={p.usage.five_hour_used} resetsAt={resetTime(p.usage.five_hour_resets_at)} tone="claude"/>
                <RateLimitBar label={t("Weekly limit")} windowMinutes={10080} usedPercent={p.usage.seven_day_used} resetsAt={resetTime(p.usage.seven_day_resets_at)} tone="claude"/>
              </> : <p className="text-xs text-gray-500 dark:text-gray-400">{t("Remaining usage has not been verified yet")}</p>}
            </div>
            {p.usage && <p className="mt-1 text-xs text-gray-600 dark:text-gray-300">{t("Last checked: {time}", { time: new Date(p.usage.checked_at * 1000).toLocaleTimeString(locale) })}</p>}
            {p.last_warmup_at && <p className="mt-1 text-xs text-gray-600 dark:text-gray-300">{t("Last warm-up: {time}", { time: new Date(p.last_warmup_at * 1000).toLocaleString(locale) })}</p>}
            {p.error && <p className="mt-2 text-xs text-red-700 dark:text-red-300">{localizeMessage(p.error)}</p>}
            <div className="mt-3 flex flex-wrap gap-2">
              <button className={button} disabled={busy || !p.logged_in || !mac} title={t("Open Claude Code in Terminal with this account")} onClick={() => void openCode(p)}>{t("Open Claude Code")}</button>
              <button className={button} disabled={busy || !mac} onClick={() => { setError(null); setDesktopTarget(p); }}>{p.desktop_status === "saved" ? t("Switch Desktop") : t("Desktop sign-in")}</button>
<button className={button} disabled={busy || !p.logged_in} onClick={() => void action("warmup_claude_account", { profileId: p.id })}>{t("Warm up")}</button>
              {!p.is_default && <button className={button} disabled={busy} onClick={() => { setError(null); setEmail(p.email); setLogin(null); setAdding(true); }}>{t("Code login link")}</button>}
            </div>
          </article>))}
      </div>
      <dialog ref={dialog} aria-labelledby="claude-dialog-title" onCancel={(e) => { e.preventDefault(); void closeDialog(); }} className="m-auto w-full max-w-lg rounded-2xl border border-gray-200 bg-white p-6 text-gray-900 shadow-xl backdrop:bg-black/50 dark:border-gray-700 dark:bg-gray-900 dark:text-gray-100">
        <h3 id="claude-dialog-title" className="text-lg font-semibold">{desktopTarget ? t("Switch Claude Desktop account") : t("Add Claude account")}</h3>
        {desktopTarget ? <>
          <p className="mt-3 text-sm">{t("The Claude Desktop profile for {email} will open. Code login links do not sign into Desktop; on first use, sign in with this email in the Claude window. Once saved, the profile can be opened again.", { email: desktopTarget.email })}</p>
          <p className="mt-2 text-xs text-gray-600 dark:text-gray-300">{t("Finish any active Desktop work before Claude closes and reopens.")}</p>
          <button className={`${button} mt-4`} disabled={busy} onClick={async () => { if (await action("switch_claude_desktop", { profileId: desktopTarget.id }))
            setDesktopTarget(null); }}>{t("Close Claude and switch")}</button>
        </> : login?.status === "complete" ? <>
          <p role="status" className="mt-3 text-sm">{t("Claude Code sign-in saved for {email}.", { email })}</p>
          <p className="mt-2 text-sm text-gray-600 dark:text-gray-300">{t("Open a terminal now or complete the separate Desktop sign-in.")}</p>
          <div className="mt-4 flex flex-wrap gap-2">
            <button className={button} disabled={busy || !mac || !loginProfile?.logged_in} onClick={() => { if (loginProfile)
            void openCode(loginProfile); }}>{t("Open Claude Code")}</button>
            <button className={button} disabled={busy || !mac || !loginProfile} onClick={() => { if (loginProfile) {
            setAdding(false);
            setDesktopTarget(loginProfile);
        } }}>{t("Complete Desktop sign-in")}</button>
          </div>
        </> : <>
          <form onSubmit={(e) => { e.preventDefault(); void startLogin(); }}>
            <label className="mt-4 block text-sm" htmlFor="claude-email">{t("Account email")}</label>
            <input autoFocus id="claude-email" type="email" required value={email} disabled={busy || login?.status === "pending"} onChange={(e) => setEmail(e.target.value)} className="mt-1 w-full rounded-lg border border-gray-300 bg-transparent px-3 py-2 dark:border-gray-600"/>
            <button type="submit" className={`${button} mt-3`} disabled={busy || login?.status === "pending"}>{busy ? t("Preparing\u2026") : t("Generate login link")}</button>
          </form>
          {login?.status === "pending" && <p role="status" className="mt-3 text-sm">{t("Waiting for Code sign-in to be saved on this Mac. Return here after the browser step; the account is confirmed only after the native login completes.")}</p>}
          {login?.auth_url && <div className="mt-3 space-y-2">
            <button className={button} onClick={() => void openExternalUrl(login.auth_url!).catch((e) => setError(String(e)))}>{t("Open login link")}</button>{" "}
            <button className={button} onClick={() => void navigator.clipboard.writeText(login.auth_url!).then(() => setCopied(true)).catch((e) => setError(String(e)))}>{copied ? t("Copied") : t("Copy link")}</button>
            <p className="text-xs text-gray-600 dark:text-gray-300">{t("Open this original Claude link in any browser. Complete the Google steps in that browser.")}</p>
            <p className="break-all text-xs text-gray-600 dark:text-gray-300">{login.auth_url}</p>
          </div>}
          {login?.status === "pending" && <form className="mt-4 space-y-2" onSubmit={e => { e.preventDefault(); void submitReturn(); }}>
            <label htmlFor="claude-return" className="block text-sm">{t("Return URL or authorization code")}</label>
            <input id="claude-return" type="password" autoComplete="off" value={returnInput} onChange={e => setReturnInput(e.target.value)} className="w-full rounded-lg border border-gray-300 bg-transparent px-3 py-2 dark:border-gray-600"/>
            <p className="text-xs text-gray-600 dark:text-gray-300">{t("Paste the final localhost callback URL or the complete code#state shown by Claude.")}</p>
            <button className={button} disabled={busy || !returnInput.trim()}>{t("Complete sign-in")}</button>
          </form>}
          <p className="mt-3 text-xs text-gray-600 dark:text-gray-300">{t("This link signs into Claude Code. Complete Desktop sign-in separately in the Claude app.")}</p>
        </>}
        {error && <p role="alert" className="mt-3 text-sm text-red-700 dark:text-red-300">{localizeMessage(error)}</p>}
        <button className={`${button} mt-4`} disabled={busy} onClick={() => void closeDialog()}>{login?.status === "pending" ? t("Cancel sign-in") : t("Close")}</button>
      </dialog>
    </section>);
}
