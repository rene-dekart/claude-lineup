import type { I18nModule } from '../index.js'

/** Messages of the inventory scan: catalog, hooks, MCP, memory, skills. */
export const scan: I18nModule = {
  ru: {
    'scan.catalogNoTokenFigures': 'каталог не дал цифр по токенам',
    'scan.catalogCost': 'каталог Anthropic · {model}: всегда ~{always}{invoke}',
    'scan.catalogCostOnInvoke': ', при вызове ~{tokens}',
    'scan.manifestNoCost': 'в манифесте нет данных о стоимости',
    'scan.catalogCacheUnread':
      'Кэш каталога не прочитан ({file}): {error}. Неустановленные плагины будут видны только из манифестов магазинов плагинов.',
    'scan.marketplaceNoManifest': 'Магазин плагинов «{name}»: манифест не найден — его плагины не видны.',
    'scan.marketplaceReadError': 'Магазин плагинов «{name}»: {error}.',
    'scan.onDisk': 'на диске',

    'scan.hookSessionStartCost': 'вывод идёт в контекст и режется на 10 000 символов',
    'scan.hookCost': 'вывод в контекст не попадает',
    'scan.unreadableFiles': 'Не прочитано файлов: {count}. Инвентарь по ним неполный.',

    'scan.mcpFromPlugin': 'из плагина {id}: {desc}',
    'scan.mcpTools': '{count} инстр.: {head}',
    'scan.mcpFromSession':
      'число Claude Code из отчёта /context за {date}: столько заняли инструменты и инструкции этого сервера в той сессии',
    'scan.memoryFromSession': 'число Claude Code из отчёта /context за {date}',
    'scan.memoryFromSessionExtra':
      'подхвачен сессией и не виден в обходе: импорт через @ или авто-память, число из отчёта /context за {date}',

    'scan.probeNotMeasured': 'не измерено — нажмите «Измерить»',
    'scan.probeMeasuredNote': 'tools/list и инструкции сервера, замер {date}',
    'scan.probeServerNotFound': 'сервер "{name}" не найден в ~/.claude.json',
    'scan.probeHttpUnsupported': 'http-транспорт: живой замер не поддержан',
    'scan.probeNoResponse': 'сервер не ответил на tools/list',
    'scan.probeLiveMeasure': 'живой замер tools/list и инструкций сервера',

    'scan.memoryGlobal': 'глобальная память, грузится всегда, размер файла / 4',
    'scan.memoryProject': 'память проекта, размер файла / 4',
    'scan.memoryParent': 'родительский каталог — грузится всегда, размер файла / 4',

    'scan.skillNoDescription': 'описания нет',
    'scan.skillFrontmatterCost': 'frontmatter description / 4',
  },
  en: {
    'scan.catalogNoTokenFigures': 'the catalog returned no token figures',
    'scan.catalogCost': 'Anthropic catalog · {model}: always ~{always}{invoke}',
    'scan.catalogCostOnInvoke': ', ~{tokens} on invoke',
    'scan.manifestNoCost': 'the manifest has no cost data',
    'scan.catalogCacheUnread':
      'Catalog cache unread ({file}): {error}. Uninstalled plugins will only be visible from marketplace manifests.',
    'scan.marketplaceNoManifest': 'Marketplace "{name}": manifest not found — its plugins are not visible.',
    'scan.marketplaceReadError': 'Marketplace "{name}": {error}.',
    'scan.onDisk': 'on disk',

    'scan.hookSessionStartCost': 'output goes into context and is truncated at 10,000 characters',
    'scan.hookCost': 'output does not reach context',
    'scan.unreadableFiles': 'Unreadable files: {count}. The inventory based on them is incomplete.',

    'scan.mcpFromPlugin': 'from plugin {id}: {desc}',
    'scan.mcpTools': '{count} tools: {head}',
    'scan.mcpFromSession':
      'Claude Code\'s count from the /context report of {date}: what this server\'s tools and instructions took in that session',
    'scan.memoryFromSession': 'Claude Code\'s count from the /context report of {date}',
    'scan.memoryFromSessionExtra':
      'picked up by the session and invisible to a scan: an @import or auto-memory, count from the /context report of {date}',

    'scan.probeNotMeasured': 'not measured — click "Measure"',
    'scan.probeMeasuredNote': 'tools/list and server instructions, measured {date}',
    'scan.probeServerNotFound': 'server "{name}" not found in ~/.claude.json',
    'scan.probeHttpUnsupported': 'http transport: live measurement not supported',
    'scan.probeNoResponse': 'the server did not respond to tools/list',
    'scan.probeLiveMeasure': 'live measurement of tools/list and server instructions',

    'scan.memoryGlobal': 'global memory, always loaded, file size / 4',
    'scan.memoryProject': 'project memory, file size / 4',
    'scan.memoryParent': 'parent directory — always loaded, file size / 4',

    'scan.skillNoDescription': 'no description',
    'scan.skillFrontmatterCost': 'frontmatter description / 4',
  },
}
