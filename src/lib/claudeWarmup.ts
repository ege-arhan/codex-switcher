// The backend rechecks native credentials and quota before sending the warm-up.
export function canWarmupClaude(profile: { logged_in: boolean; plan_type: string | null; error: string | null; usage: { five_hour_used: number; seven_day_used: number; checked_at: number; model_used?: number | null } | null }, now = Date.now() / 1000): boolean {
  const usage = profile.usage;
  return profile.logged_in && !profile.error && ["pro", "max", "team", "enterprise"].includes(profile.plan_type ?? "") && !!usage
    && now >= usage.checked_at && now - usage.checked_at <= 60
    && [usage.five_hour_used, usage.seven_day_used, usage.model_used ?? 0].every(value => Number.isFinite(value) && value >= 0 && value < 100);
}
