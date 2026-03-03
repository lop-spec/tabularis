'use client';

import { useState, useEffect } from 'react';
import Link from 'next/link';

type CookiePrefs = {
  necessary: true;
  measurement: boolean;
  marketing: boolean;
};

const STORAGE_KEY = 'tabularis-cookie-consent';

/**
 * Initialises Matomo with two-level privacy:
 * - Always: cookieless tracking with anonymised IP (no consent required)
 * - If cookieConsent=true: enables persistent cookies via setCookieConsentGiven()
 *
 * If the script is already loaded, only the consent state is updated.
 */
function initMatomo(cookieConsent: boolean) {
  if (typeof window === 'undefined') return;

  const _paq: unknown[][] = ((window as any)._paq =
    (window as any)._paq || []);

  if ((window as any).__matomoLoaded) {
    // Script already running — update consent state only
    if (cookieConsent) {
      _paq.push(['setCookieConsentGiven']);
    } else {
      _paq.push(['forgetCookieConsentGiven']);
    }
    return;
  }

  (window as any).__matomoLoaded = true;

  // Require cookie consent by default → cookieless until granted
  _paq.push(['requireCookieConsent']);
  // Grant immediately if consent already stored
  if (cookieConsent) {
    _paq.push(['setCookieConsentGiven']);
  }

  _paq.push(['trackPageView']);
  _paq.push(['enableLinkTracking']);

  const u = '//analytics.debbaweb.it/';
  _paq.push(['setTrackerUrl', u + 'matomo.php']);
  _paq.push(['setSiteId', '4']);

  const d = document;
  const g = d.createElement('script');
  const s = d.getElementsByTagName('script')[0];
  g.async = true;
  g.src = u + 'matomo.js';
  s.parentNode!.insertBefore(g, s);
}

export function CookieConsent() {
  const [visible, setVisible] = useState(false);
  const [measurement, setMeasurement] = useState(false);
  const [marketing, setMarketing] = useState(false);

  useEffect(() => {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      // No stored prefs — start cookieless tracking, show banner
      initMatomo(false);
      setVisible(true);
      return;
    }
    try {
      const prefs: CookiePrefs = JSON.parse(raw);
      setMeasurement(prefs.measurement);
      setMarketing(prefs.marketing);
      initMatomo(prefs.measurement);
    } catch {
      initMatomo(false);
      setVisible(true);
    }
  }, []);

  function save(measurementVal: boolean, marketingVal: boolean) {
    const prefs: CookiePrefs = {
      necessary: true,
      measurement: measurementVal,
      marketing: marketingVal,
    };
    localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
    setMeasurement(measurementVal);
    setMarketing(marketingVal);
    setVisible(false);
    initMatomo(measurementVal);
  }

  if (!visible) return null;

  return (
    <div className="cookie-banner" role="dialog" aria-label="Cookie preferences">
      <p className="cookie-desc">
        Tabularis uses cookies to improve your experience and for analytics.{' '}
        <Link href="/cookie-policy">Read our cookie policy</Link> for more
        details.
      </p>

      <div className="cookie-rows">
        <div className="cookie-row">
          <span className="cookie-label">NECESSARY</span>
          <div
            className="cookie-toggle cookie-toggle--on cookie-toggle--locked"
            title="Necessary cookies are always active"
          >
            <span className="cookie-toggle-thumb" />
          </div>
        </div>

        <div className="cookie-row">
          <span className="cookie-label">MEASUREMENT</span>
          <button
            className={`cookie-toggle ${measurement ? 'cookie-toggle--on' : 'cookie-toggle--off'}`}
            onClick={() => setMeasurement((v) => !v)}
            aria-pressed={measurement}
            aria-label="Toggle measurement cookies"
          >
            <span className="cookie-toggle-thumb" />
          </button>
        </div>

        <div className="cookie-row">
          <span className="cookie-label">MARKETING</span>
          <button
            className={`cookie-toggle ${marketing ? 'cookie-toggle--on' : 'cookie-toggle--off'}`}
            onClick={() => setMarketing((v) => !v)}
            aria-pressed={marketing}
            aria-label="Toggle marketing cookies"
          >
            <span className="cookie-toggle-thumb" />
          </button>
        </div>
      </div>

      <div className="cookie-actions">
        <button
          className="cookie-btn cookie-btn--save"
          onClick={() => save(measurement, marketing)}
        >
          Save
        </button>
        <button
          className="cookie-btn cookie-btn--reject"
          onClick={() => save(false, false)}
        >
          Reject All
        </button>
        <button
          className="cookie-btn cookie-btn--accept"
          onClick={() => save(true, true)}
        >
          Accept All
        </button>
      </div>
    </div>
  );
}
