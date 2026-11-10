import type { I18nModule } from '../index.js'

export const sess: I18nModule = {
  ru: {
    'sess.projectSkillsNone': 'Режим навыков «нет» в файле настроек невыразим: --disable-slash-commands — это флаг командной строки, работают только отдельные состояния навыков.',
    'sess.skillStatesProject': 'Состояния навыков записаны в settings.json ключом skillOverrides. Обратите внимание: файл умеет только гасить навык — вернуть выключенный здесь навык обратно этот профиль не сможет, ключ просто исчезнет.',
    'sess.projectMemoryNeedsBare': 'Отключение авто-памяти в файле настроек невыразимо: это делается флагом запуска, не записью.',
    'sess.projectHooksAllOrNothing': 'Отдельный обработчик отключить нельзя. Доступно только disableAllHooks — гасит все обработчики, включая нужные.',
    'sess.mcpServersDroppedFromProject': 'Ключ mcpServers не записан в settings.json: Claude Code его игнорирует.',
    'sess.fileChangedSinceDiff': 'Файл изменился после расчёта diff: {path}. Пересчитайте перед записью.',
    // The rollback path. It refuses rather than restoring over someone's edit.
    'sess.bakNotFound': 'Точка восстановления {id} не найдена.',
    'sess.bakConflict': 'Файл изменился после записи: {path}. Откат не выполнен — сначала разберитесь с этой правкой.',
    'sess.bakOutside': 'Цель вне проекта, к которому относится запись: {path}.',
    'sess.claudeJsonBusy': 'Запущена сессия Claude Code — она держит ~/.claude.json в памяти и перезапишет правку при выходе. Закройте сессии и примените заново.',
  },
  en: {
    'sess.projectSkillsNone': 'The "none" skill mode cannot be expressed in a settings file: --disable-slash-commands is a command-line flag, so only the per-skill states apply.',
    'sess.skillStatesProject': 'Skill states were written into settings.json under skillOverrides. Note that the file can only turn a skill off — this profile cannot bring back a skill disabled here, the key simply disappears.',
    'sess.projectMemoryNeedsBare': 'Turning auto-memory off cannot be expressed in a settings file: a launch flag does it, not a write.',
    'sess.projectHooksAllOrNothing': 'An individual hook cannot be disabled. Only disableAllHooks is available — it kills every hook, including the ones you need.',
    'sess.mcpServersDroppedFromProject': 'The mcpServers key was not written to settings.json: Claude Code ignores it.',
    'sess.fileChangedSinceDiff': 'The file changed after the diff was computed: {path}. Recompute before writing.',
    // The rollback path. It refuses rather than restoring over someone's edit.
    'sess.bakNotFound': 'No restore point {id}.',
    'sess.bakConflict': 'The file changed after the write: {path}. The rollback was not performed — deal with that edit first.',
    'sess.bakOutside': 'The target is outside the project this write belongs to: {path}.',
    'sess.claudeJsonBusy': 'A Claude Code session is running — it holds ~/.claude.json in memory and will overwrite the edit when it exits. Close the sessions and apply again.',
  },
}
