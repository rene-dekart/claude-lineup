import type { I18nModule } from '../index.js'

export const core: I18nModule = {
  ru: {
    'cost.unknownCount': '{n} элем., стоимость неизвестна',
    'cost.sum': 'сумма по {n} элем.',
    'io.readError': 'не читается: {err}',
    'io.badJson': 'битый JSON: {err}',
    'api.uiNotBuilt': 'UI не собран. Запустите npm run dev или npm run build.',
    'api.noRoute': 'нет маршрута {method} {path}',
  },
  en: {
    'cost.unknownCount': '{n} items, cost unknown',
    'cost.sum': 'sum of {n} items',
    'io.readError': 'cannot read: {err}',
    'io.badJson': 'broken JSON: {err}',
    'api.uiNotBuilt': 'The UI is not built. Run npm run dev or npm run build.',
    'api.noRoute': 'no route {method} {path}',
  },
}
