# steM. Türkçe yerelleştirme dosyası

app-title = steM.
app-tagline = Tahribatsız Ses Düzenleyici ve Karıştırıcı

# Menü Çubuğu - Dosya
menu-file = Dosya
menu-file-new = Yeni Proje
menu-file-open = Proje Aç...
menu-file-save = Projeyi Kaydet
menu-file-save-as = Projeyi Farklı Kaydet...
menu-file-import-audio = Ses İçe Aktar...
menu-file-export = Dışa Aktar...
menu-file-export-selection = Seçimi Dışa Aktar...
menu-file-preferences = Tercihler...
menu-file-exit = Çıkış

# Menü Çubuğu - Düzenle
menu-edit = Düzenle
menu-edit-undo = Geri Al
menu-edit-redo = Yinele
menu-edit-cut = Kes
menu-edit-copy = Kopyala
menu-edit-paste = Yapıştır
menu-edit-delete = Sil
menu-edit-split = Oynatma Başlığından Böl
menu-edit-select-all = Tümünü Seç
menu-edit-deselect = Seçimi Kaldır

# Menü Çubuğu - Görünüm
menu-view = Görünüm
menu-view-zoom-in = Yakınlaştır
menu-view-zoom-out = Uzaklaştır
menu-view-zoom-fit = Ekrana Sığdır
menu-view-reset-zoom = Yakınlaştırmayı Sıfırla
menu-view-toggle-mixer = Karıştırıcı
menu-view-toggle-spectrogram = Spektrogram
menu-view-toggle-meters = Seviye Göstergeleri
menu-view-toggle-history = Geçmiş

# Menü Çubuğu - Parça
menu-track = Parça
menu-track-add-audio = Ses Parçası Ekle
menu-track-duplicate = Parçayı Çoğalt
menu-track-delete = Parçayı Sil
menu-track-mute-all = Tüm Parçaları Sustur
menu-track-unmute-all = Tüm Parçaların Sesini Aç

# Menü Çubuğu - Yardım
menu-help = Yardım
menu-help-documentation = Belgeler
menu-help-about = steM. Hakkında

# Aktarım Denetimleri (Transport)
transport-play = Oynat
transport-pause = Duraklat
transport-stop = Durdur
transport-record = Kayıt
transport-loop = Döngüsel Çalma
transport-timecode = Zaman
transport-samples = Örnek
transport-tempo = Tempo (BPM)
transport-time-signature = Ölçü
transport-master-volume = Ana Ses Düzeyi

# Parça Başlıkları ve Denetimleri
track-header-mute = M
track-header-solo = S
track-header-arm = R
track-header-volume = Kazanç
track-header-pan = Pan
track-header-effects = Efektler
track-header-add-clip = Parça Klibi Ekle
track-default-name = Parça { $number }
track-master-name = Ana Çıkış

# Klip İşlemleri
clip-fade-in = Açma (Fade In)
clip-fade-out = Kısma (Fade Out)
clip-gain = Klip Kazancı
clip-split = Böl
clip-trim = Kırp

# Karıştırıcı Paneli (Mixer)
mixer-title = Karıştırıcı
mixer-track-label = Parça
mixer-master-label = Ana Çıkış
mixer-fader = Potansiyometre
mixer-pan-center = Merkez
mixer-pan-left = Sol %{ $value }
mixer-pan-right = Sağ %{ $value }
mixer-peak = Tepe Değeri (Peak)
mixer-rms = RMS
mixer-lufs = LUFS

# Ses DSP ve Efektler
effect-equalizer = Ekolayzır
effect-compressor = Kompresör
effect-limiter = Sınırlayıcı
effect-reverb = Yankı
effect-delay = Gecikme
effect-noise-reduction = Gürültü Azaltma
effect-normalize = Normalleştir
effect-gain = Kazanç
effect-threshold = Eşik Değeri
effect-ratio = Oran
effect-attack = Saldırı (Attack)
effect-release = Bırakma (Release)
effect-makeup-gain = Telafi Kazancı
effect-frequency = Frekans
effect-q-factor = Q Faktörü
effect-dry-wet = Kuru / Islak

# Görünümler
view-waveform = Dalga Biçimi
view-spectrogram = Spektrogram
view-history-title = Düzenleme Geçmişi
view-no-history = Henüz işlem yapılmadı

# İletişim Pencereleri ve Ayarlar
dialog-about-title = steM. Hakkında
dialog-about-description = steM. yüksek performanslı, tahribatsız bir ses düzenleme ve çok kanallı karıştırma istasyonudur.
dialog-about-license = Lisans: GNU Genel Kamu Lisansı v3.0
dialog-about-built-with = Rust, egui, cpal, symphonia, hound, flacenc, rubato ve realfft ile geliştirilmiştir.
dialog-preferences-title = Tercihler
dialog-preferences-language = Dil
dialog-preferences-audio-device = Çıkış Aygıtı
dialog-preferences-sample-rate = Örnekleme Hızı
dialog-preferences-buffer-size = Arabellek Boyutu
dialog-preferences-latency = Tahmini Gecikme
dialog-close = Kapat
dialog-apply = Uygula
dialog-cancel = İptal

