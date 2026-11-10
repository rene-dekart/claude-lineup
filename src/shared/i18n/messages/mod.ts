import type { I18nModule } from '../index.js'

/**
 * The module catalog, and the pages the modules themselves draw.
 *
 * A catalog row carries two facts and no third: whether this machine has the
 * tool, and whether the human switched the module on. `not-found` is an answer
 * rather than an error, so it is stated plainly, with the command that changes it.
 */
export const mod: I18nModule = {
  ru: {
    // The catalog
    'mod.title': 'Модули',
    'mod.head': 'Необязательные модули',
    'mod.col.module': 'Модуль',
    'mod.col.tool': 'Инструмент',
    'mod.col.action': 'Действие',
    'mod.state.found': 'найден',
    'mod.state.notFound': 'не найден',
    // Nobody ran the tool check yet, and — separately — it is running right now.
    // A dash would say the same thing as "not found", and that would be a lie.
    'mod.state.unknown': 'не проверялся',
    'mod.state.checking': 'проверяется…',
    'mod.on': 'включён',
    'mod.off': 'выключен',
    'mod.enable': 'Включить',
    'mod.disable': 'Выключить',
    'mod.unknown': 'Модуль «{id}» не существует',
    'mod.disabled': 'Модуль «{id}» выключен',
    'mod.empty': 'Ни одного модуля',

    // openkb: what the knowledge base holds
    'mod.kb.title': 'openkb — база знаний',
    'mod.kb.noRoot': 'openkb не называет базу по умолчанию',
    'mod.kb.refresh': 'Обновить',
    'mod.kb.docs': 'Документы',
    'mod.kb.entities': 'Сущности',
    'mod.kb.concepts': 'Понятия',
    'mod.kb.counts': 'raw {raw} · wiki {wiki} · summaries {summaries} · sources {sources}',
    'mod.kb.links': '{in} ← {out}',
    'mod.kb.empty': 'Пусто',
    'mod.kb.pick': 'Выберите страницу слева',
    'mod.kb.bytes': '{n} Б',
    // A list arrives a hundred rows at a time and the screen keeps exactly those:
    // the figures are the only way to tell a short list from the end of a long one.
    'mod.kb.pageRange': '{from}–{to} из {total}',
    'mod.kb.prev': 'Назад',
    'mod.kb.next': 'Вперёд',

    // openkb: search
    'mod.kb.search': 'Поиск',
    'mod.kb.searchPh': 'слово по всем страницам wiki',
    'mod.kb.hits': 'совпадений: {n}',
    'mod.kb.noHits': 'Ничего не найдено',

    // openkb: adding to the base. Only `openkb add` writes — the wiki's index is
    // theirs, and a hand-edited page silently breaks it.
    'mod.kb.add': 'Добавить',
    'mod.kb.addTitle': 'Добавить в базу',
    // Two ways in, one dialog: a path comes from the desktop's picker, a link is
    // typed. The picker cannot choose a URL and a field cannot choose a file.
    'mod.kb.srcLocal': 'С компьютера',
    'mod.kb.srcUrl': 'Ссылка',
    'mod.kb.pickFile': 'Файл…',
    'mod.kb.pickDir': 'Папка…',
    'mod.kb.nothingPicked': 'Ничего не выбрано',
    'mod.kb.urlPh': 'https://…',
    'mod.kb.cancel': 'Отмена',
    'mod.kb.badKind': 'Неизвестный источник: путь берётся файлом или папкой',
    'mod.kb.adding': 'openkb add: идёт компиляция…',
    'mod.kb.addOk': 'openkb add завершился, код {code}',
    'mod.kb.addFail': 'openkb add вернул ошибку, код {code}',
    'mod.kb.badArg': 'Аргумент не принят: нужен существующий абсолютный путь или http(s)-адрес',
  },
  en: {
    // The catalog
    'mod.title': 'Modules',
    'mod.head': 'Optional modules',
    'mod.col.module': 'Module',
    'mod.col.tool': 'Tool',
    'mod.col.action': 'Action',
    'mod.state.found': 'found',
    'mod.state.notFound': 'not found',
    'mod.state.unknown': 'not checked',
    'mod.state.checking': 'checking…',
    'mod.on': 'on',
    'mod.off': 'off',
    'mod.enable': 'Turn on',
    'mod.disable': 'Turn off',
    'mod.unknown': 'No module "{id}"',
    'mod.disabled': 'Module "{id}" is off',
    'mod.empty': 'No modules',

    // openkb: what the knowledge base holds
    'mod.kb.title': 'openkb — knowledge base',
    'mod.kb.noRoot': 'openkb names no default knowledge base',
    'mod.kb.refresh': 'Reload',
    'mod.kb.docs': 'Documents',
    'mod.kb.entities': 'Entities',
    'mod.kb.concepts': 'Concepts',
    'mod.kb.counts': 'raw {raw} · wiki {wiki} · summaries {summaries} · sources {sources}',
    'mod.kb.links': '{in} ← {out}',
    'mod.kb.empty': 'Empty',
    'mod.kb.pick': 'Pick a page on the left',
    'mod.kb.bytes': '{n} B',
    'mod.kb.pageRange': '{from}–{to} of {total}',
    'mod.kb.prev': 'Back',
    'mod.kb.next': 'Next',

    // openkb: search
    'mod.kb.search': 'Search',
    'mod.kb.searchPh': 'a word across every wiki page',
    'mod.kb.hits': 'hits: {n}',
    'mod.kb.noHits': 'Nothing found',

    // openkb: adding to the base
    'mod.kb.add': 'Add',
    'mod.kb.addTitle': 'Add to the base',
    'mod.kb.srcLocal': 'From this computer',
    'mod.kb.srcUrl': 'Link',
    'mod.kb.pickFile': 'File…',
    'mod.kb.pickDir': 'Folder…',
    'mod.kb.nothingPicked': 'Nothing picked',
    'mod.kb.urlPh': 'https://…',
    'mod.kb.cancel': 'Cancel',
    'mod.kb.badKind': 'Unknown source: a path is a file or a folder',
    'mod.kb.adding': 'openkb add: compiling…',
    'mod.kb.addOk': 'openkb add finished, code {code}',
    'mod.kb.addFail': 'openkb add failed, code {code}',
    'mod.kb.badArg': 'Argument refused: needs an existing absolute path or an http(s) URL',
  },
}
