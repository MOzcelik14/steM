# steM. Teknik Denetim ve Mimari Analiz Raporu

**Denetim Tarihi:** 2026-11-22  
**Denetlenen Sürüm:** `0.6.1` (`Cargo.toml`)  
**Kapsam:** Yalnızca güncel Rust workspace (`stem-core`, `stem-dsp`, `stem-audio`, `stem-i18n`, `stem-app`). Kod, bağımlılıklar ve yapılandırma üzerinde hiçbir değişiklik yapılmamıştır.

---

## 1. Yönetici Özeti

**steM.**, beş crate'li temiz bir workspace mimarisine, sıfır `unsafe` bloğa, `rtrb` halka tamponları + `crossbeam-channel` üzerinden atomik zaman çizelgesi değişimine dayalı gerçek zamanlı ses geri çağırma hattına ve kapsamlı birim/entegrasyon testlerine sahiptir. Bununla birlikte, gerçek zamanlı ses motoru ile çevrimdışı dışa aktarım arasındaki **miksaj/efekt eşitsizliği**, arayüz iş parçacığında yapılan **senkron tam-proje render/decode/DSP işlemleri**, **kayıt sırasında diske akış (streaming) olmaması** ve **Undo/Redo ile kurtarma günlüğü tutarsızlıkları** üretim kalitesini doğrudan etkileyen başlıca risklerdir.

### En Kritik 5 Bulgu

1. **Gerçek Zamanlı Çalma (`AudioProcessor`) ile Dışa Aktarım (`export.rs`) Eşitsizliği (Kritik — `AUD-01`):**  
   `AudioProcessor::process_interleaved` (`crates/stem-audio/src/callback.rs:397-443`) tüm kanalları doğrudan Master'a toplar; submix bus yönlendirmesi (`SubmixBus`), pre/post-fader aux send'ler, pan/send otomasyonu ve harici sidechain ducking yalnızca `export.rs` içinde uygulanır. Ayrıca `AudioProcessor::with_config` başlangıçta `EffectChain::default_chain` oluşturur (`callback.rs:197,206`) ve projeden yüklenen `track.effects` / `project.master_effects` yapılandırmalarını ses motoruna aktaran bir `AudioCommand` bulunmaz.
2. **Kayıt Sırasında Tüm Sesin RAM'de Biriktirilmesi ve Overrun Veri Kaybı (Kritik — `REC-01`):**  
   `AudioRecorder` arka plan iş parçacığı (`crates/stem-audio/src/record.rs:222-240`) tüm kayıt süresince örnekleri bellekteki tek bir `Vec<f32>` içinde biriktirir ve yalnızca `stop_recording()` çağrıldığında döndürür (`record.rs:301-339`). Uzun kayıtlarda bellek büyür, çökme durumunda tüm kayıt kaybolur ve `RealtimeRecordCallback::process` (`record.rs:431-433`) ring buffer taşmasında sessizce örnek düşürür (stereo hizası bozulma riski taşır).
3. **UI İş Parçacığını Kilitleyen Senkron İşlemler (Kritik — `PRF-01`):**  
   `StemApp::sync_timeline_to_audio_engine` (`crates/stem-app/src/app/mod.rs:243-257`) UI iş parçacığında hem `render_project_mixdown` hem `render_project_tracks` çağırır. Bu fonksiyon otomasyon noktası sürüklenirken **her karede** (`views/timeline.rs:1203-1215`) ve zaman seçimi gibi ses değiştirmeyen sürüklemelerin bitiminde dahi (`views/timeline.rs:1892-1893`) çalışır. Ek olarak dosya açma/referans karşılaştırma decode işlemleri (`app/waveform_ops.rs:195, 602`) ve tüm STFT onarım/DSP işlemleri UI iş parçacığında senkron yürütülür.