# Dışa Aktarma Penceresi
export-title = Sesi Dışa Aktar
export-format = Biçim
export-format-wav = WAV (Kayıpsız PCM)
export-format-flac = FLAC (Kayıpsız Sıkıştırılmış)
export-format-mp3 = MP3 (MPEG Katman-3)
export-sample-rate = Örnekleme Hızı
export-bit-depth = Bit Derinliği
export-16bit = 16-bit Tamsayı
export-24bit = 24-bit Tamsayı
export-32bit-float = 32-bit Kayan Nokta
export-dither = TPDF Dither
export-dither-enabled = Etkin (Bit derinliği azaltma için önerilir)
export-dither-disabled = Devre Dışı
export-normalize-before = Dışa Aktarmadan Önce Normalleştir
export-target-lufs = Hedef Ses Gürlüğü (LUFS)
export-range = Aralık
export-range-entire = Tüm Proje
export-range-selection = Zaman Seçimi
export-btn-start = Dışa Aktar
export-btn-cancel = Dışa Aktarmayı İptal Et
export-btn-browse = Gözat...
export-status-in-progress = Ses dışa aktarılıyor...
export-status-success = Dışa aktarma başarıyla tamamlandı!
export-status-cancelled = Dışa aktarma kullanıcı tarafından iptal edildi.
export-status-error = Dışa aktarma başarısız oldu: { $reason }

# Durum Çubuğu ve Bildirimler
status-ready = Hazır
status-playing = Oynatılıyor
status-paused = Duraklatıldı
status-recording = Kayıt Yapılıyor
status-loading = Ses yükleniyor...
status-generating-peaks = Dalga biçimi önbelleği oluşturuluyor...
status-project-saved = Proje başarıyla kaydedildi.
status-imported-file = { $filename } içe aktarıldı

# Birimler ve Biçimlendirme
unit-db = dB
unit-hz = Hz
unit-khz = kHz
unit-ms = ms
unit-sec = sn
unit-samples = örnek
unit-bpm = BPM

# Gelişmiş Zaman Kodu ve Izgara
transport-mode-timecode = Zaman (DD:SS.mmm)
transport-mode-smpte = SMPTE (SS:DD:SS:KK)
transport-mode-bars = Ölçü ve Vuruş (Ölçü.Vuruş.Tik)
transport-mode-samples = Örnek (Mutlak)

# Etkileşimli Efektler ve Görselleştiriciler
effect-eq-title = Parametrik Ekolayzır
effect-comp-title = Dinamik Kompresör
effect-band-lowshelf = Alçak Raf (Low Shelf)
effect-band-peaking = Çan (Peaking)
effect-band-highshelf = Yüksek Raf (High Shelf)
effect-band-highpass = Yüksek Geçiren
effect-band-lowpass = Alçak Geçiren
effect-band-bypass = Devre Dışı (Bypass)
track-add-effect = Efekt Ekle...
track-effects-rack = Efekt Rafı

# Demo Oturumu ve Kayıt
menu-file-load-demo = Tanıtım Oturumunu Yükle
status-demo-loaded = Tanıtım çok kanallı oturumu yüklendi!
status-recording-armed = Kayıt için hazırlandı
status-recording-saved = Kayıt yeni klip olarak kaydedildi
recording-input-device = Giriş Aygıtı
recording-armed-track = Kayıt için silahlandırıldı

# Dalga Formu Düzenleyici Modu
view-mode-multitrack = Çok Kanallı
view-mode-waveform = Dalga Formu Düzenleyici
menu-file-open-waveform = Dalga Formu Düzenleyicide Aç...
waveform-title = Dalga Formu Düzenleyici
waveform-no-file = Dalga formu düzenleyicide yüklü dosya yok. Bir ses dosyası açın veya çok kanallı zaman çizelgesindeki bir klibe çift tıklayın.
waveform-selection-start = Başlangıç: { $time }
waveform-selection-end = Bitiş: { $time }
waveform-selection-duration = Süre: { $time }
waveform-snap-zero-crossing = Sıfır Geçişine Hizala
waveform-loop-selection = Seçimi Döngüye Al
waveform-play-selection = Seçimi Oynat
waveform-play-all = Tümünü Oynat
waveform-btn-cut = Kes
waveform-btn-copy = Kopyala
waveform-btn-paste = Yapıştır
waveform-btn-delete = Sil
waveform-btn-trim = Kırp
waveform-btn-silence = + Sessizlik
waveform-btn-reverse = Ters Çevir
waveform-btn-invert = Fazı Ters Çevir
waveform-btn-apply-to-project = Projeye Uygula
waveform-status-cut = Seçim panoya kesildi
waveform-status-copied = Seçim panoya kopyalandı
waveform-status-pasted = Panodan yapıştırıldı
waveform-status-deleted = Seçim silindi
waveform-status-trimmed = Seçime kırpıldı
waveform-status-silence-inserted = 1 saniye sessizlik eklendi
waveform-status-reversed = Ses aralığı ters çevrildi
waveform-status-phase-inverted = Ses fazı ters çevrildi
waveform-status-applied = Dalga formu değişiklikleri proje klibine uygulandı

