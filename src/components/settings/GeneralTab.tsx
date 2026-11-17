import { Globe, Cpu, Shield, Layers, Server, ExternalLink } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useSettingsStore } from "../../stores/useSettingsStore";
import { useUpdateStore } from "../../stores/useUpdateStore";
import { SupportedLanguage, SUPPORTED_LANGUAGES } from "../../i18n";
import { CustomSelect } from "../ui/CustomSelect";
import { openExternalUrl } from "../../lib/openUrl";

/** CKL's fork; upstream donation links were removed for the CKL build. */
const REPO_URL = "https://github.com/CKL-Cargo/termimus-ssh";

export function GeneralTab() {
  const { t } = useTranslation();
  const { language, setLanguage } = useSettingsStore();
  const currentVersion = useUpdateStore((s) => s.currentVersion);

  return (
    <div className="space-y-5">
      {/* ── Section 1: Language ────────────────────────────────────────────── */}
      <div className="rounded-2xl border border-[var(--border)] bg-[var(--surface-low)] p-5 shadow-sm">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
          <div className="flex items-start sm:items-center gap-3.5">
            <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl border border-[var(--primary)]/30 bg-[var(--primary)]/10 text-[var(--primary)]">
              <Globe size={22} />
            </div>
            <div>
              <h3 className="text-sm font-semibold text-[var(--text-primary)]">
                {t("settings.general.languageTitle")}
              </h3>
              <p className="text-xs text-[var(--text-muted)] mt-0.5 leading-relaxed">
                {t("settings.general.languageDesc")}
              </p>
            </div>
          </div>

          <div className="w-56 shrink-0">
            <CustomSelect
              value={language}
              onChange={(val) => setLanguage(val as SupportedLanguage)}
              options={SUPPORTED_LANGUAGES.map((lang) => ({
                value: lang.code,
                label: lang.nativeName,
                description: lang.label !== lang.nativeName ? `(${lang.label})` : undefined,
              }))}
            />
          </div>
        </div>
      </div>

      {/* ── Section 2: App Banner & Architecture ───────────────────────────── */}
      <div className="rounded-2xl border border-[var(--border)] bg-[var(--surface-low)] p-6 shadow-sm">
        <div className="flex items-start gap-4">
          <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-[var(--primary)]/15 text-[var(--primary)] shrink-0">
            <Cpu size={26} />
          </div>
          <div className="space-y-1">
            <div className="flex items-center gap-2.5">
              <h3 className="text-base font-semibold text-[var(--text-primary)]">Termimus</h3>
              <span className="rounded bg-[var(--primary)]/15 px-2 py-0.5 text-[10px] font-mono font-semibold text-[var(--primary)] border border-[var(--primary)]/30">
                v{currentVersion}
              </span>
            </div>
            <p className="text-xs text-[var(--text-secondary)]">
              {t("settings.general.appInfoDesc")}
            </p>
          </div>
        </div>

        <div className="mt-5 grid grid-cols-1 sm:grid-cols-3 gap-3 text-xs">
          <div className="rounded-xl border border-[var(--border)] bg-[var(--surface-container)]/40 p-3.5 space-y-1">
            <div className="flex items-center gap-1.5 font-semibold text-[var(--text-primary)]">
              <Shield size={14} className="text-[var(--primary)]" />
              <span>{t("settings.general.zeroKnowledgeTitle")}</span>
            </div>
            <p className="text-[11px] text-[var(--text-muted)]">
              {t("settings.general.zeroKnowledgeDesc")}
            </p>
          </div>

          <div className="rounded-xl border border-[var(--border)] bg-[var(--surface-container)]/40 p-3.5 space-y-1">
            <div className="flex items-center gap-1.5 font-semibold text-[var(--text-primary)]">
              <Layers size={14} className="text-[var(--secondary)]" />
              <span>{t("settings.general.localFirstTitle")}</span>
            </div>
            <p className="text-[11px] text-[var(--text-muted)]">
              {t("settings.general.localFirstDesc")}
            </p>
          </div>

          <div className="rounded-xl border border-[var(--border)] bg-[var(--surface-container)]/40 p-3.5 space-y-1">
            <div className="flex items-center gap-1.5 font-semibold text-[var(--text-primary)]">
              <Server size={14} className="text-[var(--tertiary)]" />
              <span>{t("settings.general.syncTitle")}</span>
            </div>
            <p className="text-[11px] text-[var(--text-muted)]">
              {t("settings.general.syncDesc")}
            </p>
          </div>
        </div>

        <div className="mt-5 flex flex-wrap items-center gap-3 border-t border-[var(--border)] pt-4">
          <button
            onClick={() => openExternalUrl(REPO_URL)}
            className="flex items-center gap-1.5 rounded-lg border border-[var(--border)] bg-[var(--surface-high)] px-3 py-1.5 text-xs text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--surface-highest)] transition-colors cursor-pointer"
          >
            <ExternalLink size={12} />
            {t("settings.general.viewOnGithub")}
          </button>
        </div>
      </div>
    </div>
  );
}
