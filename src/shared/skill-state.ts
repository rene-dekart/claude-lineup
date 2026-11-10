import type { SkillState } from './types.js'

/**
 * Whether a skill state leaves the skill in reach. Four states, two of them "on"
 * in the sense a row asks about: `name-only` keeps the skill listed for the
 * model, `user-invocable-only` leaves it to a typed `/name`. Only `off` and
 * `user-invocable-only` take it out of the model's hands.
 *
 * One function for both readers of a state — the file's `skillOverrides` when
 * the inventory is scanned, and the profile's own map when a row is drawn.
 * Written twice, the two would drift and the box would disagree with the file
 * again, which is the defect this pair exists to prevent.
 */
export function skillStateOn(state: SkillState): boolean {
  return state !== 'off' && state !== 'user-invocable-only'
}
