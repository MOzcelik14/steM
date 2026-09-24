# steM. Windows Kurulum ve Derleme Kılavuzu

<p align="center">
  <strong>Separate the sound. Keep the soul.</strong><br>
  <em>Geliştirici: <strong>M. Özçelik</strong></em>
</p>

steM. çapraz platform (cross-platform) mimarisi düşünülerek geliştirilmiştir. Linux tabanlı (GTK4 ve Libadwaita) doğmuş olsa da, yerel GTK4 ve GStreamer çalışma ortamları sayesinde **Windows 10 ve 11 (64-bit)** üzerinde sorunsuz çalışır.

---

## 🚀 Seçenek 1: Hazır Kurulum / Taşınabilir Sürüm (Kullanıcılar İçin Önerilen)

Önceden derlenmiş taşınabilir Windows zip paketleri [GitHub Actions CI/CD hattımız](https://github.com/MOzcelik14/steM/actions) tarafından otomatik olarak üretilir.

1. [Sürümler (Releases) sayfasına](https://github.com/MOzcelik14/steM/releases) gidin.
2. `steM-Windows-x64.zip` dosyasını indirin.
3. Arşivi istediğiniz bir klasöre çıkartın (örn: `C:\steM`).
4. `steM.exe` dosyasını çalıştırın.

> [!NOTE]
> İlk ayrıştırma işleminde Demucs yapay zeka modelleri otomatik olarak `%APPDATA%\steM\cache` veya kullanıcı dizininize indirilecektir.

---

## 🛠️ Seçenek 2: Kaynak Koddan Çalıştırma (Geliştiriciler İçin)

Windows üzerinde GTK4, Libadwaita ve GStreamer ortamı en kararlı biçimde **MSYS2 UCRT64** ile sağlanır.

### 1. MSYS2 Kurulumu
[msys2.org](https://www.msys2.org/) adresinden MSYS2'yi indirip kurun.

### 2. Bağımlılıkları Yükleme
**MSYS2 UCRT64** terminalini açın ve şu komutları çalıştırın:

```bash
pacman -Syu
pacman -S \
  mingw-w64-ucrt-x86_64-python \
  mingw-w64-ucrt-x86_64-python-pip \
  mingw-w64-ucrt-x86_64-python-gobject \
  mingw-w64-ucrt-x86_64-python-cairo \
  mingw-w64-ucrt-x86_64-gtk4 \
  mingw-w64-ucrt-x86_64-libadwaita \
  mingw-w64-ucrt-x86_64-gstreamer \
  mingw-w64-ucrt-x86_64-gst-plugins-base \
  mingw-w64-ucrt-x86_64-gst-plugins-good \
  mingw-w64-ucrt-x86_64-gst-plugins-bad \
  mingw-w64-ucrt-x86_64-gst-libav \
  mingw-w64-ucrt-x86_64-ffmpeg \
  mingw-w64-ucrt-x86_64-pyinstaller
```

### 3. Depoyu Klonlama ve Yapay Zeka Paketlerini Kurma
```bash
git clone https://github.com/MOzcelik14/steM.git
cd steM

python -m pip install --upgrade pip
# NVIDIA CUDA GPU hızlandırması için:
pip install torch torchaudio --index-url https://download.pytorch.org/whl/cu124
# Yalnızca CPU için:
# pip install torch torchaudio --index-url https://download.pytorch.org/whl/cpu

pip install demucs soundfile numpy scipy
```

### 4. steM.'i Başlatma
```bash
python -m stem.app
```

Veya `scripts/run_windows.bat` betiğine çift tıklayabilirsiniz.

---

## 📦 Seçenek 3: Bağımsız .exe Olarak Paketleme

PyInstaller ile tek parça taşınabilir uygulama derlemek için:

### MSYS2 UCRT64 Ortamında:
```bash
python scripts/build_windows.py
```

### Windows PowerShell Ortamında:
```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
.\scripts\build_windows.ps1
```

Derlenen paket `dist/steM/` dizininde ve `dist/steM-Windows-x64.zip` arşivinde hazır olacaktır.
