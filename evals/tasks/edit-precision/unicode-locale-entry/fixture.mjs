// Builds a large localization file with accented Latin, CJK, Korean, Arabic, Hebrew, Cyrillic and emoji text.

export const FILE_NAME = "locales.json";
export const TARGET_LOCALE = "ja";
export const TARGET_KEY = "settings.language.title";
export const OLD_VALUE = "言語";
export const NEW_VALUE = "言語と地域 🌏";
const TARGET_INDEX = 120;
const KEY_COUNT = 260;

const SECTIONS = ["settings", "cart", "checkout", "profile", "orders", "search", "help", "errors", "emails", "nav", "footer", "admin", "billing"];
const ITEMS = ["title", "subtitle", "label", "hint", "button", "tooltip", "placeholder", "empty", "success", "failure", "heading", "caption", "link", "note", "summary", "confirm", "cancel", "retry", "details", "status"];

const WORDS = {
  en: ["Settings ⚙️", "Log out 👋", "Save 💾", "Family 👨‍👩‍👧‍👦", "Done ✅", "Search 🔍", "Welcome", "Orders"],
  fr: ["Paramètres", "Déconnexion", "Réinitialiser", "Café crème", "Été", "Œuvre", "Où êtes-vous ?", "À bientôt", "Ça marche"],
  de: ["Einstellungen", "Schließen", "Größe", "Übersicht", "Änderungen", "Fußgängerübergang", "Bestätigen"],
  es: ["Configuración", "Cerrar sesión", "¿Estás seguro?", "Año nuevo", "Niño", "Descripción", "Envío"],
  ja: ["設定", "ログアウト", "保存", "キャンセル", "ようこそ", "お支払い", "注文履歴", "検索"],
  zh: ["设置", "退出登录", "保存", "取消", "欢迎", "付款", "订单历史", "搜索"],
  ko: ["설정", "로그아웃", "저장", "취소", "환영합니다", "결제", "주문 내역", "검색"],
  ar: ["الإعدادات", "تسجيل الخروج", "حفظ", "إلغاء", "مرحبا", "الدفع", "سجل الطلبات", "بحث"],
  he: ["הגדרות", "התנתקות", "שמירה", "ביטול", "ברוכים הבאים", "תשלום", "היסטוריית הזמנות", "חיפוש"],
  ru: ["Настройки", "Выйти", "Сохранить", "Отмена", "Добро пожаловать", "Оплата", "История заказов", "Поиск"],
};
export const LOCALES = Object.keys(WORDS);

/**
 * Returns a deterministic pseudo random number generator.
 * @param seed - integer seed
 */
function mulberry32(seed) {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * Builds the list of unique entry keys, with the target key at its fixed position.
 */
function buildKeys() {
  const keys = [];
  for (const section of SECTIONS) for (const item of ITEMS) keys.push(`${section}.${item}`);
  const chosen = keys.filter((key) => key !== TARGET_KEY).slice(0, KEY_COUNT - 1);
  chosen.splice(TARGET_INDEX, 0, TARGET_KEY);
  return chosen;
}

/**
 * Builds one translated value for a locale.
 * @param locale - locale code
 * @param random - the generator
 */
function buildValue(locale, random) {
  const words = WORDS[locale];
  const count = 1 + Math.floor(random() * 3);
  const picked = [];
  for (let index = 0; index < count; index += 1) picked.push(words[Math.floor(random() * words.length)]);
  let value = picked.join(" ");
  if (random() < 0.2) value += " {count}";
  if (random() < 0.15 && (locale === "ar" || locale === "he")) value = "‏" + value + "‏";
  if (random() < 0.1) value += " — é";
  return value;
}

/**
 * Builds the JSON text of one locale block.
 * @param locale - locale code
 * @param keys - the entry keys
 * @param random - the generator
 */
function buildLocaleBlock(locale, keys, random) {
  const lines = keys.map((key) => {
    const value = key === TARGET_KEY && locale === TARGET_LOCALE ? OLD_VALUE : buildValue(locale, random);
    return `    ${JSON.stringify(key)}: ${JSON.stringify(value)}`;
  });
  return `  "${locale}": {\n${lines.join(",\n")}\n  }`;
}

/**
 * Builds the whole localization file text, raw UTF-8 with no escapes.
 * @param seed - integer seed
 */
export function buildLocales(seed) {
  const random = mulberry32(seed);
  const keys = buildKeys();
  const blocks = LOCALES.map((locale) => buildLocaleBlock(locale, keys, random));
  return `{\n${blocks.join(",\n")}\n}\n`;
}

/**
 * Returns the byte range [start, end) of the target value literal in the file bytes.
 * @param text - the original file text
 */
export function targetValueByteRange(text) {
  const blockStart = text.indexOf(`  "${TARGET_LOCALE}": {`);
  const lineStart = text.indexOf(`    "${TARGET_KEY}": `, blockStart);
  const valueStart = lineStart + `    "${TARGET_KEY}": `.length;
  const valueEnd = text.indexOf("\n", valueStart) - 1;
  return [Buffer.byteLength(text.slice(0, valueStart)), Buffer.byteLength(text.slice(0, valueEnd))];
}