4. **Hatalı Başlıklarla Bellek Tükenmesi (OOM) Riski ve Sınırsız RON Deserileştirme (Yüksek — `SEC-01`, `SEC-02`):**  
   `decode_stream_inner` (`crates/stem-audio/src/decode.rs:359-364`), kanal başına `MAX_DECODE_FRAMES` (`2.764.800.000` örnek) kadar ön bellek (`reserve`) ayırmaya çalışır; 8 kanallı sahte bir başlık ~88,5 GB ayırmayı tetikleyebilir. `Project::from_ron` (`crates/stem-core/src/project.rs:1442-1446`) ise girdi boyutu veya özyineleme derinliği sınırı olmadan `ron::from_str` çağırır.
5. **Yaygın Undo/Redo Eksiklikleri ve `WaveformHistory::default()` Panik Riski (Yüksek — `DAT-01`, `DAT-02`):**  
   `WaveformHistory::default()` `max_entries: 0` üretir ve `push_state` (`crates/stem-core/src/waveform.rs:708-709`) boş `Vec` üzerinde `remove(0)` çağırarak panikler. `UndoRedoStack::default()` (`crates/stem-core/src/history.rs:231-235`) `max_history: 0` ile tüm eylemleri siler. Ayrıca UI'daki fade, split, ripple delete, silence, mikser, efekt, kayıt ve `action_waveform_apply_to_project` işlemlerinin çoğu `self.history` yığınına yazmaz.

### Çalıştırılan Doğrulama Komutları ve Sonuçları

| Komut | Durum | Özet Çıktı |
| :--- | :--- | :--- |
| `cargo check --workspace` | Başarılı (0 hata) | Tüm 5 crate hatasız derlendi. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Başarılı (0 uyarı) | Tüm hedefler uyarı vermeden geçti. |
| `cargo test --workspace` | Başarılı | `stem-audio` (23 test), `stem-core` (33 test), `stem-dsp` (27 test), `stem-i18n` (3 test), `stem-app` (3 test) — toplam 89 test geçti, 0 başarısız. |
| `cargo audit` | Tamamlandı (10 uyarı) | `heapless 0.8.0` için 9 adet `unsound` bildirimi (`RUSTSEC-2026-0574`..`RUSTSEC-2026-0582`, tarih 2026-11-18) ve `async-executor 1.14.0` için `yanked` uyarısı tespit edildi. |

---

## 2. Bulgu Tablosu

