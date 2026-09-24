# steM. — Yapay Zekâ Destekli Ses Ayrıştırma Stüdyosu

<p align="center">
  <img src="data/icons/hicolor/scalable/apps/com.mozcelik.stem.svg" width="128" height="128" alt="steM. İkonu"/>
</p>

<p align="center">
  <strong>Yapay Zekâ Destekli Ses Ayrıştırma Stüdyosu</strong><br>
  <em>Geliştirici: <strong>M. Özçelik</strong></em>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/Lisans-MIT-purple.svg" alt="Lisans: MIT"></a>
  <img src="https://img.shields.io/badge/Platform-Linux%20Mint%2022.3%20%7C%20Ubuntu%2024.04-informational.svg" alt="Platform">
  <img src="https://img.shields.io/badge/Windows-10%20%7C%2011%20Ready-0078d7.svg?logo=windows" alt="Windows Ready">
  <img src="https://img.shields.io/badge/GTK-4.0%20%2B%20Libadwaita-blueviolet.svg" alt="GTK4">
  <img src="https://img.shields.io/badge/Yapay%20Zek%C3%A2-Demucs%20v4-ff007f.svg" alt="Demucs">
  <img src="https://img.shields.io/badge/GPU%20H%C4%B1zland%C4%B1rma-NVIDIA%20CUDA-76b900.svg" alt="CUDA">
</p>

<p align="center">
  <strong>Türkçe</strong> • <a href="README.md">🇬🇧 <strong>English Documentation</strong></a> • <a href="docs/WINDOWS.tr.md">🪟 <strong>Windows Kılavuzu</strong></a>
</p>

> [!NOTE]
> 🇬🇧 **English Documentation**: For instructions, guides, and full documentation in English, please refer to [**README.md**](README.md). Windows users can refer to [**docs/WINDOWS.md**](docs/WINDOWS.md) (or [**docs/WINDOWS.tr.md**](docs/WINDOWS.tr.md)).

---

## Genel Bakış

**steM.**, müzisyenler, prodüktörler, DJ'ler ve ses mühendisleri için geliştirilmiş modern, yerel bir Linux masaüstü ses iş istasyonu uygulamasıdır. Meta'nın en gelişmiş derin öğrenme modeli olan **Demucs v4** motorunu kullanan steM., müzik parçalarını kristal netliğinde bağımsız kanallara (vokal, davul, bas, enstrümantal ve diğer) ayrıştırır.

Basit bir arayüz sarmalayıcısının ötesinde olan steM.; DAW standartlarında çok kanallı senkronize ses oynatıcı, bağımsız kanal fader'ları, solo/mute matrisi, Cairo tabanlı interaktif dalga formu görüntüleyici ve 4 GB VRAM'e sahip NVIDIA RTX 3050 gibi modern dizüstü GPU'larına özel bellek koruma optimizasyonları sunar.

---

## Öne Çıkan Özellikler

### 🎧 Ses İçe Aktarma ve Çoklu İşlem Kuyruğu
- **Desteklenen Formatlar**: MP3, WAV, FLAC, OGG, M4A, AAC ve diğer FFmpeg uyumlu formatlar.
- **Sürükle ve Bırak**: Dosya yöneticisinden (Nemo, Nautilus) tekli veya çoklu dosyaları doğrudan pencereye bırakabilme.
- **Ön Doğrulama**: Parçanın örnekleme frekansı (sample rate), kanal sayısı, süresi ve dosya bütünlüğünün otomatik incelenmesi.
- **Toplu İşlem Kuyruğu**: Birden fazla parçayı sıraya ekleyip arayüzü dondurmadan arka planda sırayla ayrıştırabilme.

### 🧠 Demucs v4 ile Yapay Zekâ Ayrıştırması
- **4 Kanallı Ayrıştırma**: Vokal, davul, bas ve diğer sesler.
- **2 Kanallı Ayrıştırma**: Hızlı vokal izolasyonu ve enstrümantal (altyapı) çıkarma.
- **Model Seçenekleri**:
  - `htdemucs`: Standart Hibrit Transformatör (Hız ve stüdyo kalitesi dengesi).
  - `htdemucs_ft`: İnce ayarlanmış (fine-tuned) yüksek doğruluklu stüdyo modeli.
  - `htdemucs_6s`: Piyano ve gitarı da ayıran 6 kanallı model.
  - `mdx_extra`: MDX-Net mimarisi tabanlı alternatif model.
- **Hassasiyet Ayarları**: Shifts (rastgele kaydırma), örtüşme (overlap) ve segment boyutu konfigürasyonu.
- **Anlık İlerleme Takibi**: Demucs alt sürecinden gerçek zamanlı yüzde ilerleme verisi.
- **İptal Desteği**: Devam eden ayrıştırmayı sistemi ve dosyaları bozmadan anında durdurabilme.

