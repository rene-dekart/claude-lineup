import type { I18nModule } from '../index.js'

/**
 * The app shell: topbar controls, the status line and the result pane.
 * Interpolated messages are single templates with `{name}` placeholders.
 */
export const ui: I18nModule = {
  ru: {
    // Shell controls
    'ui.newProfile': 'Новый профиль',
    // The rail's first destination: the three panes, the app's main screen.
    'ui.settings': 'Настройки',
    'ui.tagline': 'настройка Claude Code',
    'ui.cwdPlaceholder': 'Путь к проекту, например D:\\Projects\\foo',
    'ui.cwdPick': 'Выбрать папку',
    'ui.cwdType': 'Ввести путь вручную',
    'ui.reload': 'Обновить',
    'ui.theme': 'Тема',
    // The OS picker dialog, shown by the desktop the server runs on
    'dlg.title': 'Папка проекта',
    'dlg.fileTitle': 'Файл для добавления в базу',
    'dlg.unsupported': 'Выбор пути работает только на Windows.',
    'dlg.failed': 'Не удалось открыть диалог: {error}',
    'dlg.timeout': 'Диалог не закрылся за {seconds} с — закрываю.',
    // Status: inventory, profiles, apply
    'ui.inventoryError': 'Ошибка инвентаря: {message}',
    'ui.profilesUnreadable': 'Профили не читаются: {message}',
    'ui.resolveFailed': 'Разбор не удался: {message}',
    'ui.profileSaved': 'Профиль «{title}» сохранён.',
    'ui.saveFailed': 'Не сохранился: {message}',
    'ui.applyRejected': 'Применение отклонено: {message}',
    'ui.filesWritten': 'Записано файлов: {count}. Резервных копий: {backups}.',
    // Status and list: rolling a write back
    'ui.history': 'История',
    'ui.historyTitle': 'История записей',
    'ui.historyEmpty': 'Записей ещё не было.',
    'ui.historyFiles': 'файлов: {n}',
    'ui.historyProfile': 'профиль',
    'ui.undo': 'Откатить',
    'ui.undone': 'Откатил: вернул {restored}, удалил {deleted}.',
    'ui.historyDelete': 'Удалить эту запись — файлы проекта не трогаются',
    'ui.historyDeleted': 'Запись удалена: {id}.',
    'ui.historyClear': 'Очистить историю',
    'ui.historyClearArmed': 'Очистить всё?',
    'ui.historyClearTitle':
      'Удалить все записи истории. Сами файлы проектов останутся как есть, но ' +
      'откатить эти записи будет уже нельзя.',
    'ui.historyCleared': 'История очищена: удалено записей — {n}.',
    'ui.catalogUnreadable': 'Каталог не читается: {message}',
    'ui.indexUnreadable': 'Индекс не читается: {message}',
    // Status: plans and installs
    'ui.planBuilding': 'Строю план для {repo}…',
    'ui.planReady': 'План готов: {steps} ступеней.',
    'ui.planIncomplete': 'План неполный: {error}',
    'ui.planFailed': 'План не построился: {message}',
    'ui.marketplacePending':
      'Магазин плагинов объявил установку через команду — показываю её, нужно подтверждение.',
    'ui.pendingConfirm':
      '{id}: магазин плагинов объявил установку через команду — показываю её, нужно подтверждение.',
    'ui.installFailed': 'Не удалось: {error}',
    'ui.installFailedId': '{id}: не удалось. {error}',
    'ui.installFailedCode': '{id}: установка не удалась (код {code}). {output}',
    'ui.seeCommandOutput': 'см. вывод команд',
    'ui.installed': '{name} установлен.',
    'ui.installedAndAdded': '{id} установлен и добавлен в профиль.',
    'ui.installRejected': 'Установка отклонена: {message}',
    'ui.installing': 'Ставлю {id} (scope: {scope})…',
    // Status: probes
    'ui.probeStarting': 'Запускаю {name} для замера…',
    'ui.probeResult': 'Замер {name}: {tokens} токенов ({note})',
    'ui.probeFailed': 'Замер не удался: {message}',
    // Result pane
    'ui.lines': '{n} строк',
    'ui.foldLines': '⋯ {n} строк без изменений',
    'ui.patchDraft': 'патч профиля (до слияния)',
    // The pane reads the machine three ways: what the profile asks for, what the
    // files hold, and one of those files whole. The buttons label the switch;
    // only the third needs a word about what it leaves out.
    'ui.viewDraft': 'Профиль',
    'ui.viewFull': 'Полная конфигурация',
    'ui.viewClaudeJson': 'claude.json',
    'ui.claudeJsonHint':
      'Тот же файл, что и в «Полной конфигурации», но целиком: глобальные mcpServers и прочее машинное, что в той вкладке не видно. Служебные ключи Claude Code — счётчики, кеши, флаги онбординга — скрыты, а из projects оставлен только этот проект.',
    'ui.claudeJsonEmpty': 'Файл ~/.claude.json не читается.',
    'ui.configLoading': 'Читаю файлы…',
    'ui.configEmpty': 'Файлов настроек нет.',
    'ui.configFailed': 'Файлы не читаются: {message}',
    'ui.build': 'Собрать',
    'ui.applyFiles': 'Применить файлы',
    // ⌘K palette
    'ui.search': 'Поиск',
    'ui.searchPlaceholder': 'Найти сервер, плагин, навык или профиль…',
    'ui.searchEmpty': 'Ничего не нашлось.',
    'ui.hintNav': 'навигация',
    'ui.hintOpen': 'включить или открыть',
    'ui.hintApply': 'применить профиль',
    'ui.hintClose': 'закрыть',
    'ui.kindProfile': 'Профиль',
    'ui.chipOn': 'вкл',
    'ui.chipOff': 'выкл',
    'ui.profileLoaded': 'Профиль «{title}» загружен.',
    'ui.resize': 'Тянуть, чтобы изменить ширину',
  },
  en: {
    // Shell controls
    'ui.newProfile': 'New profile',
    'ui.settings': 'Settings',
    'ui.tagline': 'configure Claude Code',
    'ui.cwdPlaceholder': 'Project path, e.g. D:\\Projects\\foo',
    'ui.cwdPick': 'Pick a folder',
    'ui.cwdType': 'Type the path by hand',
    'ui.reload': 'Refresh',
    'ui.theme': 'Theme',
    // The OS folder picker dialog, shown by the desktop the server runs on
    'dlg.title': 'Project folder',
    'dlg.fileTitle': 'File to add to the knowledge base',
    'dlg.unsupported': 'The path picker only works on Windows.',
    'dlg.failed': 'Could not open the dialog: {error}',
    'dlg.timeout': 'The dialog did not finish in {seconds} s — closing it.',
    // Status: inventory, profiles, apply
    'ui.inventoryError': 'Inventory error: {message}',
    'ui.profilesUnreadable': 'Cannot read profiles: {message}',
    'ui.resolveFailed': 'Resolve failed: {message}',
    'ui.profileSaved': 'Profile "{title}" saved.',
    'ui.saveFailed': 'Not saved: {message}',
    'ui.applyRejected': 'Apply rejected: {message}',
    'ui.filesWritten': 'Files written: {count}. Backups: {backups}.',
    // Status and list: rolling a write back
    'ui.history': 'History',
    'ui.historyTitle': 'Write history',
    'ui.historyEmpty': 'Nothing has been written yet.',
    'ui.historyFiles': 'files: {n}',
    'ui.historyProfile': 'profile',
    'ui.undo': 'Roll back',
    'ui.undone': 'Rolled back: {restored} returned, {deleted} deleted.',
    'ui.historyDelete': 'Delete this entry — project files are left alone',
    'ui.historyDeleted': 'Entry deleted: {id}.',
    'ui.historyClear': 'Clear history',
    'ui.historyClearArmed': 'Clear everything?',
    'ui.historyClearTitle':
      'Delete every entry in the history. The project files stay as they are, ' +
      'but these writes can no longer be rolled back.',
    'ui.historyCleared': 'History cleared: {n} entries deleted.',
    'ui.catalogUnreadable': 'Cannot read the catalog: {message}',
    'ui.indexUnreadable': 'Cannot read the index: {message}',
    // Status: plans and installs
    'ui.planBuilding': 'Building a plan for {repo}…',
    'ui.planReady': 'Plan ready: {steps} steps.',
    'ui.planIncomplete': 'Plan incomplete: {error}',
    'ui.planFailed': 'Plan was not built: {message}',
    'ui.marketplacePending':
      'The marketplace declared a command install — showing it, confirmation needed.',
    'ui.pendingConfirm':
      '{id}: the marketplace declared a command install — showing it, confirmation needed.',
    'ui.installFailed': 'Failed: {error}',
    'ui.installFailedId': '{id}: failed. {error}',
    'ui.installFailedCode': '{id}: install failed (code {code}). {output}',
    'ui.seeCommandOutput': 'see command output',
    'ui.installed': '{name} installed.',
    'ui.installedAndAdded': '{id} installed and added to the profile.',
    'ui.installRejected': 'Install rejected: {message}',
    'ui.installing': 'Installing {id} (scope: {scope})…',
    // Status: probes
    'ui.probeStarting': 'Probing {name}…',
    'ui.probeResult': 'Probe {name}: {tokens} tokens ({note})',
    'ui.probeFailed': 'Probe failed: {message}',
    // Result pane
    'ui.lines': '{n} lines',
    'ui.foldLines': '⋯ {n} unchanged lines',
    'ui.patchDraft': 'profile patch (pre-merge)',
    // The pane reads the machine two ways: what the profile asks for, and what
    // the files hold. Both label the switch; neither explains itself.
    'ui.viewDraft': 'Profile',
    'ui.viewFull': 'Full configuration',
    'ui.viewClaudeJson': 'claude.json',
    'ui.claudeJsonHint':
      'The same file the "Full configuration" tab reads, but whole: the global mcpServers and the rest of what the machine holds, none of which that tab shows. Claude Code\'s own bookkeeping — counters, caches, onboarding flags — is hidden, and projects is narrowed to this project.',
    'ui.claudeJsonEmpty': 'Cannot read ~/.claude.json.',
    'ui.configLoading': 'Reading files…',
    'ui.configEmpty': 'No settings files.',
    'ui.configFailed': 'Cannot read the files: {message}',
    'ui.build': 'Build',
    'ui.applyFiles': 'Apply files',
    // ⌘K palette
    'ui.search': 'Search',
    'ui.searchPlaceholder': 'Find a server, plugin, skill or profile…',
    'ui.searchEmpty': 'Nothing found.',
    'ui.hintNav': 'navigate',
    'ui.hintOpen': 'toggle or open',
    'ui.hintApply': 'apply profile',
    'ui.hintClose': 'close',
    'ui.kindProfile': 'Profile',
    'ui.chipOn': 'on',
    'ui.chipOff': 'off',
    'ui.profileLoaded': 'Profile "{title}" loaded.',
    'ui.resize': 'Drag to resize',
  },
}