| ID | Önem | Kategori | Dosya:satır | Açıklama | Önerilen çözüm | Efor |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `AUD-01` | Kritik | Ses Doğruluğu | `crates/stem-audio/src/callback.rs:197,206,397-443` | Gerçek zamanlı oynatma `busses`, `sends`, pan/send otomasyonu, sidechain ve proje başlangıç efektlerini uygulamıyor. | `AudioProcessor` içine bus/send mikseri ve `LoadProjectEffects` komutu eklenmeli veya arka planda render edilen bus/efekt katmanı senkronize edilmeli. | L |
| `REC-01` | Kritik | Kayıt | `crates/stem-audio/src/record.rs:222-240,301-339,431-433` | Kayıt örnekleri diske değil RAM `Vec<f32>`'e yazılıyor; overrun sayaçsız ve stereo kanalları kaydırabiliyor. | Arka plan iş parçacığında `BufWriter`/WAV akışına artımlı yazım eklenmeli; `rtrb` yazımı stereo çift (2 örnek) atomikliğiyle korunmalı. | M |
| `PRF-01` | Kritik | Performans | `crates/stem-app/src/app/mod.rs:243-257`, `views/timeline.rs:1203-1215,1892` | `sync_timeline_to_audio_engine` UI iş parçacığında tam miksaj yapıyor; otomasyon sürüklerken her karede çağrılıyor. | Render işlemini arka plan iş parçacığına taşıyıp yalnızca sürükleme bitiminde veya değişen kanal için tetikle. | L |
| `SEC-01` | Yüksek | Güvenlik | `crates/stem-audio/src/decode.rs:359-364` | Sahte ses dosyası başlığı `8 * MAX_DECODE_FRAMES` (~88,5 GB) `Vec::with_capacity` ayırmaya çalışıp OOM yaratabilir. | Ön ayırmayı `est_frames.min(sample_rate * 60)` gibi makul bir üst sınırla kademeli büyüt. | S |
| `SEC-02` | Yüksek | Güvenlik | `crates/stem-core/src/project.rs:1442-1446,879-881` | `Project::from_ron` girdi boyutu ve özyineleme derinliği sınırı olmadan deserileştirme yapıyor. | Maksimum dosya/string boyutu (ör. 32 MB) ve `ron::Options::default().with_default_extension(...)` / derinlik sınırı ekle. | S |
| `DAT-01` | Yüksek | Veri Modeli | `crates/stem-core/src/waveform.rs:690,708-709`, `history.rs:231-235` | `WaveformHistory::default()` `push_state` sırasında panikler; `UndoRedoStack::default()` `max_history=0` nedeniyle geçmiş tutmaz. | `Default` yerine `Self::new()`'e delege et; `undo`/`redo` başarısız olursa eylemi geri koy. | S |
| `DAT-02` | Yüksek | Veri Modeli | `crates/stem-app/src/app/project_ops.rs:235-259`, `transport_ops.rs:624-896`, `waveform_ops.rs:659-695` | Split, fade, silence, ripple delete, mikser, efekt, kayıt ve `apply_to_project` işlemleri `UndoRedoStack`'e girmiyor. | Tüm proje mutasyonlarını `Action` enum'una bağla veya snapshot tabanlı komutlara geçir. | L |
| `UX-01` | Yüksek | Kısayollar / UX | `crates/stem-app/src/app/shortcuts.rs:21,135-140,232` | `Ctrl+S` basıldığında modifier kontrolü olmadığı için `S` (Split Clip) de tetikleniyor; `Escape` Stop ile seçimi her seferinde siliyor. | `Key::S` için `!input.modifiers.any()` ekle; `Escape` durdurma ile seçim temizlemeyi ayır. | S |
| `DAT-03` | Yüksek | Kurtarma | `crates/stem-app/src/app/mod.rs:157-159,608-614`, `project_ops.rs:358-372` | `.stem-recovery` `cwd`'ye yazılıyor ve `can_undo()` true ise temiz çıkışta bile silinmeyip yanlış kurtarma penceresi açıyor. | OS kullanıcı veri dizinine taşı; kullanıcı temiz çıkış yaptığında (veya kaydettiğinde) kurtarma dosyasını temizle. | M |
| `AUD-02` | Yüksek | Ses Doğruluğu | `crates/stem-audio/src/callback.rs:160-223`, `export.rs:484-586` | Donanım örnekleme hızı ile proje/kaynak örnekleme hızı farklıysa gerçek zamanlı çalmada hız/perde kayması oluşur. | Kaynakları içe aktarımda proje hızına ve/veya senkronizasyonda donanım hızına örnekle (`resample_audio`). | M |
| `DSP-01` | Orta | DSP | `crates/stem-dsp/src/analysis.rs:117-137` | `SpectrumAnalyzer::compute_magnitude_db` her çağrıda `plan_fft_forward` planlayıp 3 adet `Vec` ayırıyor. | `Arc<dyn RealToComplex<f32>>` planını ve çalışma tamponlarını struct içinde önbellekle. | S |
| `DSP-02` | Orta | DSP | `crates/stem-dsp/src/chain.rs:136-175,274-385,459-726` | `SmoothedParam` yalnızca 3 parametrede kullanılıyor; EQ, Compressor eşiği, Limiter, Delay süresi ve Bypass ani değişimde zipper noise üretiyor. | Kritik kazanç/frekans/bypass geçişlerine `SmoothedParam` veya kısa crossfade ekle. | M |
| `DSP-03` | Orta | DSP | `crates/stem-dsp/src/dynamics.rs:311-417`, `restoration.rs:536,571-577` | `DeEsser` faz-uyumsuz `input - sib` çıkarması yapıyor; `SpectralRepair::interpolate_region` `.min(1.0)` ile boşlukları yükseltemiyor. | `DeEsser`'i Linkwitz-Riley ayrıştırmaya geçir; `interpolate_region` kazancını güvenli üst sınırla (ör. `4.0`) serbest bırak. | M |
| `DSP-04` | Orta | DSP | `crates/stem-dsp/src/dynamics.rs:144-196`, `analysis.rs:141-185,362-368` | `Limiter` lookahead içermiyor (ani saldırı); `compute_true_peak_dbfs` FIR yerine kübik Hermite kullanıyor; `TonalBalance` tek 4096 pencerede hesaplanıyor. | Lookahead gecikmesi (1–5 ms) ekle; `TonalBalance`'ı parça geneline ortala. | M |
| `PRF-02` | Orta | Performans | `crates/stem-app/src/views/spectrogram.rs:375-419,489-500`, `waveform.rs:838-857`, `timeline.rs:2048-2085` | Şelale spektrogram her karede 4.800 `rect_filled`, waveform/timeline 2.000–6.000 `line_segment` çiziyor; `timeline` tüm klibi sorguluyor. | Şelaleyi `ColorImage`/`TextureHandle` ile çiz; `timeline` peak sorgusunu görünür ekran aralığına kırp ve `Shape::line` kullan. | M |
| `PRF-03` | Orta | Bellek | `crates/stem-core/src/waveform.rs:250-277,479-524` | `copy_range` ve `render_range` tek bir dilim için tüm belgeyi render ediyor; sanal `source_buffers` hiç temizlenmiyor. | Yalnızca kesişen segmentleri render et ve geçmişten düşen sanal `Uuid`'leri toplayan GC ekle. | M |
| `SEC-03` | Orta | Güvenlik | `crates/stem-audio/src/export.rs:963-975`, `plugins.rs:273-277`, `project.rs:1491-1501` | `format_export_filename` path ayırıcılarını temizlemiyor; `inspect_lv2_bundle` ve `collect_all_and_save` yol çakışması/traversal kontrolü yapmıyor. | Dosya adı belirteçlerini sanitize et; `bundle_path.join` sonrası `canonicalize`/alt-dizin doğrulaması yap; `Audio/` kopyalamada çakışma önle. | S |
| `REC-02` | Orta | Kayıt | `crates/stem-audio/src/record.rs:118-134,207-210` | Cihaz kaybında yalnızca `device_lost` bayrağı set ediliyor, otomatik akış yenileme yok. | Hata sonrası kullanıcıya uyarı ve yeniden bağlan/kurtar akışı sun. | S |
| `I18N-01` | Orta | Yerelleştirme | `crates/stem-app/src/views/{menu,spectrogram,dialogs,effects,timeline}.rs`, `stem-i18n/src/lib.rs:253-263` | 30+ yeni UI metni (Mastering, Spectral Repair, Plugin Browser, Demucs, durum mesajları) İngilizce hardcoded; `TimecodeMode` 30fps/120BPM sabit. | Tüm metinleri `en.ftl`/`tr.ftl` anahtarlarına taşı; zaman kodunu aktif `Project` hızına bağla. | M |
| `UX-02` | Orta | UX / Erişilebilirlik | `crates/stem-app/src/app/mod.rs:569-605`, `views/effects.rs`, `state.rs:495-524` | `show_history_panel` menüde var ama panel render edilmiyor; `EffectsRackState` kanal/bus bazında değil global; kısayol tablosu 27 tuşla sınırlı. | Geçmiş panelini çiz; efekt rafını seçili track/bus `effects` vektörüne bağla; tuş haritasını genişlet. | M |
| `QUA-01` | Orta | CI/CD & Bağımlılık | `.github/workflows/`, `Cargo.lock` | PR/push üzerinde çalışan CI (`fmt`, `clippy`, `test`, `deny`) yok; `cargo audit` `heapless 0.8.0` (9 unsound) ve `async-executor 1.14.0` (yanked) raporluyor. | `.github/workflows/ci.yml` ekle; `cargo update` ile bağımlılıkları güncelle ve Windows/macOS testlerini etkinleştir. | S |
| `QUA-02` | Düşük | Kod Kalitesi | `crates/stem-audio/src/decode.rs:292-297`, `export.rs:119-586` | `catch_unwind_silent` global `HOOK_LOCK` mutex'i ile tüm decode çağrılarını serileştiriyor; `render_project_mixdown_range` ve `render_project_tracks` kod tekrarlıyor. | Ortak kanal render yardımcısı çıkar; global panic hook değişimini kaldır veya test/başlatma aşamasına al. | S |

