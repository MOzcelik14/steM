(() => {
  "use strict";

  const REPO = "MOzcelik14/steM";
  const RELEASE_API = `https://api.github.com/repos/${REPO}/releases/latest`;
  const FALLBACK_VERSION = "v0.5.0";
  const CACHE_KEY = "stem_gh_release_v1";
  const CACHE_TTL_MS = 15 * 60 * 1000;
  const LANG_KEY = "stem_lang";

  const TRANSLATIONS = {
    tr: {
      pageTitle: "steM. — Tahribatsız Çok Kanallı Ses Stüdyosu",
      "nav.features": "Özellikler",
      "nav.workflow": "İş Akışı",
      "nav.formats": "Formatlar",
      "nav.download": "İndir",
      "nav.github": "GitHub",
      "hero.badge": "Linux ve Windows · GPL-3.0",
      "hero.latest": "Güncel Sürüm:",
      "hero.title": "Orijinal kayıtlarınızı bozmadan düzenleyin, miksleyin ve yayınlayın.",
      "hero.subtitle":
        "steM., podcaster'lar, ses tasarımcıları ve müzisyenler için geliştirilmiş hafif ve yüksek performanslı bir masaüstü ses iş istasyonudur. Gerçek zamanlı efekt zincirleri, çok kanallı kayıt ve yayın standardı analiz tek pencerede.",
      "hero.ctaPrimary": "Ücretsiz İndir",
      "hero.ctaSecondary": "Özellikleri İncele",
      "hero.zoom": "Tam Boyut",
      "spec.engineLabel": "Ses Motoru",
      "spec.engineValue": "Sıfır tahsisatlı gerçek zamanlı işleme",
      "spec.loudnessLabel": "Yayın Standardı",
      "spec.loudnessValue": "EBU R128 / ITU-R BS.1770",
      "spec.licenseLabel": "Lisans",
      "spec.licenseValue": "Açık Kaynak (GPL-3.0)",
      "features.eyebrow": "Temel Yetenekler",
      "features.title": "Stüdyo düzeyinde ses işleme, karmaşıklık olmadan.",
      "features.subtitle":
        "Günlük kurgu işlerinden çok kanallı podcast ve müzik prodüksiyonuna kadar ihtiyacınız olan araçlar doğrudan erişilebilir durumdadır.",
      "f1.title": "Tahribatsız Çok Kanallı Kurgu",
      "f1.desc":
        "Klipleri sürükleyip bırakın, kesin, çoğaltın ve klip içi ses kazancı çizgisiyle seviye dengeleyin. Çakışan kliplerde otomatik eşit güçlü crossfade uygulanır; kaynak dosyalarınız asla değişmez.",
      "f2.title": "Gerçek Zamanlı Efekt Rafı",
      "f2.desc":
        "Kanal ve ana çıkış için sıralanabilir EQ, Compressor, Limiter, Noise Gate, De-Esser, Delay ve Reverb modülleri. Podcast Vokal, Sıcak Radyo ve Temiz Konuşma hazır ayarlarıyla anında sonuç alın.",
      "f3.title": "Kayıt, Metronom ve Canlı VU Metreler",
      "f3.desc":
        "Giriş yönlendirme, geri sayım (count-in), punch-in/punch-out kayıt, duyulabilir BPM metronomu ve her kanal için gecikmesiz canlı Peak/RMS seviye göstergeleri.",
      "f4.title": "Ses Temizleme ve Restorasyon",
      "f4.desc":
        "Spektral gürültü azaltma ile dip gürültüsü ve uğultuyu temizleyin, tık ve patlama giderici ile eski veya kirli kayıtları netleştirin.",
      "f5.title": "Görsel ve Yayın Analizi",
      "f5.desc":
        "Çok çözünürlüklü dalga formu, 2B şelale spektrogram, frekans analizörü ve Spotify (-14 LUFS), Podcast (-16 LUFS) ile Yayın (-23 LUFS) hedeflerine uygun EBU R128 normalizasyonu.",
      "f6.title": "Şablonlar ve Toplu Dışa Aktarım",
      "f6.desc":
        "2 Kişilik Podcast veya Vokal + Altyapı şablonlarıyla saniyeler içinde başlayın. Bölüm işaretçilerine göre tüm bölümleri tek seferde ayrı dosyalar halinde dışa aktarın.",
      "workflow.eyebrow": "Hızlı İş Akışı",
      "workflow.title": "Fikirden yayına üç adımda.",
      "w1.title": "İçe Aktarın veya Kaydedin",
      "w1.desc":
        "Ses dosyalarını dosya yöneticisinden doğrudan zaman çizelgesine sürükleyin ya da hazır proje şablonlarından biriyle çok kanallı kayda başlayın.",
      "w2.title": "Kesin, Temizleyin ve Miksleyin",
      "w2.desc":
        "Zaman aralığı seçimiyle istenmeyen bölümleri temizleyin, ızgaraya yapıştırma (Snap) ile hizalayın ve gerçek zamanlı efekt rafıyla sesi dengeleyin.",
      "w3.title": "Platform Standardında Dışa Aktarın",
      "w3.desc":
        "WAV, FLAC, MP3 veya OGG Vorbis formatlarından birini seçin, hedef LUFS değerini belirleyin ve projenizi ya da bölüm işaretçilerinizi dışa aktarın.",
      "formats.eyebrow": "Uyumluluk ve Mimari",
      "formats.title": "Yaygın tüm ses formatlarıyla çalışır.",
      "formats.desc":
        "Ek kodek paketi kurmadan yüksek çözünürlüklü kayıtları açın, düzenleyin ve hedef platforma uygun formatta teslim edin.",
      "formats.importTitle": "İçe Aktarım",
      "formats.exportTitle": "Dışa Aktarım",
      "formats.platformTitle": "İşletim Sistemleri",
      "download.eyebrow": "İndirme Merkezi",
      "download.subtitle":
        "Sürüm bilgileri ve paket bağlantıları GitHub Releases ile otomatik olarak senkronize edilir.",
      "download.syncLabel": "GitHub Sürümü",
      "download.linuxMeta": "Taşınabilir arşiv (.tar.gz)",
      "download.windowsMeta": "Taşınabilir arşiv (.zip)",
      "download.btn": "İndir",
      "download.notes": "Sürüm Notları ve SHA256 Doğrulama",
      "download.source": "Kaynak Koddan Derle",
      "footer.license": "GPL-3.0-or-later lisansı ile açık kaynak olarak dağıtılmaktadır.",
      "footer.releases": "Sürümler",
      "footer.issues": "Hata Bildir",
      "lightbox.caption": "steM. — Çok Kanallı Zaman Çizelgesi ve Mikser",
      "lightbox.close": "Kapat",
    },
    en: {
      pageTitle: "steM. — Non-Destructive Multi-Track Audio Workstation",
      "nav.features": "Features",
      "nav.workflow": "Workflow",
      "nav.formats": "Formats",
      "nav.download": "Download",
      "nav.github": "GitHub",
      "hero.badge": "Linux & Windows · GPL-3.0",
      "hero.latest": "Latest Release:",
      "hero.title": "Edit, mix, and publish without altering your original recordings.",
      "hero.subtitle":
        "steM. is a lightweight, high-performance desktop audio workstation built for podcasters, sound designers, and musicians. Real-time effect chains, multi-track recording, and broadcast loudness analysis in one window.",
      "hero.ctaPrimary": "Download Free",
      "hero.ctaSecondary": "Explore Features",
      "hero.zoom": "Full Size",
      "spec.engineLabel": "Audio Engine",
      "spec.engineValue": "Zero-allocation real-time processing",
      "spec.loudnessLabel": "Broadcast Standard",
      "spec.loudnessValue": "EBU R128 / ITU-R BS.1770",
      "spec.licenseLabel": "License",
      "spec.licenseValue": "Open Source (GPL-3.0)",
      "features.eyebrow": "Core Capabilities",
      "features.title": "Studio-grade audio processing, without the clutter.",
      "features.subtitle":
        "From everyday voice editing to multi-track podcast and music production, every essential tool stays directly within reach.",
      "f1.title": "Non-Destructive Multi-Track Editing",
      "f1.desc":
        "Drag and drop clips, split, duplicate, and balance levels with on-clip gain lines. Overlapping clips apply automatic equal-power crossfades while keeping your source files untouched.",
      "f2.title": "Real-Time Effects Rack",
      "f2.desc":
        "Reorderable EQ, Compressor, Limiter, Noise Gate, De-Esser, Delay, and Reverb per track or master bus. Start faster with Podcast Vocal, Warm Radio, and Clean Speech presets.",
      "f3.title": "Recording, Metronome & Live Meters",
      "f3.desc":
        "Input channel routing, count-in timer, punch-in/punch-out recording, audible BPM metronome, and lock-free live Peak/RMS meters on every track.",
      "f4.title": "Audio Cleanup & Restoration",
      "f4.desc":
        "Remove background hum and hiss with spectral noise reduction, and restore transient clarity with the click and pop remover.",
      "f5.title": "Visual & Loudness Analysis",
      "f5.desc":
        "Multi-resolution waveforms, 2D waterfall spectrogram, frequency analyzer, and EBU R128 normalization targeting Spotify (-14 LUFS), Podcasts (-16 LUFS), and Broadcast (-23 LUFS).",
      "f6.title": "Templates & Batch Chapter Export",
      "f6.desc":
        "Launch in seconds with 2-Person Podcast or Vocal + Backing Track templates. Export marker-delimited chapters into separate numbered files in one pass.",
      "workflow.eyebrow": "Streamlined Workflow",
      "workflow.title": "From raw recording to release in three steps.",
      "w1.title": "Import or Record",
      "w1.desc":
        "Drop audio files straight from your file manager onto any track, or open a project template and start multi-track recording immediately.",
      "w2.title": "Cut, Clean & Mix",
      "w2.desc":
        "Select time ranges to trim or silence unwanted sections, align clips with Snap to Grid, and shape your sound in the real-time effects rack.",
      "w3.title": "Export to Platform Standards",
      "w3.desc":
        "Choose WAV, FLAC, MP3, or OGG Vorbis, pick your target LUFS loudness level, and export your full mix or chapter segments.",
      "formats.eyebrow": "Compatibility & Architecture",
      "formats.title": "Works with all common audio formats.",
      "formats.desc":
        "Open high-resolution recordings without extra codec packs and deliver in the exact format your audience expects.",
      "formats.importTitle": "Import",
      "formats.exportTitle": "Export",
      "formats.platformTitle": "Operating Systems",
      "download.eyebrow": "Download Center",
      "download.subtitle":
        "Version numbers and package links are automatically synchronized with GitHub Releases.",
      "download.syncLabel": "GitHub Release",
      "download.linuxMeta": "Portable archive (.tar.gz)",
      "download.windowsMeta": "Portable archive (.zip)",
      "download.btn": "Download",
      "download.notes": "Release Notes & SHA256 Checksums",
      "download.source": "Build from Source",
      "footer.license": "Distributed as open-source software under GPL-3.0-or-later.",
      "footer.releases": "Releases",
      "footer.issues": "Report an Issue",
      "lightbox.caption": "steM. — Multi-Track Timeline & Mixer",
      "lightbox.close": "Close",
    },
  };

  let currentLang = detectInitialLang();
  let cachedRelease = null;

  function detectInitialLang() {
    try {
      const saved = localStorage.getItem(LANG_KEY);
      if (saved === "tr" || saved === "en") return saved;
    } catch (_) {}
    return navigator.language && navigator.language.toLowerCase().startsWith("tr") ? "tr" : "en";
  }

  function applyTranslations(lang) {
    const dict = TRANSLATIONS[lang] || TRANSLATIONS.en;
    currentLang = lang;
    document.documentElement.lang = lang;
    document.title = dict.pageTitle;

    document.querySelectorAll("[data-i18n]").forEach((el) => {
      const key = el.getAttribute("data-i18n");
      if (dict[key]) {
        el.textContent = dict[key];
      }
    });

    document.querySelectorAll(".lang-btn").forEach((btn) => {
      const active = btn.getAttribute("data-lang") === lang;
      btn.setAttribute("aria-pressed", active ? "true" : "false");
    });

    if (cachedRelease) {
      renderRelease(cachedRelease);
    }
  }

  function formatBytes(bytes) {
    if (!bytes || bytes <= 0) return "";
    const mb = bytes / (1024 * 1024);
    return `${mb.toFixed(1)} MB`;
  }

  function formatDate(isoString, lang) {
    if (!isoString) return "";
    const d = new Date(isoString);
    if (Number.isNaN(d.getTime())) return "";
    return new Intl.DateTimeFormat(lang === "tr" ? "tr-TR" : "en-US", {
      year: "numeric",
      month: "short",
      day: "numeric",
    }).format(d);
  }

  function renderRelease(release) {
    const version = release.tag_name || FALLBACK_VERSION;
    const dateLabel = formatDate(release.published_at, currentLang);
    const dict = TRANSLATIONS[currentLang] || TRANSLATIONS.en;

    document.querySelectorAll("[data-gh-version]").forEach((el) => {
      el.textContent = version;
    });

    document.querySelectorAll("[data-gh-date]").forEach((el) => {
      el.textContent = dateLabel ? `(${dateLabel})` : "";
    });

    const releaseUrl = release.html_url || `https://github.com/${REPO}/releases/latest`;
    const notesLink = document.getElementById("dl-release-notes");
    if (notesLink) notesLink.href = releaseUrl;

    const assets = Array.isArray(release.assets) ? release.assets : [];
    const linuxAsset = assets.find((a) => /linux.*\.tar\.gz$/i.test(a.name));
    const winAsset = assets.find((a) => /windows.*\.zip$/i.test(a.name));

    const linuxLink = document.getElementById("dl-linux");
    const linuxMeta = document.getElementById("dl-linux-meta");
    if (linuxLink) linuxLink.href = linuxAsset ? linuxAsset.browser_download_url : releaseUrl;
    if (linuxMeta) {
      const size = linuxAsset ? formatBytes(linuxAsset.size) : "";
      linuxMeta.textContent = size ? `${linuxAsset.name} · ${size}` : dict["download.linuxMeta"];
    }

    const winLink = document.getElementById("dl-windows");
    const winMeta = document.getElementById("dl-windows-meta");
    if (winLink) winLink.href = winAsset ? winAsset.browser_download_url : releaseUrl;
    if (winMeta) {
      const size = winAsset ? formatBytes(winAsset.size) : "";
      winMeta.textContent = size ? `${winAsset.name} · ${size}` : dict["download.windowsMeta"];
    }
  }

  async function syncGitHubRelease() {
    try {
      const raw = sessionStorage.getItem(CACHE_KEY);
      if (raw) {
        const parsed = JSON.parse(raw);
        if (parsed && Date.now() - parsed.ts < CACHE_TTL_MS && parsed.data) {
          cachedRelease = parsed.data;
          renderRelease(cachedRelease);
          return;
        }
      }
    } catch (_) {}

    try {
      const res = await fetch(RELEASE_API, {
        headers: { Accept: "application/vnd.github+json" },
      });
      if (!res.ok) return;
      const data = await res.json();
      if (!data || !data.tag_name) return;

      cachedRelease = {
        tag_name: data.tag_name,
        published_at: data.published_at,
        html_url: data.html_url,
        assets: Array.isArray(data.assets)
          ? data.assets.map((a) => ({
              name: a.name,
              size: a.size,
              browser_download_url: a.browser_download_url,
            }))
          : [],
      };

      try {
        sessionStorage.setItem(CACHE_KEY, JSON.stringify({ ts: Date.now(), data: cachedRelease }));
      } catch (_) {}

      renderRelease(cachedRelease);
    } catch (_) {
      // Keep fallback version & release link when offline
    }
  }

  function setupLightbox() {
    const dialog = document.getElementById("screenshot-dialog");
    const openBtn = document.getElementById("open-lightbox");
    const openImgBtn = document.getElementById("open-lightbox-img");
    const closeBtn = document.getElementById("close-lightbox");
    if (!dialog) return;

    const open = () => {
      if (typeof dialog.showModal === "function") {
        dialog.showModal();
      }
    };

    if (openBtn) openBtn.addEventListener("click", open);
    if (openImgBtn) openImgBtn.addEventListener("click", open);
    if (closeBtn) closeBtn.addEventListener("click", () => dialog.close());
    dialog.addEventListener("click", (e) => {
      if (e.target === dialog) dialog.close();
    });
  }

  document.querySelectorAll(".lang-btn").forEach((btn) => {
    btn.addEventListener("click", () => {
      const lang = btn.getAttribute("data-lang");
      if (lang === "tr" || lang === "en") {
        try {
          localStorage.setItem(LANG_KEY, lang);
        } catch (_) {}
        applyTranslations(lang);
      }
    });
  });

  applyTranslations(currentLang);
  setupLightbox();
  syncGitHubRelease();
})();
