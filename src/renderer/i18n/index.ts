import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import ko from "./locales/ko.json";
import en from "./locales/en.json";
import { SUPPORTED_LANGUAGES, type AppLanguage } from "../types";

const DEFAULT_LANGUAGE: AppLanguage = "ko";

function isSupportedLanguage(language: string): language is AppLanguage {
  return SUPPORTED_LANGUAGES.includes(language as AppLanguage);
}

function resolveInitialLanguage() {
  const browserLanguage = navigator.language.slice(0, 2).toLowerCase();
  if (isSupportedLanguage(browserLanguage)) {
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