---

## 3. Kritik ve Yüksek Bulgular İçin Ayrıntılı Açıklama ve Düzeltme Önerileri

### `AUD-01`: Gerçek Zamanlı Oynatma (`callback.rs`) ile Dışa Aktarım (`export.rs`) Eşitsizliği
- **Durum:** `StemApp::sync_timeline_to_audio_engine` (`crates/stem-app/src/app/mod.rs:243-257`) hem `render_project_mixdown` (`channels`) hem `render_project_tracks` (`track_channels`) üretir. Ancak `AudioProcessor::process_interleaved` (`callback.rs:397-443`) `track_channels` boş olmadığı sürece `timeline.channels` tamponunu hiç okumaz; bunun yerine her kanalı tek tek `self.track_effects[t]` içinden geçirip doğrudan Master'a toplar. Bu nedenle `SubmixBus` yönlendirmeleri, pre/post-fader send'ler, pan/send otomasyonu, sidechain ve proje dosyasından yüklenen `track.effects` / `master_effects` gerçek zamanlı dinlemede duyulmaz.
- **Somut Düzeltme:** `AudioCommand::LoadTrackEffects { track_idx, configs }` ve `LoadMasterEffects(Vec<EffectConfig>)` komutları eklenerek efekt zincirleri arka planda/komutla güncellenmeli; `SharedAudioTimeline` içine önceden mikslenmiş bus tamponları veya `AudioProcessor` içine sabit boyutlu `BusMixerState` (`MAX_BUSSES`) eklenmelidir.

