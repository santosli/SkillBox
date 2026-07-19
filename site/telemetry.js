(() => {
  const CONSENT_KEY = "skillbox.analytics.consent.v1";
  const DEVICE_KEY = "vibeloft.telemetry.device.v1";
  const ALLOWED = "granted";
  const DECLINED = "declined";
  const SCRIPT_ID = "vibeloft-telemetry";
  const SCRIPT_URL = "https://vibeloft.ai/telemetry/v1.js";
  const PRODUCT_ID = "2b756c64-1f61-4945-a187-d86bf25e56aa";
  const AUTH_KEY = "vl_web.XAL7WeL2WGVevtX9ODb1QqtJIHydeMfvGTx3tFl4l7E";

  const consentPanel = document.querySelector("[data-analytics-consent]");
  const status = document.querySelector("[data-analytics-status]");
  const allowButton = document.querySelector("[data-analytics-allow]");
  const declineButton = document.querySelector("[data-analytics-decline]");

  function privacySignalEnabled() {
    const doNotTrack = String(navigator.doNotTrack ?? navigator.msDoNotTrack ?? "").toLowerCase();
    return navigator.globalPrivacyControl === true || doNotTrack === "1" || doNotTrack === "yes";
  }

  function readConsent() {
    try {
      return localStorage.getItem(CONSENT_KEY);
    } catch {
      return null;
    }
  }

  function writeConsent(value) {
    try {
      localStorage.setItem(CONSENT_KEY, value);
    } catch {
      // The VibeLoft client also disables itself when first-party storage is unavailable.
    }
  }

  function clearDeviceIdentity() {
    try {
      localStorage.removeItem(DEVICE_KEY);
    } catch {
      // There is no persisted identity to remove when storage is unavailable.
    }
  }

  function stopAnalytics() {
    const client = globalThis.VibeLoftTelemetry?.client;
    if (client?.stop) void client.stop({ flush: false });
    document.getElementById(SCRIPT_ID)?.remove();
    clearDeviceIdentity();
  }

  function startClient() {
    const telemetry = globalThis.VibeLoftTelemetry;
    if (!telemetry?.createClient) return;
    if (telemetry.client && !telemetry.client.stopped) return;
    telemetry.client = telemetry.createClient({ productId: PRODUCT_ID, authKey: AUTH_KEY }).start();
  }

  function startAnalytics() {
    if (privacySignalEnabled()) return;

    if (globalThis.VibeLoftTelemetry?.createClient) {
      startClient();
      return;
    }

    if (document.getElementById(SCRIPT_ID)) return;
    const script = document.createElement("script");
    script.id = SCRIPT_ID;
    script.src = SCRIPT_URL;
    script.defer = true;
    script.referrerPolicy = "no-referrer";
    script.addEventListener("load", () => {
      if (readConsent() === ALLOWED && !privacySignalEnabled()) startClient();
    });
    document.head.append(script);
  }

  function hideConsent() {
    if (consentPanel) consentPanel.hidden = true;
  }

  function showConsent({ focus = false } = {}) {
    if (!consentPanel || !status || !allowButton) return;

    const privacySignal = privacySignalEnabled();
    const consent = readConsent();
    allowButton.disabled = privacySignal;
    if (privacySignal) {
      status.textContent = "Analytics is disabled because your browser sends a privacy signal.";
    } else if (consent === ALLOWED) {
      status.textContent = "Analytics is currently allowed. Choose Decline to withdraw consent and delete the local device ID.";
    } else if (consent === DECLINED) {
      status.textContent = "Analytics is currently declined. No VibeLoft script will load unless you allow it.";
    } else {
      status.textContent = "With your permission, this website loads VibeLoft telemetry to count page views. It does not access the SkillBox app or local files.";
    }
    consentPanel.hidden = false;
    if (focus) queueMicrotask(() => (privacySignal ? declineButton : allowButton)?.focus());
  }

  allowButton?.addEventListener("click", () => {
    if (privacySignalEnabled()) {
      showConsent();
      return;
    }
    writeConsent(ALLOWED);
    startAnalytics();
    hideConsent();
  });

  declineButton?.addEventListener("click", () => {
    writeConsent(DECLINED);
    stopAnalytics();
    hideConsent();
  });

  for (const button of document.querySelectorAll("[data-manage-analytics]")) {
    button.addEventListener("click", () => showConsent({ focus: true }));
  }

  if (privacySignalEnabled()) {
    stopAnalytics();
  } else if (readConsent() === ALLOWED) {
    startAnalytics();
  } else if (readConsent() !== DECLINED) {
    showConsent();
  }
})();
