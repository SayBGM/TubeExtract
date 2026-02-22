import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import ko from "./locales/ko.json";
import en from "./locales/en.json";

const DEFAULT_LANGUAGE = "ko";
const SUPPORTED_LANGUAGES = ["ko", "en"] as const;

function resolveInitialLanguage() {
  const browserLanguage = navigator.language.slice(0, 2).toLowerCase();
  if (SUPPORTED_LANGUAGES.includes(browserLanguage as (typeof SUPPORTED_LANGUAGES)[number])) {
    return browserLanguage;
  }
  return DEFAULT_LANGUAGE;
}

i18n.use(initReactI18next).init({
  resources: {
    ko: { translation: ko },
    en: { translation: en },
  },
  lng: resolveInitialLanguage(),
  fallbackLng: DEFAULT_LANGUAGE,
  interpolation: {
    escapeValue: false,
  },
});

export default i18n;