### `REC-01`: Kayıt Sırasında RAM Birikimi ve Ring Buffer Taşması
- **Durum:** `crates/stem-audio/src/record.rs:222-240` kayıt worker iş parçacığında örnekleri diske yazmak yerine büyüyen bir `Vec<f32>` içine iter. Uzun bir kayıt oturumunda uygulama çökerse hiçbir veri diske yazılmamış olur. Ayrıca `RealtimeRecordCallback::process` (`record.rs:431-433`) örnekleri tek tek `record_producer.push(sample)` ile gönderdiği için tampon dolduğunda tek sayıda örnek düşüp L/R kanallarını ters çevirebilir.
- **Somut Düzeltme:** Worker iş parçacığında `hound::WavWriter<BufWriter<File>>` (geçici `.stem-rec-{uuid}.wav` dosyasına) artımlı yazım yapılmalı; callback tarafında ise `record_producer.slots() >= ch_count` kontrolü yapılarak tam kare (frame) bütünlüğü korunmalı ve bir `AtomicU64` overrun sayacı UI'a raporlanmalıdır.

### `PRF-01`: UI İş Parçacığında Senkron Render, Decode ve DSP
- **Durum:** `sync_timeline_to_audio_engine` (`app/mod.rs:243-257`) her çağrıda tüm zaman çizelgesini yeniden render eder. `views/timeline.rs:1203-1215` (otomasyon sürüklerken her kare) ve `views/timeline.rs:1892-1893` (zaman seçimi bırakıldığında bile) bu fonksiyonu tetikler. Ayrıca `action_open_file_in_waveform_editor` (`waveform_ops.rs:195`), `action_compare_reference_audio` (`waveform_ops.rs:602`) ve tüm STFT restorasyon araçları UI iş parçacığında çalışarak arayüzü dondurur.
- **Somut Düzeltme:** Zaman çizelgesi render, decode ve STFT işlemlerini `std::thread::spawn` + `crossbeam_channel` worker hattına taşıyın; otomasyon/sürükleme işlemlerinde senkronizasyonu fare bırakma (`drag_stopped`) anına erteleyin.

