import { Fragment, forwardRef, useCallback, useEffect, useImperativeHandle, useRef, useState } from "react";
import { useI18n, localizeMessage } from "../lib/i18n";
import { invokeBackend, isTauriRuntime, openExternalUrl } from "../lib/platform";
import { RateLimitBar } from "./UsageBar";

interface CursorProfile {
    id: string;
    email: string;
    plan_name: string | null;
    is_merged: boolean;
    usage: { period: string; used_percent: number | null; reset_at: string | null; checked_at: number } | null;
    error: string | null;
}
interface CursorProfiles { profiles: CursorProfile[]; active_desktop_id: string | null }
interface CursorLogin { flow_id: string; auth_url: string | null; status: string; error: string | null }
export type CursorToolbarCommand = "refresh" | "import" | "add";
export interface CursorToolbarState { ready: boolean; busy: boolean }
export interface CursorAccountsHandle { run: (command: CursorToolbarCommand) => Promise<void> }
const button = "focus-visible:outline-2 focus-visible:outline-offset-2 rounded-lg border border-gray-300 px-3 py-2 text-sm font-medium text-gray-800 hover:bg-gray-100 disabled:opacity-50 dark:border-gray-600 dark:text-gray-100 dark:hover:bg-gray-800";

export const CursorAccounts = forwardRef<CursorAccountsHandle, {
    onAccountCountChange: (count: number) => void;
    onToolbarChange: (state: CursorToolbarState) => void;
    masked: boolean;
}>(function CursorAccounts({ onAccountCountChange, onToolbarChange, masked }, ref) {
    const { t, locale } = useI18n();
    const [store, setStore] = useState<CursorProfiles | null>(null);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [adding, setAdding] = useState(false);
    const [email, setEmail] = useState("");
    const [login, setLogin] = useState<CursorLogin | null>(null);
    const [copied, setCopied] = useState(false);
    const dialog = useRef<HTMLDialogElement>(null);
    const desktop = isTauriRuntime();
    const load = useCallback(async () => {
        try { setStore(await invokeBackend<CursorProfiles>("list_cursor_profiles")); }
        catch (e) { setError(String(e)); }
    }, []);
    useEffect(() => {
        if (!desktop) return;
        void load();
        const reload = () => { void load(); };
        window.addEventListener("focus", reload);
        const timer = window.setInterval(() => {
            if (!dialog.current?.closest("[hidden]") && document.visibilityState === "visible") {
                void invokeBackend("refresh_cursor_profiles").then(load).catch(error => setError(String(error)));
            }
        }, 300000);
        return () => { window.removeEventListener("focus", reload); window.clearInterval(timer); };
    }, [desktop, load]);
    useEffect(() => { if (store) onAccountCountChange(store.profiles.length); }, [store, onAccountCountChange]);
    useEffect(() => { onToolbarChange({ ready: desktop && !!store, busy }); }, [desktop, store, busy, onToolbarChange]);
    useEffect(() => { if (adding) dialog.current?.showModal(); else dialog.current?.close(); }, [adding]);
    useEffect(() => {
        if (login?.status !== "pending") return;
        let disposed = false;
        let timer: ReturnType<typeof setTimeout>;
        const poll = async () => {
            try {
                const next = await invokeBackend<CursorLogin | null>("get_cursor_login");
                if (disposed) return;
                setLogin(next);
                if (next?.error) setError(next.error);
                if (next?.status === "complete") await load();
                else if (next?.status === "pending") timer = setTimeout(poll, 2000);
            } catch (e) {
                if (!disposed) { setError(String(e)); timer = setTimeout(poll, 5000); }
            }
        };
        timer = setTimeout(poll, 1000);
        return () => { disposed = true; clearTimeout(timer); };
    }, [login?.status, load]);
    const action = async (command: string, args?: Record<string, unknown>) => {
        setBusy(true); setError(null);
        try { await invokeBackend(command, args); await load(); }
        catch (e) { setError(String(e)); }
        finally { setBusy(false); }
    };
    useImperativeHandle(ref, () => ({ async run(command) {
        if (busy || !store) return;
        if (command === "add") { setError(null); setEmail(""); setLogin(null); setCopied(false); setAdding(true); }
        else await action(command === "import" ? "import_current_cursor_profile" : "refresh_cursor_profiles");
    } }));
    const startLogin = async () => {
        setBusy(true); setError(null); setCopied(false);
        try { setLogin(await invokeBackend<CursorLogin>("start_cursor_login", { email: email.trim() })); }
        catch (e) { setError(String(e)); }
        finally { setBusy(false); }
    };
    const closeDialog = async () => {
        if (busy) return;
        if (login?.status === "pending") {
            setBusy(true);
            try { await invokeBackend("cancel_cursor_login"); }
            catch (e) { setError(String(e)); return; }
            finally { setBusy(false); }
        }
        setAdding(false); setLogin(null);
    };
    const active = store?.profiles.find(p => p.id === store.active_desktop_id);
    const others = store?.profiles.filter(p => p.id !== active?.id) ?? [];
    const ordered = active ? [active, ...others] : others;
    if (!desktop) return null;
    return <section aria-labelledby="cursor-accounts-title" className="provider-section provider-section--cursor space-y-4">
        <h2 id="cursor-accounts-title" className="sr-only">{t("Grok / Cursor accounts")}</h2>
        <details className="border-t border-gray-200 pt-3 dark:border-gray-700">
            <summary className="cursor-pointer text-sm font-medium text-gray-700 dark:text-gray-200">{t("Grok / Cursor account details")}</summary>
            <p className="mt-2 text-xs text-gray-600 dark:text-gray-300">{t("Unified Grok and Cursor subscriptions share one usage pool. Usage shown here comes from Cursor.")}</p>
            <p className="mt-2 text-xs text-gray-600 dark:text-gray-300">{t("Desktop switching is manual. Save your work and close Cursor before switching. Automatic switching, chat continuation and warm-up are not available.")}</p>
        </details>
        {error && <p role="alert" className="rounded-lg bg-red-50 p-3 text-sm text-red-800 dark:bg-red-950 dark:text-red-200">{localizeMessage(error)}</p>}
        {store?.profiles.length === 0 && <p className="text-sm text-gray-600 dark:text-gray-300">{t("Import the account signed into Cursor Desktop, or add another account with a login link.")}</p>}
        <div className="account-list grid gap-3">
            {ordered.map((p, index) => {
                const reset = p.usage?.reset_at ? Date.parse(p.usage.reset_at) / 1000 : NaN;
                const used = p.usage?.used_percent;
                const hasUsage = typeof used === "number" && Number.isFinite(used) && used >= 0 && used <= 100;
                return <Fragment key={p.id}>
                    {active && index === 0 && <h2 className="text-base font-semibold text-gray-700 dark:text-gray-200">{t("Active Account")}</h2>}
                    {index === (active ? 1 : 0) && <h2 className="mt-4 text-base font-semibold text-gray-700 dark:text-gray-200">{t("Other Accounts ({count})", { count: others.length })}</h2>}
                    <article data-active={p.id === active?.id} className={`account-card account-card--cursor rounded-xl border bg-white p-4 dark:bg-gray-900 ${p.id === active?.id ? "border-emerald-500" : "border-gray-200 dark:border-gray-700"}`}>
                        <div className="account-card-header flex flex-wrap items-center justify-between gap-2">
                            <h3 className="break-all font-semibold">{p.id === active?.id && <span className="mr-2 inline-block h-2 w-2 rounded-full bg-emerald-500" aria-hidden="true" />}{masked ? "••••••@••••••" : p.email}</h3>
                            <span className="rounded-full border border-gray-300 px-2 py-1 text-xs font-medium dark:border-gray-600">{p.plan_name || t("Unknown plan")}</span>
                        </div>
                        <p className="mt-1 text-xs text-gray-600 dark:text-gray-300">{p.is_merged ? t("Shared Grok / Cursor subscription") : t("Cursor subscription")}</p>
                        <div className="mt-3 space-y-2">{hasUsage ? <RateLimitBar label={p.is_merged ? t("Shared usage limit") : t("Cursor usage limit")} windowMinutes={p.usage?.period.includes("WEEKLY") ? 10080 : undefined} usedPercent={used!} resetsAt={Number.isFinite(reset) ? reset : null} /> : <p className="text-xs text-gray-500 dark:text-gray-400">{t("Cursor has not provided a remaining usage percentage. Check the usage dashboard.")}</p>}</div>
                        {p.usage && <p className="mt-1 text-xs text-gray-600 dark:text-gray-300">{t("Last checked: {time}", { time: new Date(p.usage.checked_at * 1000).toLocaleTimeString(locale) })}</p>}
                        {p.error && <p className="mt-2 text-xs text-red-700 dark:text-red-300">{localizeMessage(p.error)}</p>}
                        <div className="account-card-actions mt-3 flex flex-wrap gap-2">
                            <button className={`${button} account-primary-action`} disabled={busy || p.id === active?.id} onClick={() => void action("switch_cursor_desktop", { profileId: p.id })}>{p.id === active?.id ? t("Active") : t("Switch Cursor Desktop")}</button>
                            {p.id === active?.id && <button className={button} disabled={busy} onClick={() => void action("open_cursor_desktop")}>{t("Open Cursor")}</button>}
                            <button className={button} onClick={() => void openExternalUrl("https://cursor.com/dashboard?tab=usage").catch(e => setError(String(e)))}>{t("Cursor usage")}</button>
                            {p.is_merged && <button className={button} onClick={() => void openExternalUrl("https://grok.com/dashboard").catch(e => setError(String(e)))}>{t("Open Grok")}</button>}
                        </div>
                    </article>
                </Fragment>;
            })}
        </div>
        <dialog ref={dialog} aria-labelledby="cursor-dialog-title" onCancel={e => { e.preventDefault(); void closeDialog(); }} className="m-auto w-full max-w-lg rounded-2xl border border-gray-200 bg-white p-6 text-gray-900 shadow-xl backdrop:bg-black/50 dark:border-gray-700 dark:bg-gray-900 dark:text-gray-100">
            <h3 id="cursor-dialog-title" className="text-lg font-semibold">{t("Add Grok / Cursor account")}</h3>
            {login?.status === "complete" ? <p role="status" className="mt-3 text-sm">{t("Cursor sign-in saved for {email}.", { email: masked ? "••••••@••••••" : email })}</p> : <>
                <form onSubmit={e => { e.preventDefault(); void startLogin(); }}>
                    <label htmlFor="cursor-email" className="mt-4 block text-sm">{t("Account email")}</label>
                    <input autoFocus id="cursor-email" type="email" required autoComplete="email" value={email} disabled={busy || login?.status === "pending"} onChange={e => setEmail(e.target.value)} className="mt-1 w-full rounded-lg border border-gray-300 bg-transparent px-3 py-2 dark:border-gray-600" />
                    <button className={`${button} mt-3`} disabled={busy || login?.status === "pending"}>{busy ? t("Preparing…") : t("Generate login link")}</button>
                </form>
                {login?.status === "pending" && <p role="status" className="mt-3 text-sm">{t("Waiting for Cursor sign-in. The account is added only after its identity is verified.")}</p>}
                {login?.auth_url && <div className="mt-3 space-y-2">
                    <div className="flex flex-wrap gap-2">
                        <button className={button} onClick={() => void openExternalUrl(login.auth_url!).catch(e => setError(String(e)))}>{t("Open login link")}</button>
                        <button className={button} onClick={() => void navigator.clipboard.writeText(login.auth_url!).then(() => setCopied(true)).catch(e => setError(String(e)))}>{copied ? t("Copied") : t("Copy link")}</button>
                    </div>
                    <p className="text-xs text-gray-600 dark:text-gray-300">{t("Copy the original Cursor link into any browser. Return here after signing in; completion is checked automatically.")}</p>
                </div>}
            </>}
            {error && <p role="alert" className="mt-3 text-sm text-red-700 dark:text-red-300">{localizeMessage(error)}</p>}
            <button className={`${button} mt-4`} disabled={busy} onClick={() => void closeDialog()}>{login?.status === "pending" ? t("Cancel sign-in") : t("Close")}</button>
        </dialog>
    </section>;
});
