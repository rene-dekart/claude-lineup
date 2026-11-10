import type { I18nModule } from '../index.js'

export const prof: I18nModule = {
  ru: {
    'prof.title': 'Профиль',
    'prof.ctxTotal':
      'Столько попадёт в контекст при старте: то, что профиль действительно включит, а не вся панель инвентаря. Размер известен у {known} из {all} записей — у MCP-сервера он появляется только после замера кнопкой ⚡.',
    'prof.ctxMeter': 'Контекст',
    'prof.ctxDelta':
      'На {delta} меньше, чем со всем включённым. Это и есть смысл конфигуратора: не абсолютная цифра, а разница.',
    'prof.ctxNoSession':
      'Отчёта /context в этом проекте нет. Запустите сессию с нужными настройками и выполните /context — тогда здесь появятся настоящие числа.',
    'prof.srcSession': 'число Claude Code из сессии',
    'prof.srcProbe': 'наш живой замер, символов / 4',
    'prof.srcSize': 'размер файла / 4',
    'prof.srcNone': 'не измерено',
    'prof.load': 'Загрузить',
    'prof.loadHint':
      'Список сохранённых профилей. Выбор загружает профиль в форму; внесённые после этого изменения сохраняются только по кнопке «Сохранить профиль». Кнопка «Новый» создаёт пустой профиль.',
    'prof.selectPlaceholder': '— выбрать —',
    'prof.new': 'Новый',
    'prof.idHint':
      'Служебный идентификатор профиля. Служит именем файла в каталоге data/profiles, поэтому допустимы только латинские буквы, цифры, точка, дефис и подчёркивание; остальные символы заменяются на подчёркивание. В команду запуска не попадает.',
    'prof.titleField': 'Название',
    'prof.titleHint':
      'Название профиля для человека. Показывается в списке профилей и в палитре поиска. На состав запускаемой конфигурации не влияет.',
    'prof.all': 'все',
    'prof.only': 'только',
    'prof.none': 'нет',
    'prof.mcpOnlyTitle': 'остальные — в disabledMcpServers',
    'prof.mcpDisableTitle': 'все серверы — в disabledMcpServers',
    'prof.skillsNoneTitle': 'в файле настроек невыразимо',
    'prof.mcpPickServers': 'выберите в инвентаре слева',
    'prof.mcpHint':
      'MCP-серверы. «все» — профиль про MCP молчит, списки в файлах не трогаются. «только» — отмеченные остаются, все прочие записываются выключенными. «нет» — выключенными записываются все.',
    'prof.skills': 'Навыки',
    'prof.skillsHint':
      'Навыки. «все» — доступны все найденные навыки, кроме выключенных в карточке. «нет» — в файле настроек невыразимо, профиль только предупредит. Отдельный навык настраивается в его карточке: четыре состояния skillOverrides. Важно: skillOverrides не действует на навыки плагинов — их гасит только выключение плагина целиком.',
    'prof.hooks': 'Обработчики',
    'prof.hooksAll': 'выключить все',
    'prof.hooksHint':
      'Отключение обработчиков. Доступно только «выключить все»: отдельного ключа для одного обработчика в Claude Code не существует.',
    'prof.memoryAuto': 'читать автоматически',
    'prof.memoryHint':
      'Автоматическое чтение CLAUDE.md, правил и авто-памяти. Ключа для отключения в файле настроек нет: профиль предупредит, что гасится это только флагом запуска, а не записью.',
    'prof.save': 'Сохранить профиль',
    'prof.delete': 'Удалить',
  },
  en: {
    'prof.title': 'Profile',
    'prof.ctxTotal':
      'This is what the launch will put in context: what the profile actually enables, not the whole inventory pane. The size is known for {known} of {all} entries — an MCP server only gets one after the ⚡ measurement.',
    'prof.ctxMeter': 'Context',
    'prof.ctxDelta':
      '{delta} less than with everything on. That difference, not the absolute number, is what a configurator is for.',
    'prof.ctxNoSession':
      'No /context report in this project yet. Run a session with the settings you want and call /context — real numbers will appear here.',
    'prof.srcSession': 'Claude Code\'s count from a session',
    'prof.srcProbe': 'our live measurement, chars / 4',
    'prof.srcSize': 'file size / 4',
    'prof.srcNone': 'not measured',
    'prof.load': 'Load',
    'prof.loadHint':
      'The list of saved profiles. Selecting one loads it into the form; changes made afterwards are kept only by pressing "Save profile". The "New" button creates an empty profile.',
    'prof.selectPlaceholder': '— select —',
    'prof.new': 'New',
    'prof.idHint':
      'The profile identifier. It serves as the file name in the data/profiles directory, so only Latin letters, digits, dot, hyphen and underscore are allowed; any other character becomes an underscore. It never reaches the launch command.',
    'prof.titleField': 'Title',
    'prof.titleHint':
      'The human-readable name of the profile. It appears in the profile list and in the search palette. It does not affect the configuration being launched.',
    'prof.all': 'all',
    'prof.only': 'only',
    'prof.none': 'none',
    'prof.mcpOnlyTitle': 'the rest go into disabledMcpServers',
    'prof.mcpDisableTitle': 'every server goes into disabledMcpServers',
    'prof.skillsNoneTitle': 'not expressible in a settings file',
    'prof.mcpPickServers': 'pick in the inventory on the left',
    'prof.mcpHint':
      'MCP servers. "all" means the profile is silent about MCP and the lists in the files are left alone. "only" keeps the ticked servers and writes every other one off. "none" writes them all off.',
    'prof.skills': 'Skills',
    'prof.skillsHint':
      'Skills. "all" makes every discovered skill available except the ones switched off in their card. "none" cannot be expressed in a settings file — all the profile does is warn. A single skill is set in its own card: the four skillOverrides states. One caveat: skillOverrides does not apply to plugin skills — only turning the plugin off does.',
    'prof.hooks': 'Hooks',
    'prof.hooksAll': 'disable all',
    'prof.hooksHint':
      'Disabling hooks. Only "disable all" is available: Claude Code has no key for a single hook.',
    'prof.memoryAuto': 'read automatically',
    'prof.memoryHint':
      'Automatic reading of CLAUDE.md, rules and auto-memory. A settings file has no key to switch this off: the profile only warns that a launch flag, not a write, is what does it.',
    'prof.save': 'Save profile',
    'prof.delete': 'Delete',
  },
}