### ⚡ Donanım Optimizasyonu (RTX 3050 4 GB VRAM)
- **Otomatik CUDA Tespiti**: Sistemdeki NVIDIA GPU, sürücü ve VRAM kapasitesini anında tanıma.
- **Düşük VRAM Güvenlik Modu**: 4 GB VRAM limitini korumak için 6 saniyelik segment parçalama ve tekli işçi (`-j 1`) parametreleri.
- **CUDA OOM Kurtarma**: GPU belleği yetersiz kaldığında hatayı yakalayarak kullanıcıya tek tıkla CPU üzerinden devam etme seçeneği sunma.
- **CPU Geri Dönüşü (Fallback)**: Uyumlu GPU bulunmadığında çok çekirdekli CPU ile kesintisiz çalışma.

### 🎚️ Çok Kanallı Mikser ve Oynatıcı Stüdyo
- **Faz Kilitli Senkronize Oynatma**: Ayrıştırılan tüm kanalları birbiriyle sıfır gecikmeyle ve faz uyumuyla eşzamanlı çalabilen GStreamer motoru.
- **Kanal Şeritleri**: Ana Parça, Vokal, Davul, Bas ve Diğer kanalları için renk kodlu DAW mikser şeritleri.
- **Fader & Seviyeler**: %0 ile %150 arası bağımsız dikey ses fader'ları.
- **Profesyonel Solo & Mute Matrisi**:
  - Bağımsız **Mute (M)** butonları.
  - Dinamik **Solo (S)** mantığı: Bir veya daha fazla kanal solo yapıldığında diğer kanallar otomatik olarak susturulur.
- **Etkileşimli Dalga Formu (Waveform)**:
  - Cairo ile çizilen pürüzsüz ses dalgası görseli.
  - Tıklanabilir ve sürüklenebilir oynatma çizgisi (playhead) ile anında istenen saniyeye atlama.
  - Dijital zaman sayacı (`00:00 / 03:45`) ve döngü (loop) butonu.

### 💾 Dışa Aktarma (Export)
- **Formatlar**: 24-bit PCM WAV (kayıpsız) ve 320 kbps yüksek kaliteli MP3.
- **Hedef Klasör**: Ayarlanabilir dışa aktarma konumu (varsayılan: `~/Müzik/steM_Stems`).
- **Düzenli Klasörleme**: Her parça için `{Hedef}/{Şarkı Adı}/{Kanal}.{uzantı}` yapısında otomatik klasörleme.
- **Üzerine Yazma Koruması**: Aynı isimdeki oturumların üzerine yanlışlıkla yazılmasını önleyen otomatik numaralandırma.

---

## Kurulum

### Otomatik Kurulum (Önerilen)

Projeyi klonlayıp kurulum betiğini çalıştırın:

```bash
git clone https://github.com/MOzcelik14/steM.git
cd steM
./install.sh
```

Bu betik:
1. Sistemdeki GTK4 ve Libadwaita paketlerini kullanan izole bir Python sanal ortamı (`.venv`) oluşturur.
2. CUDA 12.4 destekli PyTorch ve Demucs v4 kütüphanelerini yükler.
3. Uygulama ikonunu ve `.desktop` başlatıcısını Linux Mint menünüzün **Ses ve Video** kategorisine ekler.
4. Terminalden doğrudan erişim için `~/.local/bin/stem` komutunu tanımlar.

---

### 🪟 Windows 10 / 11 Kurulumu

steM. Windows üzerinde tam uyumlulukla çalışabilir:

1. **Hazır Paketler**: [GitHub Releases](https://github.com/MOzcelik14/steM/releases) sayfasından GitHub Actions tarafından derlenen `steM-Windows-x64.zip` arşivini indirip doğrudan çalıştırabilirsiniz.
2. **Geliştirici Kurulumu**: MSYS2 UCRT64 terminali ile kaynak koddan yerel olarak çalıştırabilirsiniz.
3. Ayrıntılı kurulum, derleme adımları ve betikler için [**Windows Kılavuzuna (docs/WINDOWS.tr.md)**](docs/WINDOWS.tr.md) göz atın.

---

## Uygulamayı Başlatma

- **Menüden**: Linux Mint / Cinnamon menüsünü açın, **Ses ve Video** altındaki **steM.** uygulamasını tıklayın.
- **Terminalden**:
  ```bash
  stem
  ```
- **Proje Dizininden**:
  ```bash
  ./run.sh
  ```

---

## Otomatik Testler

Uygulamanın test paketini çalıştırmak için:

```bash
python3 -m unittest discover -s tests -p "test_*.py" -v
```

---

## Lisans

Bu proje **MIT Lisansı** ile lisanslanmış açık kaynaklı bir yazılımdır. Detaylar için [LICENSE](LICENSE) dosyasına bakabilirsiniz.

Telif Hakkı © 2026 **M. Özçelik**. Tüm hakları saklıdır.
