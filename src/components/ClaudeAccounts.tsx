import { useCallback, useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { RateLimitBar } from "./UsageBar";
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
  desktop_status: "saved" | "login_required" | "different_account" | "unavailable";
  usage: ClaudeUsage | null;
  error: string | null;
}
interface ClaudeProfiles {
  profiles: ClaudeProfile[];
  active_code_id: string | null;
  active_desktop_id: string | null;
  auto_switch_enabled: boolean;
}
interface ClaudeLogin {
  profile_id: string;
  auth_url: string | null;
  status: string;
  error: string | null;
}

const button = "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#d97757] rounded-lg border border-gray-300 px-3 py-2 text-sm font-medium text-gray-800 hover:bg-gray-100 disabled:opacity-50 dark:border-gray-600 dark:text-gray-100 dark:hover:bg-gray-800";

export function ClaudeAccounts() {
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
    try { setStore(await invokeBackend<ClaudeProfiles>("list_claude_profiles")); }
    catch (e) { setError(String(e)); }
  }, []);

  useEffect(() => {
    if (!desktop) return;
    void load();
    const reload = () => { void load(); };
    window.addEventListener("focus", reload);
    return () => window.removeEventListener("focus", reload);
  }, [desktop, load]);
  useEffect(() => {
    if (adding || desktopTarget) dialog.current?.showModal();
    else dialog.current?.close();
  }, [adding, desktopTarget]);

  useEffect(() => {
    if (login?.status !== "pending") return;
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const next = await invokeBackend<ClaudeLogin | null>("get_claude_login");
        if (disposed) return;
        setLogin(next);
        if (next?.status === "complete") {
          await invokeBackend("refresh_claude_profiles");
          await load();
          return;
        }
        if (next?.error) setError(next.error);
        if (next?.status === "pending") timer = setTimeout(poll, 1000);
      } catch (e) { if (!disposed) setError(String(e)); }
    };
    timer = setTimeout(poll, 500);
    return () => { disposed = true; clearTimeout(timer); };
  }, [login?.status, load]);

  const action = async (command: string, args?: Record<string, unknown>) => {
    setBusy(true); setError(null);
    try {
      await invokeBackend<ClaudeProfiles>(command, args);
      await load();
      return true;
    } catch (e) { setError(String(e)); return false; }
    finally { setBusy(false); }
  };

  const startLogin = async () => {
    setBusy(true); setError(null); setCopied(false);
    try { setLogin(await invokeBackend<ClaudeLogin>("start_claude_login", { email })); }
    catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  };

  const closeDialog = async () => {
    if (login?.status === "pending") await invokeBackend("cancel_claude_login").catch((e) => setError(String(e)));
    setLogin(null); setAdding(false); setDesktopTarget(null); setCopied(false);
    void load();
  };

  const chooseProject = async () => {
    setError(null);
    try {
      const selected = await open({ directory: true, multiple: false, title: "Claude Code projesini seç" });
      if (!selected || Array.isArray(selected)) return;
      localStorage.setItem("claude-project-directory", selected);
      setCwd(selected);
    } catch (e) { setError(String(e)); }
  };

  const openCode = async (p: ClaudeProfile) => {
    if (await action("open_claude_code", { profileId: p.id, cwd: cwd || null })) {
      if (adding) void closeDialog();
    }
  };

  const resetTime = (value: string | null) => {
    const seconds = value ? Date.parse(value) / 1000 : NaN;
    return Number.isFinite(seconds) ? seconds : null;
  };
  const loginProfile = store?.profiles.find(p => p.id === login?.profile_id);


  if (!desktop) return null;
  return (
    <section aria-labelledby="claude-accounts-title" className="mb-8 rounded-xl border border-[#d97757]/60 bg-white p-5 dark:border-[#d97757]/70 dark:bg-gray-900">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 id="claude-accounts-title" className="text-lg font-semibold text-[#a54f36] dark:text-[#e6a28a]">Claude hesapları</h2>
          <p className="text-sm text-gray-600 dark:text-gray-300">Claude Code ve Claude Desktop girişlerini yönet.</p>
        </div>
        <div className="flex flex-wrap gap-2">
          <button className={button} disabled={busy} onClick={() => void action("import_current_claude_profile")}>Mevcut hesabı ekle</button>
          <button className={button} disabled={busy} onClick={() => void action("refresh_claude_profiles")}>Kullanımı yenile</button>
          <button className={button} disabled={busy || !store} onClick={() => { setError(null); setEmail(""); setLogin(null); setAdding(true); }}>Hesap ekle</button>
        </div>
      </div>
      <label className="mt-4 flex items-center gap-2 text-sm font-medium text-gray-900 dark:text-gray-100">
        <input type="checkbox" checked={store?.auto_switch_enabled ?? false} disabled={busy || !store}
          onChange={(e) => void action("set_claude_auto_switch", { enabled: e.target.checked })} />
        Claude Code auto-switch
      </label>
      <div className="mt-3 flex flex-wrap items-center gap-2 text-xs text-gray-600 dark:text-gray-300">
        <span className="break-all">Proje: {cwd || "Ana klasör"}</span>
        <button className={button} disabled={busy} onClick={() => void chooseProject()}>Proje klasörü seç</button>
        {cwd && <button className={button} disabled={busy} onClick={() => { localStorage.removeItem("claude-project-directory"); setCwd(""); }}>Ana klasör</button>}
      </div>
      <p className="mt-2 text-xs text-gray-600 dark:text-gray-300">Buradan açılan terminalde limit dolunca kullanımı doğrulanmış hesaba geçer ve aynı yerel sohbeti sürdürür. Normal Desktop sohbetleri hesaplar arasında taşınmaz.</p>
      {error && <p role="alert" className="mt-3 rounded-lg bg-red-50 p-3 text-sm text-red-800 dark:bg-red-950 dark:text-red-200">{error}</p>}
      {store?.profiles.length === 0 && <p className="mt-4 text-sm text-gray-600 dark:text-gray-300">Önce açık Claude Code hesabını ekle, ardından ikinci hesabın için giriş bağlantısı oluştur.</p>}
      <div className="mt-4 grid gap-4 sm:grid-cols-2">
        {store?.profiles.map((p) => (
          <article key={p.id} className={`rounded-xl border bg-white p-5 dark:bg-gray-900 ${store.active_code_id === p.id ? "border-[#d97757]" : "border-gray-200 dark:border-gray-700"}`}>
            <h3 className="break-all font-semibold text-gray-900 dark:text-gray-100">{p.email}</h3>
            <p className="mt-1 text-xs text-gray-600 dark:text-gray-300">
              {p.logged_in ? "Code girişi doğrulandı" : "Code girişi gerekli"}
              {store.active_code_id === p.id && " · Seçili Code hesabı"}
              {store.active_desktop_id === p.id && " · Seçili Desktop profili"}
            </p>
            <p className="mt-1 text-xs text-gray-600 dark:text-gray-300">
              {p.desktop_status === "saved" ? "Desktop: kayıtlı hesap profili" : p.desktop_status === "different_account" ? "Desktop: farklı hesapla giriş yapılmış" : "Desktop: ayrı giriş gerekli"}
            </p>
            <div className="mt-3 space-y-2">
              {p.usage ? <>
                <RateLimitBar label="5 saatlik kullanım" windowMinutes={300} usedPercent={p.usage.five_hour_used} resetsAt={resetTime(p.usage.five_hour_resets_at)} tone="claude" />
                <RateLimitBar label="Haftalık kullanım" windowMinutes={10080} usedPercent={p.usage.seven_day_used} resetsAt={resetTime(p.usage.seven_day_resets_at)} tone="claude" />
              </> : <p className="text-xs text-gray-500 dark:text-gray-400">Kalan kullanım henüz doğrulanmadı</p>}
            </div>
            {p.usage && <p className="mt-1 text-xs text-gray-600 dark:text-gray-300">Son kontrol: {new Date(p.usage.checked_at * 1000).toLocaleTimeString()}</p>}
            {p.error && <p className="mt-2 text-xs text-red-700 dark:text-red-300">{p.error}</p>}
            <div className="mt-3 flex flex-wrap gap-2">
              <button className={button} disabled={busy || !p.logged_in || !mac} title="Seçilen hesapla Terminal’de Claude Code’u aç" onClick={() => void openCode(p)}>Claude Code aç</button>
              <button className={button} disabled={busy || !mac} onClick={() => { setError(null); setDesktopTarget(p); }}>{p.desktop_status === "saved" ? "Desktop’a geç" : "Desktop girişi"}</button>
              {!p.is_default && <button className={button} disabled={busy} onClick={() => { setError(null); setEmail(p.email); setLogin(null); setAdding(true); }}>Code giriş bağlantısı</button>}
            </div>
          </article>
        ))}
      </div>
      <dialog ref={dialog} aria-labelledby="claude-dialog-title" onCancel={(e) => { e.preventDefault(); void closeDialog(); }}
        className="m-auto w-full max-w-lg rounded-2xl border border-gray-200 bg-white p-6 text-gray-900 shadow-xl backdrop:bg-black/50 dark:border-gray-700 dark:bg-gray-900 dark:text-gray-100">
        <h3 id="claude-dialog-title" className="text-lg font-semibold">{desktopTarget ? "Claude Desktop hesabını değiştir" : "Claude hesabı ekle"}</h3>
        {desktopTarget ? <>
          <p className="mt-3 text-sm">{desktopTarget.email} için Claude Desktop profili açılacak. Code giriş bağlantısı Desktop oturumunu açmaz; ilk kullanımda Claude penceresinde bu e-postayla giriş yap. Giriş bir kez kaydedildikten sonra bu profil tekrar açılır.</p>
          <p className="mt-2 text-xs text-gray-600 dark:text-gray-300">Claude Desktop kapanıp yeniden açılacağı için açık masaüstü işlerini önce tamamla.</p>
          <button className={`${button} mt-4`} disabled={busy} onClick={async () => { if (await action("switch_claude_desktop", { profileId: desktopTarget.id })) setDesktopTarget(null); }}>Claude’u kapat ve geç</button>
        </> : login?.status === "complete" ? <>
          <p role="status" className="mt-3 text-sm">{email} için Claude Code girişi kaydedildi.</p>
          <p className="mt-2 text-sm text-gray-600 dark:text-gray-300">Şimdi terminalde açabilir veya ayrı Desktop girişini tamamlayabilirsin.</p>
          <div className="mt-4 flex flex-wrap gap-2">
            <button className={button} disabled={busy || !mac || !loginProfile?.logged_in} onClick={() => { if (loginProfile) void openCode(loginProfile); }}>Claude Code aç</button>
            <button className={button} disabled={busy || !mac || !loginProfile} onClick={() => { if (loginProfile) { setAdding(false); setDesktopTarget(loginProfile); } }}>Desktop girişini tamamla</button>
          </div>
        </> : <>
          <form onSubmit={(e) => { e.preventDefault(); void startLogin(); }}>
            <label className="mt-4 block text-sm" htmlFor="claude-email">Hesap e-postası</label>
            <input autoFocus id="claude-email" type="email" required value={email} disabled={busy || login?.status === "pending"}
              onChange={(e) => setEmail(e.target.value)} className="mt-1 w-full rounded-lg border border-gray-300 bg-transparent px-3 py-2 dark:border-gray-600" />
            <button type="submit" className={`${button} mt-3`} disabled={busy || login?.status === "pending"}>{busy ? "Hazırlanıyor…" : "Giriş bağlantısı oluştur"}</button>
          </form>
          {login?.status === "pending" && <p role="status" className="mt-3 text-sm">Claude’un resmi girişinin tamamlanması bekleniyor…</p>}
          {login?.auth_url && <div className="mt-3 space-y-2">
            <button className={button} onClick={() => void openExternalUrl(login.auth_url!).catch((e) => setError(String(e)))}>Giriş bağlantısını aç</button>{" "}
            <button className={button} onClick={() => void navigator.clipboard.writeText(login.auth_url!).then(() => setCopied(true)).catch((e) => setError(String(e)))}>{copied ? "Kopyalandı" : "Bağlantıyı kopyala"}</button>
            <p className="break-all text-xs text-gray-600 dark:text-gray-300">{login.auth_url}</p>
          </div>}
          <p className="mt-3 text-xs text-gray-600 dark:text-gray-300">Bu bağlantı Claude Code içindir. Desktop girişi karttaki “Desktop girişi” düğmesiyle Claude uygulamasında tamamlanır.</p>
        </>}
        {error && <p role="alert" className="mt-3 text-sm text-red-700 dark:text-red-300">{error}</p>}
        <button className={`${button} mt-4`} disabled={busy} onClick={() => void closeDialog()}>{login?.status === "pending" ? "Girişi iptal et" : "Kapat"}</button>
      </dialog>
    </section>
  );
}
