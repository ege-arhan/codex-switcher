import assert from "node:assert/strict";
import test from "node:test";
import { canWarmupClaude } from "../src/lib/claudeWarmup.ts";
test("bulk Claude warm-up excludes unverified, exhausted and nonpaid accounts", () => {
  const profile = { logged_in: true, plan_type: "pro", error: null, usage: { five_hour_used: 95, seven_day_used: 60, checked_at: 1000, model_used: null } };
  assert.ok(canWarmupClaude(profile, 1030));
  for (const plan_type of ["free", null, "unknown"]) assert.equal(canWarmupClaude({ ...profile, plan_type }, 1030), false);
  assert.equal(canWarmupClaude({ ...profile, error: "HTTP 429" }, 1030), false);
  assert.equal(canWarmupClaude({ ...profile, logged_in: false }, 1030), false);
  assert.equal(canWarmupClaude({ ...profile, usage: null }, 1030), false);
  assert.equal(canWarmupClaude(profile, 1061), false);
  for (const value of [100, NaN, Infinity, -1]) {
    assert.equal(canWarmupClaude({ ...profile, usage: { ...profile.usage, five_hour_used: value } }, 1030), false);
    assert.equal(canWarmupClaude({ ...profile, usage: { ...profile.usage, model_used: value } }, 1030), false);
  }
});