# Aşama B: Efekt Rafı ve Ses İşlemleri
effect-btn-apply = Seçime Uygula
effect-btn-preview = Canlı Önizleme
effect-btn-bypass = Devre Dışı
effect-btn-reset = Sıfırla
effect-btn-move-up = Yukarı Taşı
effect-btn-move-down = Aşağı Taşı
effect-limiter-title = Tepe Sınırlayıcı (Limiter)
effect-delay-title = Stereo Gecikme (Delay)
effect-reverb-title = Algoritmik Yankı (Reverb)
effect-target-master = Ana Çıkış (Master)
effect-target-track = Kanal { $index }
effect-target-waveform = Dalga Biçimi Önizleme
effect-feedback = Geri Besleme
effect-mix = Kuru/Islak Karışım
effect-delay-time = Gecikme Süresi
effect-room-size = Oda Boyutu
effect-damping = Sönümleme
waveform-btn-fade-in = Giriş Sönümleme (Fade In)
waveform-btn-fade-out = Çıkış Sönümleme (Fade Out)
waveform-btn-normalize = Normalleştir...
waveform-fade-linear = Doğrusal
waveform-fade-log = Logaritmik
waveform-fade-exp = Üstel
waveform-fade-scurve = S-Eğrisi
dialog-normalize-title = Tepe Değeri Normalleştirme
dialog-normalize-target = Hedef Tepe Değeri (dBFS):
dialog-normalize-btn-apply = Uygula
status-effect-applied = Efekt ses seçimine uygulandı
status-fade-applied = Sönümleme eğrisi ses seçimine uygulandı
status-normalized = Ses tepe değeri başarıyla normalleştirildi

# Phase C: Temizleme, Onarım ve Analiz
menu-cleanup = Temizleme
menu-cleanup-capture-noise = Seçimden Gürültü Profilini Yakala
menu-cleanup-noise-reduction = Gürültü Azaltma (Spektral Geçit)...
menu-cleanup-click-remover = Çıtırtı ve Parazit Giderici...
cleanup-noise-reduction = Gürültü Azaltma
cleanup-capture-noise-profile = Gürültü Profilini Yakala
cleanup-open-noise-dialog = Gürültü Azaltma Ayarları...
cleanup-reduction-amount = Azaltma Miktarı (dB)
cleanup-sensitivity = Duyarlılık
cleanup-profile-status = Profil Durumu
cleanup-profile-captured = Yakalandı ({ $bins } bant, { $frames } kare)
cleanup-profile-none = Henüz gürültü profili yakalanmadı
cleanup-btn-capture = Seçimden Yakala
cleanup-btn-apply = Seçime Uygula
cleanup-btn-apply-all = Tüm Dosyaya Uygula
cleanup-status-applied = Gürültü azaltma uygulandı
cleanup-click-remover = Çıtırtı ve Parazit Giderici
cleanup-click-sensitivity = Duyarlılık
cleanup-click-max-width = Maksimum Çıtırtı Süresi (örnek)
cleanup-click-btn-repair = Çıtırtıları Tespit Et ve Onar
cleanup-click-result = { $count } çıtırtı/parazit onarıldı
cleanup-click-none = Seçimde çıtırtı tespit edilmedi

menu-analysis = Analiz
menu-analysis-loudness = Ses Yüksekliği Raporu (EBU R128)...
menu-analysis-spectrogram = Spektrogram Panelini Aç/Kapat
analysis-loudness-report = Ses Yüksekliği Raporu
analysis-loudness-title = EBU R128 Ses Yüksekliği Raporu
analysis-target-standard = Hedef Standart
analysis-integrated-lufs = Bütünleşik Ses Yüksekliği
analysis-momentary-max = Maks. Anlık
analysis-short-term-max = Maks. Kısa Süreli
analysis-loudness-range = Ses Yüksekliği Aralığı (LRA)
analysis-true-peak = Gerçek Tepe Değer (True Peak)
analysis-target-diff = Hedef Farkı
analysis-btn-reanalyze = Sesi Yeniden Analiz Et
analysis-compliant = UYUMLU
analysis-non-compliant = UYUMSUZ

spectrogram-fft-size = FFT Boyutu
spectrogram-palette = Renk Skalası
spectrogram-mode = Görüntüleme Modu
spectrogram-mode-both = Her İkisi (Bölünmüş)
spectrogram-mode-waterfall = Spektrogram (2B)
spectrogram-mode-curve = Spektrum (1B)
meter-peak = Tepe
meter-rms = RMS
