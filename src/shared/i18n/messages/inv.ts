import type { I18nModule } from '../index.js'

export const inv: I18nModule = {
  ru: {
    'inv.title': 'Инвентарь',
    'inv.noProject':
      'Укажите путь к проекту вверху — тогда станет видно, что реально грузится ' +
      'в его сессиях.',
    'inv.elements': '{n} элем.',
    'inv.kindMcp': 'MCP-серверы',
    'inv.kindPlugin': 'Плагины',
    'inv.kindSkill': 'Навыки',
    'inv.kindMemory': 'Память',
    'inv.kindPermissions': 'Права',
    'inv.permFiles': 'Файлы',
    'inv.permRepo': 'Репозиторий',
    'inv.permSystem': 'Система и сеть',
    'inv.permData': 'Данные и сервисы',
    'inv.ruleAllow': 'Разрешить',
    'inv.ruleAsk': 'Спрашивать',
    'inv.ruleDeny': 'Запретить',
    'inv.ruleAllowTitle':
      'Разрешить: правила не будет — найденное в файлах снимается, новое не пишется. ' +
      'Правила: {rules}',
    'inv.ruleAskTitle': 'Спрашивать: спросить перед вызовом. В файл пойдёт ask: {rules}',
    'inv.ruleDenyTitle': 'Запретить: не вызывать. В файл пойдёт deny: {rules}',
    'inv.ruleScope': 'В файлах сейчас: {state} — задано на уровне «{scope}»',
    'inv.ruleScopeLocked':
      'В файлах сейчас: {state} — задано на уровне «{scope}». Профиль пишет только в проектный ' +
      'файл, оттуда правило не снять',
    'inv.ruleChanged': 'изменено',
    'inv.ruleChangedTitle':
      'В файлах сейчас: {state}. В профиле выбрано другое — применится по кнопке «Применить файлы»',
    'inv.toggleGroup': 'Свернуть или развернуть: {name}',
    'inv.probeTitle': 'Запустить сервер и замерить tools/list',
    'inv.mcpOffBy': 'выключен: {where}',
    'inv.mcpOffByTitle':
      'Этот сервер называют в файлах настроек — Claude Code его не запускает. ' +
      'Ключ disabledMcpServers лежит в записи проекта внутри ~/.claude.json, ' +
      'disabledMcpjsonServers — в самом файле настроек.',
    'inv.skillsAbbrev': '{n} нав.',
    'inv.catalogNotInstalled': 'Каталог · не установлено',
    'inv.indexNetwork': 'Индекс · сеть',
    'inv.netTitle': 'Внешний индекс skillselion: репозитории, которых нет на машине',
    'inv.mcpRegistryTitle':
      'Официальный реестр MCP: имя, транспорт и репозиторий. Рейтингов в нём нет — ' +
      'отсортировать нечего, и порядок такой, какой отдал реестр. Отсюда не ставится: ' +
      'единственный выход из строки — репозиторий.',
    'inv.searchPlugins': 'Поиск по каталогу плагинов',
    'inv.searchRegistry': 'Поиск по каталогу MCP',
    'inv.mcpSecret': 'нужен ключ',
    'inv.mcpSecretTitle':
      'Подключению нужен секрет: токен или ключ, которого автор в комплекте не даёт',
    'inv.sortTitle': 'У большинства записей индекса рейтингов нет — тогда порядок решает имя',
    'inv.sortStars': 'звёзды',
    'inv.sortInstalls': 'установки',
    'inv.sortTrust': 'доверие',
    'inv.sortName': 'имя',
    'inv.scopeTitle': 'Куда ставить: user — глобально, project/local — только этот проект',
    'inv.planHead': 'План установки',
    'inv.planRest': '— ничего ещё не запущено, выполнят эти команды:',
    'inv.pluginLabel': 'плагин: {id}',
    'inv.runPlanTitle': 'Выполнить обе ступени через claude CLI',
    'inv.planIncompleteTitle': 'План неполный: вторая ступень не определена',
    'inv.runViaCli': 'Выполнить через CLI',
    'inv.cancel': 'Отмена',
    'inv.marketHead': 'Магазин плагинов объявил установку',
    'inv.marketRest': 'через команду. Она выполнится на этой машине:',
    'inv.allowAndInstall': 'Разрешить и установить',
    'inv.installs': '{n} уст.',
    'inv.installTitle': 'Установить через claude plugin install --scope {scope}',
    'inv.install': 'Установить',
    'inv.planTitle': 'План: claude plugin marketplace add {repo} → install',
    'inv.indexEmpty': 'Индекс ничего не отдал по этому запросу.',
    'inv.indexEmptyBare':
      'Индекс ничего не отдал по этому запросу ' +
      '(без запроса он тоже пуст — попробуйте слово).',
    'inv.partial':
      'Показаны не все: под запрос подошло {total}, показано {shown}. ' +
      'Уточните поиск.',
    'inv.hasMore': 'Индекс отдал порцию и говорит, что записей больше — уточните поиск.',
  },
  en: {
    'inv.title': 'Inventory',
    'inv.noProject':
      'Set a project path above to see what actually loads in its sessions.',
    'inv.elements': '{n} items',
    'inv.kindMcp': 'MCP servers',
    'inv.kindPlugin': 'Plugins',
    'inv.kindSkill': 'Skills',
    'inv.kindMemory': 'Memory',
    'inv.kindPermissions': 'Permissions',
    'inv.permFiles': 'Files',
    'inv.permRepo': 'Repository',
    'inv.permSystem': 'System and network',
    'inv.permData': 'Data and services',
    'inv.ruleAllow': 'Allow',
    'inv.ruleAsk': 'Ask',
    'inv.ruleDeny': 'Deny',
    'inv.ruleAllowTitle':
      'Allow: no rule at all — what the files say is removed, nothing new is written. ' +
      'Rules: {rules}',
    'inv.ruleAskTitle': 'Ask: prompt before the call. The file gets ask: {rules}',
    'inv.ruleDenyTitle': 'Deny: never call it. The file gets deny: {rules}',
    'inv.ruleScope': 'The files say {state} — set at the {scope} level',
    'inv.ruleScopeLocked':
      'The files say {state} — set at the {scope} level. The profile only writes the project ' +
      'file, so the rule cannot be taken out from there',
    'inv.ruleChanged': 'changed',
    'inv.ruleChangedTitle':
      'The files say {state}. The profile asks for something else — «Apply files» makes it so',
    'inv.toggleGroup': 'Collapse or expand: {name}',
    'inv.probeTitle': 'Start the server and measure tools/list',
    'inv.mcpOffBy': 'off: {where}',
    'inv.mcpOffByTitle':
      'The settings files name this server — Claude Code never starts it. ' +
      'disabledMcpServers sits in the project entry inside ~/.claude.json, ' +
      'disabledMcpjsonServers in the settings file itself.',
    'inv.skillsAbbrev': '{n} sk.',
    'inv.catalogNotInstalled': 'Catalog · not installed',
    'inv.indexNetwork': 'Index · network',
    'inv.netTitle': 'External skillselion index: repositories not on this machine',
    'inv.mcpRegistryTitle':
      'The official MCP registry: name, transport and repository. It has no ratings — ' +
      'nothing to sort by, so the order is the one the registry returned. Nothing installs ' +
      'from here: the only way out of a row is the repository.',
    'inv.searchPlugins': 'Search the plugin catalog',
    'inv.searchRegistry': 'Search the MCP catalog',
    'inv.mcpSecret': 'needs a key',
    'inv.mcpSecretTitle':
      'Connecting needs a secret: a token or a key the author does not ship with the server',
    'inv.sortTitle': 'Most index entries have no ratings — the name decides the order then',
    'inv.sortStars': 'stars',
    'inv.sortInstalls': 'installs',
    'inv.sortTrust': 'trust',
    'inv.sortName': 'name',
    'inv.scopeTitle': 'Where to install: user — globally, project/local — this project only',
    'inv.planHead': 'Install plan for',
    'inv.planRest': '— nothing has run yet; these commands will:',
    'inv.pluginLabel': 'plugin: {id}',
    'inv.runPlanTitle': 'Run both stages via the claude CLI',
    'inv.planIncompleteTitle': 'Incomplete plan: the second stage is undefined',
    'inv.runViaCli': 'Run via CLI',
    'inv.cancel': 'Cancel',
    'inv.marketHead': 'The marketplace announced an install of',
    'inv.marketRest': 'via a command. It will run on this machine:',
    'inv.allowAndInstall': 'Allow and install',
    'inv.installs': '{n} installs',
    'inv.installTitle': 'Install via claude plugin install --scope {scope}',
    'inv.install': 'Install',
    'inv.planTitle': 'Plan: claude plugin marketplace add {repo} → install',
    'inv.indexEmpty': 'The index returned nothing for this query.',
    'inv.indexEmptyBare':
      'The index returned nothing for this query ' +
      '(it is also empty without one — try a word).',
    'inv.partial':
      'Not all shown: {total} matched, {shown} displayed. Narrow your search.',
    'inv.hasMore':
      'The index returned a batch and says there are more records — narrow your search.',
  },
}