### `SEC-01` & `SEC-02`: Bozuk Medya ve RON Dosyalarıyla OOM / DoS Riski
- **Durum:** `decode_stream_inner` (`decode.rs:359-364`) konteyner başlığındaki `n_frames` değerini `MAX_DECODE_FRAMES` ile sınırlar ancak bu sınır 8 kanallı `f32` için hâlâ ~88,5 GB ön ayırma (`Vec::with_capacity`) demektir. `Project::from_ron` (`project.rs:1442-1446`) ise hiçbir boyut/derinlik sınırı olmadan `ron::from_str` çalıştırır.
- **Somut Düzeltme:** `decode.rs` içinde başlangıç `with_capacity` değerini `est_frames.min(sample_rate as usize * 60)` ile sınırlayın (gerçek veri geldikçe büyüsün); `from_ron` öncesinde dosya/string boyutunu kontrol edin ve `ron::Options` ile makul bir özyineleme sınırı tanımlayın.

### `DAT-01`, `DAT-02` & `DAT-03`: Undo/Redo Tutarsızlıkları ve Kurtarma Günlüğü
- **Durum:** `WaveformHistory::default()` (`waveform.rs:690,708-709`) `max_entries == 0` yüzünden `push_state` çağrısında doğrudan panikler. `UndoRedoStack::default()` (`history.rs:231-235`) tüm eylemleri düşürür. UI aksiyonlarının çoğu (`split`, `fade`, `silence`, `ripple_delete`, `mixer`, `effects`, `record`, `apply_to_project`) geçmişe yazılmaz. `.stem-recovery` (`app/mod.rs:157-159,608-614`) çalışma dizinine yazılır ve `can_undo()` true iken temiz çıkışta silinmez.
- **Somut Düzeltme:** `Default` implementasyonlarını `Self::new()`'e bağlayın; tüm düzenleme komutlarını `Action` enum'u üzerinden geçirin; `.stem-recovery` dosyasını OS kullanıcı veri/cache dizinine taşıyıp temiz çıkışta koşulsuz silin.

### `AUD-02` & `UX-01`: Örnekleme Hızı Uyumsuzluğu ve Kısayol Çakışması
- **Durum:** `AudioProcessor` farklı örnekleme hızındaki kaynakları gerçek zamanlı oynatmada yeniden örneklemez; `Ctrl+S` basıldığında `!input.modifiers.any()` kontrolü olmadığı için `S` (Split Clip) komutu da tetiklenir (`app/shortcuts.rs:21,232`).
- **Somut Düzeltme:** İçe aktarılan sesler proje hızından farklıysa içe aktarım worker'ında `resample_audio` uygulanmalı; `shortcuts.rs:232` satırına `!input.modifiers.any()` eklenmelidir.

---

## 4. Eski Python / GTK4 / Demucs Kalıntıları İçin Temizlik Listesi

- **Python / GTK4 Dosyaları:** Repoda eski Python veya GTK4 sürümüne ait `.py`, `.ui`, `pyproject.toml` ya da `gi`/GTK4 bağımlılık kalıntısı **bulunmamaktadır** (tamamen temizlenmiştir).
- **Demucs Entegrasyonu (Karar Gerektiren Aktif Kod):** Aşağıdaki dosya ve satırlarda harici `demucs` / `python3 -m demucs` CLI çağrısı aktif özellik olarak yer almaktadır:
  - `crates/stem-audio/src/stems.rs:136, 145-174, 212-226` (`DemucsRunner`, `Command::new("python3")`)
  - `crates/stem-app/src/app/project_ops.rs:593-691` ve `crates/stem-app/src/app/mod.rs:471-530`
  - `crates/stem-app/src/state.rs:766-790`, `crates/stem-app/src/views/dialogs.rs:1869-1943`, `crates/stem-app/src/views/menu.rs:481`
  - *Öneri:* Harici Python/Demucs CLI bağımlılığı istenmiyorsa bu modüller çıkarılmalı; korunacaksa isteğe bağlı eklenti/araç olarak ayrı dokümante edilmelidir.

