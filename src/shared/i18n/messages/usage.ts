import type { I18nModule } from '../index.js'

/**
 * Usage: who was called, how often, and when.
 *
 * Names are shown as Claude Code wrote them — an MCP server as its server name,
 * a skill as it was invoked (`superpowers:brainstorming`). They are not translated:
 * a server has one name, and a translated one would not be the name you type.
 */
export const usage: I18nModule = {
  ru: {
    'usage.title': 'Использование',
    'usage.loading': 'Читаю транскрипты…',
    'usage.failed': 'Не удалось собрать использование: {message}',
    'usage.empty': 'Ни одного вызова инструментов в истории.',

    // Four captions, one per tile. The first is the sum of what the other three
    // hold only in the sense that a call came from one of them — a server, a skill
    // or a plugin; nothing is counted twice, since a plugin is derived from the
    // names of its own servers and skills rather than reported separately.
    'usage.calls': 'Вызовы',
    'usage.mcp': 'MCP-серверы',
    'usage.skills': 'Скиллы',
    'usage.plugins': 'Плагины',

    'usage.first': 'Первый вызов: {date}',
    'usage.inSessions': 'Сессий: {n}',

    'usage.byDay': 'Вызовы по дням',
    'usage.window': 'Последние {n} дней',
    'usage.barTip': '{date}: вызовов {calls}, сессий {sessions}',

    'usage.byMcp': 'MCP-серверы',
    'usage.bySkill': 'Скиллы',
    'usage.byPlugin': 'Плагины',

    'usage.name': 'Имя',
    'usage.colSessions': 'Сессии',
    'usage.colLast': 'Последний раз',
    'usage.never': 'ни разу',
  },
  en: {
    'usage.title': 'Usage',
    'usage.loading': 'Reading transcripts…',
    'usage.failed': 'Could not collect the usage: {message}',
    'usage.empty': 'No tool calls in the history.',

    'usage.calls': 'Calls',
    'usage.mcp': 'MCP servers',
    'usage.skills': 'Skills',
    'usage.plugins': 'Plugins',

    'usage.first': 'First call: {date}',
    'usage.inSessions': 'Sessions: {n}',

    'usage.byDay': 'Calls per day',
    'usage.window': 'Last {n} days',
    'usage.barTip': '{date}: {calls} calls, {sessions} sessions',

    'usage.byMcp': 'MCP servers',
    'usage.bySkill': 'Skills',
    'usage.byPlugin': 'Plugins',

    'usage.name': 'Name',
    'usage.colSessions': 'Sessions',
    'usage.colLast': 'Last used',
    'usage.never': 'never',
  },
}
