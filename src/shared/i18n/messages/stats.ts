import type { I18nModule } from '../index.js'

/**
 * The statistics window. The figures themselves are formatted in the overlay —
 * these are the words around them, plus one entry per saving tool, whose id is
 * the key suffix the server reports (`stats.tool.<id>`).
 */
export const stats: I18nModule = {
  ru: {
    'stats.title': 'Статистика',
    'stats.loading': 'Считаю транскрипты…',
    'stats.failed': 'Не удалось собрать статистику: {message}',
    'stats.empty': 'Ни одной сессии не найдено — статистики пока нет.',

    'stats.sessions': 'Сессии',
    'stats.messages': 'Сообщения',
    'stats.projects': 'Проекты',
    // Four separate labels on purpose: a cache read costs a fraction of an input
    // token, so a single "tokens" figure would overstate the bill.
    'stats.input': 'Вход',
    'stats.output': 'Выход',
    'stats.cacheRead': 'Кэш-чтение',
    'stats.cacheWrite': 'Кэш-запись',

    'stats.since': 'Первая запись: {date}',
    'stats.longest': 'Самая долгая сессия: {min} мин',

    'stats.noProject': 'без проекта',

    'stats.byDay': 'По дням',
    'stats.window': 'Последние {n} дней',
    'stats.barTip': '{date}: сессий {sessions}, сообщений {messages}, вход {input}, выход {output}',

    'stats.savings': 'Экономия',
    'stats.noData': 'нет данных',
    'stats.savedOf': 'из {total}',
    'stats.calls': 'вызовов: {calls}',

    'stats.byProject': 'По проектам',
    'stats.path': 'Путь',
    'stats.inOut': 'вход / выход',
    'stats.notShown': 'Не показано проектов: {n}',

    'stats.byModel': 'По моделям',
    'stats.model': 'Модель',

    // The pile the sessions were read out of. A separate block because the numbers
    // on it are about the disk, not about the sessions: what is on it, and what the
    // retention rule has not taken yet.
    'stats.history': 'История',
    'stats.onDisk': 'На диске',
    'stats.olderThan': 'Старше {days} дней',
    'stats.filesSize': 'файлов: {files} · {size}',
    'stats.cleanup': 'Очистить',
    // Armed: the second click is the one that deletes. Same idiom as the restore
    // points — a destructive control says what it will do before it does it.
    'stats.cleanupArmed': 'Удалить {files}?',
    'stats.cleanupAsk': 'Удалить файлов: {files} ({size})? Их сессии исчезнут из статистики.',
    'stats.cleaned': 'Удалено файлов: {files} ({size})',
    'stats.cleanupFailed': 'Не удалось удалить файлов: {failed}',
    'stats.cleanupNoDays': 'Не сказано, за сколько дней удалять',

    'stats.notes': 'Замечания',
    // Why a saving tool has nothing to report. A tool that is simply absent and a
    // tool that ran but saved nothing must not read the same.
    'stats.note.rtkIdle': 'rtk пока не выполнил ни одной команды',
    'stats.note.noDir': 'Папка не найдена: {path}',
    'stats.note.noDbs': 'В папке нет баз данных: {path}',
    'stats.note.noEvents': 'Ни одной записи в базах: {n}',

    // Brand names: the same string in both languages, which is why this module
    // carries them at all — the server sends the id, the panel needs a label.
    'stats.tool.rtk': 'rtk',
    'stats.tool.context-mode': 'context-mode',
    'stats.tool.caveman': 'caveman',
  },
  en: {
    'stats.title': 'Statistics',
    'stats.loading': 'Reading transcripts…',
    'stats.failed': 'Could not collect the statistics: {message}',
    'stats.empty': 'No sessions found — there is no statistics yet.',

    'stats.sessions': 'Sessions',
    'stats.messages': 'Messages',
    'stats.projects': 'Projects',
    'stats.input': 'Input',
    'stats.output': 'Output',
    'stats.cacheRead': 'Cache read',
    'stats.cacheWrite': 'Cache write',

    'stats.since': 'First entry: {date}',
    'stats.longest': 'Longest session: {min} min',

    'stats.noProject': 'no project',

    'stats.byDay': 'By day',
    'stats.window': 'Last {n} days',
    'stats.barTip': '{date}: {sessions} sessions, {messages} messages, input {input}, output {output}',

    'stats.savings': 'Savings',
    'stats.noData': 'no data',
    'stats.savedOf': 'of {total}',
    'stats.calls': 'calls: {calls}',

    'stats.byProject': 'By project',
    'stats.path': 'Path',
    'stats.inOut': 'in / out',
    'stats.notShown': 'Projects not shown: {n}',

    'stats.byModel': 'By model',
    'stats.model': 'Model',

    'stats.history': 'History',
    'stats.onDisk': 'On disk',
    'stats.olderThan': 'Older than {days} days',
    'stats.filesSize': 'files: {files} · {size}',
    'stats.cleanup': 'Clean up',
    'stats.cleanupArmed': 'Delete {files}?',
    'stats.cleanupAsk': 'Delete {files} files ({size})? Their sessions disappear from the statistics.',
    'stats.cleaned': 'Deleted files: {files} ({size})',
    'stats.cleanupFailed': 'Could not delete files: {failed}',
    'stats.cleanupNoDays': 'No retention was given to delete by',

    'stats.notes': 'Notes',
    'stats.note.rtkIdle': 'rtk has not run a command yet',
    'stats.note.noDir': 'Folder not found: {path}',
    'stats.note.noDbs': 'No databases in {path}',
    'stats.note.noEvents': 'No records in any database: {n}',

    'stats.tool.rtk': 'rtk',
    'stats.tool.context-mode': 'context-mode',
    'stats.tool.caveman': 'caveman',
  },
}
