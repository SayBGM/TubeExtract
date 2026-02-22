import { motion } from "motion/react";
import { useTranslation } from "react-i18next";

export function SetupHeader() {
  const { t } = useTranslation();

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className="text-center mb-12"
    >
      <h1 className="text-4xl font-bold tracking-tight mb-3 bg-linear-to-r from-white to-zinc-400 bg-clip-text text-transparent">
        {t("setup.title")}
      </h1>
      <p className="text-zinc-500 text-lg">{t("setup.subtitle")}</p>
    </motion.div>
  );
}
