import type { I18nModule } from '../index.js'

export const net: I18nModule = {
  ru: {
    'net.indexDown': 'Внешний индекс недоступен ({reason}) — показан дисковый каталог.',
    'net.statsUnknown': 'Итог по индексу неизвестен: stats не ответил.',
    'net.noRatings':
      'У этих записей нет ни звёзд, ни установок, ни trustScore — сортировать по рейтингу нечего.',
    'net.pickNoPlugins': 'в манифесте нет ни одного плагина — ставить нечего',
    'net.pickManyPlugins':
      'в магазине плагинов {count} плагинов и ни одного с именем «{name}»: {list}. ' +
      'Имя нужно выбрать в каталоге на диске.',
    'net.unpinnedSha':
      'Версия не зафиксирована. Индекс отдаёт SHA файла ({sha}), но marketplace add ' +
      'принимает только ветку или тег — репозиторий склонируется на HEAD дефолтной ветки.',
    'net.unpinned': 'Версия не зафиксирована: репозиторий склонируется на HEAD дефолтной ветки.',
    'net.marketplaceAlreadyAdded':
      'Магазин плагинов «{name}» уже добавлен — первая ступень не нужна.',
    'net.manifestNotFound':
      'Манифест {repo} не найден ни по одному из четырёх путей ({paths}). ' +
      'Он появится на диске после первой ступени — имя плагина можно будет выбрать в каталоге.',
    'net.noMarketplaceName':
      'Не удалось определить имя магазина плагинов: второй ступени пока нет.',
    'net.cannotPickPlugin':
      'Не удалось выбрать плагин внутри магазина плагинов — уточните имя в каталоге на диске.',
    'net.marketplaceAddFailed': 'marketplace add не удался (код {code}).',
    'net.installFailed': 'install не удался (код {code}).',
    'net.marketplaceCommandAnnounced':
      'Магазин плагинов объявил установку через команду — покажите её человеку и подтвердите отдельно.',
    'net.badPluginId':
      'Некорректный id плагина: {id}. Ожидается "имя@магазин".',
    'net.badScope': 'Некорректная область установки: {scope}. Допустимо: {allowed}.',
    'net.badAcceptCommand':
      'acceptCommand должен быть sha256 в hex — CLI отказывает по любому другому.',
    'net.badMarketplaceSource':
      'Некорректный источник магазина плагинов: {source}. Ожидается owner/repo, git-URL, путь или URL.',
    'net.binNotStarted': '{bin} не запустился: {reason}',
    'net.timeout': 'не завершилось за {seconds} с — процесс убит',
    'net.mcpDown':
      'Реестр MCP недоступен ({reason}). В нём нет ни звёзд, ни установок — только ' +
      'имя, транспорт и репозиторий, поэтому пустой список означает именно отказ сети.',
  },
  en: {
    'net.indexDown': 'The external index is unavailable ({reason}) — showing the on-disk catalog.',
    'net.statsUnknown': 'The index total is unknown: stats did not respond.',
    'net.noRatings':
      'These records have no stars, installs or trustScore — there is nothing to sort by rating.',
    'net.pickNoPlugins': 'the manifest contains no plugins — there is nothing to install',
    'net.pickManyPlugins':
      'the marketplace has {count} plugins and none named "{name}": {list}. ' +
      'Pick the name in the on-disk catalog.',
    'net.unpinnedSha':
      'The version is not pinned. The index provides the file SHA ({sha}), but marketplace add ' +
      'only accepts a branch or tag — the repository will be cloned at the default branch HEAD.',
    'net.unpinned':
      'The version is not pinned: the repository will be cloned at the default branch HEAD.',
    'net.marketplaceAlreadyAdded':
      'The marketplace "{name}" is already added — the first stage is not needed.',
    'net.manifestNotFound':
      'The manifest for {repo} was not found at any of the four paths ({paths}). ' +
      'It will appear on disk after the first stage — the plugin name can then be picked in the catalog.',
    'net.noMarketplaceName':
      'Could not determine the marketplace name: there is no second stage yet.',
    'net.cannotPickPlugin':
      'Could not pick a plugin inside the marketplace — clarify the name in the on-disk catalog.',
    'net.marketplaceAddFailed': 'marketplace add failed (code {code}).',
    'net.installFailed': 'install failed (code {code}).',
    'net.marketplaceCommandAnnounced':
      'The marketplace announced installation via a command — show it to the human and confirm it separately.',
    'net.badPluginId':
      'Invalid plugin id: {id}. Expected "name@marketplace".',
    'net.badScope': 'Invalid scope: {scope}. Allowed: {allowed}.',
    'net.badAcceptCommand':
      'acceptCommand must be a sha256 in hex — the CLI rejects anything else.',
    'net.badMarketplaceSource':
      'Invalid marketplace source: {source}. Expected owner/repo, a git URL, a path or a URL.',
    'net.binNotStarted': '{bin} could not start: {reason}',
    'net.timeout': 'did not finish within {seconds} s — the process was killed',
    'net.mcpDown':
      'The MCP registry is unavailable ({reason}). It carries no stars and no installs — only ' +
      'a name, a transport and a repository — so an empty list means the network refused.',
  },
}