---

## 5. Kısa / Orta / Uzun Vadeli Geliştirme Yol Haritası

### Kısa Vadeli (1–2 Hafta — Kararlılık ve Güvenlik)
1. `UX-01` (`Ctrl+S` / `S` çakışması ve `Escape` davranışı), `DAT-01` (`WaveformHistory`/`UndoRedoStack` `Default` düzeltmesi), `SEC-01` (`decode.rs` OOM ön ayırma sınırı) ve `SEC-02` (`from_ron` boyut/derinlik sınırı).
2. `DAT-03` (`.stem-recovery` dizini ve temiz çıkış silme mantığı) + `action_new_project` ses motoru temizliği.
3. `QUA-01` (`.github/workflows/ci.yml` eklenmesi ve `heapless`/`async-executor` bağımlılık güncellemesi).

### Orta Vadeli (3–6 Hafta — Ses Eşitliği, Kayıt ve Performans)
1. `REC-01`: Kayıt sırasında diske artımlı WAV yazımı ve overrun sayacı.
2. `PRF-01` & `PRF-02`: `sync_timeline_to_audio_engine`, STFT onarım ve decode işlemlerinin arka plan worker'a alınması; spektrogram `TextureHandle` ve timeline viewport peak kırpması.
3. `AUD-01` & `AUD-02`: Gerçek zamanlı ses motorunda proje efektlerinin yüklenmesi, bus/send/sidechain eşitliği ve içe aktarım sırasında otomatik `resample_audio`.
4. `I18N-01` & `UX-02`: Kalan hardcoded metinlerin `en.ftl`/`tr.ftl`'ye taşınması, `EffectsRackState`'in kanal/bus bazlı hale getirilmesi ve Geçmiş paneli UI'ının çizilmesi.

### Uzun Vadeli (6+ Hafta — Mimari ve Gelişmiş DAW Özellikleri)
1. `DAT-02`: Tüm düzenleme/mikser/otomasyon işlemleri için birleşik komut tabanlı Undo/Redo mimarisi.
2. `DSP-02`, `DSP-03`, `DSP-04`: Tüm efekt parametrelerinde zipper-free yumuşatma, lookahead Limiter, Linkwitz-Riley De-Esser ve bütün parça ortalamalı `TonalBalance`.
3. Gerçek zamanlı Plugin Delay Compensation (PDC) ve VST3/LV2 ses işleme köprüsü.

---

## 6. Bir Sonraki Prompt İçin Önerilen Aşama Aşama İş Bölümü

Düzeltmeleri güvenli ve doğrulanabilir şekilde uygulamak için aşağıdaki 4 aşamalı iş bölümü önerilir:

- **Aşama 1 — Kritik Hata ve Güvenlik Yamaları:** `UX-01`, `DAT-01`, `DAT-03`, `SEC-01`, `SEC-02`, `SEC-03` (`cargo test` ve fuzz/security testleriyle doğrulanır).
- **Aşama 2 — Kayıt Güvenliği ve Asenkron İş Hattı:** `REC-01`, `REC-02`, `PRF-01`, `PRF-02`, `PRF-03` (UI akıcılığı ve sıfır-allocation ses testiyle doğrulanır).
- **Aşama 3 — Gerçek Zamanlı Ses / DSP Eşitliği:** `AUD-01`, `AUD-02`, `DSP-01`, `DSP-02`, `DSP-03`, `DSP-04` (export vs realtime parity testiyle doğrulanır).
- **Aşama 4 — Tam Undo/Redo, Yerelleştirme ve CI:** `DAT-02`, `I18N-01`, `UX-02`, `QUA-01`, `QUA-02` (`en`/`tr` parity + `.github/workflows/ci.yml` ile doğrulanır).
