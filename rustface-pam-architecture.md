# RustFace-PAM: Linux Yüz Tanıma Kimlik Doğrulama Modülü

**Dil:** Rust  
**Platform:** Arch Linux  
**Donanım:** Dell Latitude 5550 — IR Kamera `/dev/video2`  
**Yazar:** Anıl Akpınar (4ni1ak)

---

## İçindekiler

1. [Proje Genel Bakış](#1-proje-genel-bakış)
2. [Sistem Mimarisi](#2-sistem-mimarisi)
3. [Kritik Bileşenler](#3-kritik-bileşenler)
4. [Veri Akışı](#4-veri-akışı)
5. [Dizin Yapısı](#5-dizin-yapısı)
6. [Bağımlılıklar](#6-bağımlılıklar)
7. [PAM Entegrasyonu](#7-pam-entegrasyonu)
8. [IR Kamera Pipeline](#8-ir-kamera-pipeline)
9. [Yüz Tanıma Pipeline](#9-yüz-tanıma-pipeline)
10. [Güvenlik Modeli](#10-güvenlik-modeli)
11. [Hata Yönetimi](#11-hata-yönetimi)
12. [Geliştirme Sırası](#12-geliştirme-sırası)
13. [Test Stratejisi](#13-test-stratejisi)

---

## 1. Proje Genel Bakış

### Ne Yapıyor?

`sudo` komutu çalıştırıldığında Linux PAM sistemi devreye girer ve kimlik doğrulama ister. Bu proje, şifre yerine IR kamera ile yüz tanıma yapan bir PAM modülü yazıyor.

```
Kullanıcı: sudo pacman -Syu
Sistem: PAM devreye girer
PAM: rustface-pam.so çağırır
Modül: IR kameradan yüz okur
Modül: Kayıtlı yüzle karşılaştırır
Sonuç: Eşleşme var → sudo onaylandı
        Eşleşme yok → şifre sor
```

### Neden Rust?

| Özellik | C | Python | Rust |
|---|---|---|---|
| Memory safety | ❌ Manuel | ✅ GC var | ✅ Compile-time |
| PAM uyumu | ✅ Native | ⚠️ pam_python | ✅ FFI ile |
| Hız | ✅ En hızlı | ❌ Yavaş | ✅ C ile aynı |
| Güvenlik hatası riski | Yüksek | Orta | Çok düşük |
| Buffer overflow | Mümkün | Yok | Compile'da engellenir |

PAM modülü root context'te çalışır. Memory corruption burada sistem güvenliğini doğrudan etkiler — bu yüzden Rust tercih ediliyor.

---

## 2. Sistem Mimarisi

```
┌─────────────────────────────────────────────────────────┐
│                    KULLANICI KATMANI                     │
│   sudo / su / login / screensaver unlock                │
└─────────────────────────┬───────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│                     PAM KATMANI                          │
│   /etc/pam.d/sudo                                       │
│   auth sufficient pam_rustface.so                       │
└─────────────────────────┬───────────────────────────────┘
                          │  dlopen() ile yükler
                          ▼
┌─────────────────────────────────────────────────────────┐
│                  RUSTFACE-PAM.SO                         │
│                                                         │
│  ┌─────────────┐    ┌──────────────┐    ┌────────────┐ │
│  │  PAM Layer  │───▶│  Auth Engine │───▶│  Result    │ │
│  │  (FFI/C ABI)│    │  (Rust core) │    │  PAM_SUCCESS│ │
│  └─────────────┘    └──────┬───────┘    └────────────┘ │
│                            │                            │
│              ┌─────────────┴──────────────┐            │
│              │                            │            │
│              ▼                            ▼            │
│  ┌───────────────────┐    ┌───────────────────────┐   │
│  │  Camera Module    │    │  Face Recognition     │   │
│  │  v4l2 → frames    │    │  Module               │   │
│  │  /dev/video2      │    │  embedding compare    │   │
│  └───────────────────┘    └───────────────────────┘   │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│                  DEPOLAMA KATMANI                        │
│   /etc/rustface/                                        │
│   ├── models/          (yüz tanıma modeli)              │
│   └── faces/           (kayıtlı yüz vektörleri)        │
└─────────────────────────────────────────────────────────┘
```

### Mimari Kararlar

**Shared library (.so) olarak derlenir.**
PAM, modülleri dinamik olarak yükler. Rust kodu `cdylib` olarak derlenmeli.

**C ABI zorunlu.**
PAM sistemi C fonksiyon signature'ı bekler. Rust'tan `extern "C"` ile export edilmeli.

**Senkron çalışır.**
PAM auth süreci blocking'dir — async runtime kullanılmaz, tokio gerekmez.

**IPC yok.**
Modül ayrı bir daemon çalıştırmaz. Her sudo çağrısında kamera açılır, işlem yapılır, kamera kapanır.

---

## 3. Kritik Bileşenler

### 3.1 PAM Entry Point — EN KRİTİK

```rust
// Bu fonksiyon PAM tarafından çağrılır
// İmza TAM olarak bu olmalı — bir harf bile farklı olursa PAM yükleyemez

#[no_mangle]
pub extern "C" fn pam_sm_authenticate(
    pamh: *mut PamHandle,    // PAM handle — kullanıcı bilgisi buradan alınır
    flags: c_int,            // PAM flags
    argc: c_int,             // config argüman sayısı
    argv: *const *const c_char, // config argümanları
) -> c_int {
    // PAM_SUCCESS = 0
    // PAM_AUTH_ERR = 7
    // PAM_IGNORE = 25 — bu modülü atla, bir sonrakine geç
}
```

**Neden kritik:** Bu fonksiyonun return değeri sudo'nun çalışıp çalışmamasını belirler. Yanlış değer döndürürsen ya her zaman sudo verir ya da hiç vermez.

**Güvenli yazım kuralı:**
- Herhangi bir hata durumunda `PAM_IGNORE` dön — şifre ekranına düş
- `PAM_AUTH_ERR` sadece yüz tanındı ama eşleşmedi durumunda dön
- Panic olursa sistem kilitlenir — tüm kod `catch_unwind` içine alın

### 3.2 Camera Capture

```rust
pub struct IrCamera {
    device: Device,          // v4l crate Device
    stream: MmapStream,      // zero-copy frame stream
    device_path: String,     // /dev/video2
}

impl IrCamera {
    pub fn open(path: &str) -> Result<Self, CameraError>
    pub fn capture_frame(&mut self) -> Result<GrayImage, CameraError>
    pub fn close(self)  // Drop impl ile otomatik kapanır
}
```

**Neden kritik:** Kamera açık kalırsa bir sonraki sudo çağrısında cihaz meşgul hatası alınır. RAII pattern ile Drop trait'i implement et — scope dışına çıkınca otomatik kapansın.

### 3.3 Face Embedding

```rust
pub struct FaceRecognizer {
    model: FaceModel,        // yüklü model (belleğe bir kez alınır)
}

impl FaceRecognizer {
    // Görüntüden yüz bölgesini bul
    pub fn detect(&self, image: &GrayImage) -> Option<FaceRegion>
    
    // Yüzü 128 boyutlu vektöre dönüştür
    pub fn embed(&self, image: &GrayImage, region: &FaceRegion) -> Embedding
    
    // İki embedding arasındaki mesafeyi hesapla
    pub fn compare(&self, a: &Embedding, b: &Embedding) -> f32
}

// 128 boyutlu f32 vektör — bir yüzün matematiksel temsili
pub struct Embedding([f32; 128]);
```

**Neden kritik:** Eşik değeri (threshold) güvenliği doğrudan etkiler.
- Çok düşük threshold → yanlış kişi geçebilir (güvensiz)
- Çok yüksek threshold → doğru kişi reddedilir (kullanışsız)
- Başlangıç değeri: `0.6` — test ile ayarlanacak

### 3.4 Face Storage

```rust
pub struct FaceStore {
    base_path: PathBuf,   // /etc/rustface/faces/
}

impl FaceStore {
    // Kullanıcı adına göre kayıtlı embedding'leri yükle
    pub fn load(&self, username: &str) -> Result<Vec<Embedding>, StoreError>
    
    // Yeni yüz kaydet
    pub fn save(&self, username: &str, embedding: &Embedding) -> Result<(), StoreError>
    
    // Kullanıcının kayıtlı yüzü var mı?
    pub fn exists(&self, username: &str) -> bool
}
```

**Dosya formatı:** Her kullanıcı için `{username}.bin` — 128 x 4 byte = 512 byte. JSON değil, binary — daha hızlı okuma.

---

## 4. Veri Akışı

```
sudo komutu
    │
    ▼
PAM: pam_sm_authenticate() çağır
    │
    ▼
Kullanıcı adını al: pam_get_user()
    │
    ▼
/etc/rustface/faces/{username}.bin var mı?
    │
    ├── YOK → PAM_IGNORE (şifre ekranına geç)
    │
    └── VAR ↓
         │
         ▼
    /dev/video2 aç
         │
         ▼
    3 saniye içinde frame yakala (max 30 deneme)
         │
         ├── Kamera açılamadı → PAM_IGNORE
         │
         └── Frame alındı ↓
              │
              ▼
         Yüz tespiti (detect)
              │
              ├── Yüz yok → tekrar dene (max 3 kez)
              │   Hep yüz yok → PAM_IGNORE
              │
              └── Yüz bulundu ↓
                   │
                   ▼
              Embedding çıkar (embed)
                   │
                   ▼
              Kayıtlı embedding ile karşılaştır (compare)
                   │
                   ├── Mesafe < 0.6 → PAM_SUCCESS ✅
                   │
                   └── Mesafe ≥ 0.6 → PAM_IGNORE (şifre sor)
```

---

## 5. Dizin Yapısı

```
rustface-pam/
├── Cargo.toml
├── Cargo.lock
├── README.md
│
├── src/
│   ├── lib.rs              # PAM entry point — pam_sm_authenticate
│   ├── pam/
│   │   ├── mod.rs          # PAM types, constants
│   │   ├── handle.rs       # PamHandle wrapper (unsafe)
│   │   └── response.rs     # PAM return codes
│   ├── camera/
│   │   ├── mod.rs          # IrCamera struct
│   │   ├── capture.rs      # v4l2 frame capture
│   │   └── error.rs        # CameraError enum
│   ├── face/
│   │   ├── mod.rs          # FaceRecognizer struct
│   │   ├── detect.rs       # Yüz tespiti
│   │   ├── embed.rs        # Embedding çıkarma
│   │   └── compare.rs      # Cosine similarity
│   ├── storage/
│   │   ├── mod.rs          # FaceStore struct
│   │   ├── read.rs         # Binary embedding okuma
│   │   └── write.rs        # Binary embedding yazma
│   └── error.rs            # Ana hata tipi (thiserror)
│
├── bin/
│   └── rustface-enroll.rs  # Yüz kayıt CLI aracı
│
├── models/
│   └── .gitkeep            # Model dosyası buraya (git'e ekleme)
│
├── tests/
│   ├── camera_test.rs
│   ├── face_test.rs
│   └── storage_test.rs
│
└── install.sh              # /etc/rustface/ kurulum scripti
```

---

## 6. Bağımlılıklar

```toml
[package]
name = "rustface-pam"
version = "0.1.0"
edition = "2021"

# PAM .so olarak derlenmesi için zorunlu
[lib]
crate-type = ["cdylib"]

[dependencies]
# IR kamera — v4l2 Rust wrapper
v4l = "0.14"

# Yüz tespiti — pure Rust, C bağımlılığı yok
rustface = "0.1"

# Görüntü işleme
image = { version = "0.25", default-features = false, features = ["png"] }

# Hata yönetimi
thiserror = "2"

# Panic'i yakala — PAM modülü panic edemez
# catch_unwind için std zaten yeterli

# Logging — syslog'a yazar (journalctl'de görünür)
log = "0.4"
syslog = "7"

[profile.release]
opt-level = 3
lto = true          # Link-time optimization — daha küçük .so
strip = true        # Debug sembollerini çıkar
panic = "abort"     # Panic = process abort, unwind yok (PAM için güvenli)
```

**Neden bu bağımlılıklar?**

- `v4l`: `/dev/video2`'yi Rust'tan açmak için. `ioctl` çağrılarını wrap ediyor.
- `rustface`: dlib veya OpenCV C bağımlılığı olmadan yüz tespiti. Pure Rust.
- `image`: GrayImage tipi — frame buffer'ı işlemek için.
- `thiserror`: ergonomik error handling, `?` operatörü.
- `syslog`: PAM modülleri terminal çıktısı veremez — log syslog'a gider.

---

## 7. PAM Entegrasyonu

### 7.1 PAM Handle'dan Kullanıcı Adı Alma

```rust
// Bu unsafe — C pointer'ı Rust'a çekiyoruz
unsafe fn get_username(pamh: *mut PamHandle) -> Option<String> {
    let mut username_ptr: *const c_char = std::ptr::null();
    
    let ret = pam_get_user(
        pamh,
        &mut username_ptr,
        std::ptr::null(), // prompt — null = default kullan
    );
    
    if ret != PAM_SUCCESS || username_ptr.is_null() {
        return None;
    }
    
    // C string'i Rust String'e güvenli dönüşüm
    CStr::from_ptr(username_ptr)
        .to_str()
        .ok()
        .map(|s| s.to_owned())
}
```

### 7.2 PAM Konfigürasyon Dosyası

```
# /etc/pam.d/sudo
# Bu satır EN ÜSTE eklenmeli

auth  sufficient  /usr/lib/security/pam_rustface.so  threshold=0.6  timeout=3  debug=false
auth  include     system-auth
```

**Parametreler:**
- `threshold=0.6` — eşik değeri, 0.0-1.0 arası
- `timeout=3` — kameraya bakma süresi (saniye)
- `debug=false` — true yapınca journalctl'de detaylı log

### 7.3 Argüman Ayrıştırma

```rust
struct Config {
    threshold: f32,    // default: 0.6
    timeout_secs: u64, // default: 3
    debug: bool,       // default: false
    device: String,    // default: /dev/video2
}

impl Config {
    fn from_pam_args(argc: c_int, argv: *const *const c_char) -> Self {
        // argc ve argv'yi iterate et
        // "key=value" formatını ayrıştır
        // bilinmeyen argümanı yok say — hata verme
    }
}
```

---

## 8. IR Kamera Pipeline

### 8.1 Frame Yakalama

```rust
use v4l::prelude::*;
use v4l::video::Capture;

pub fn capture_frame(device_path: &str) -> Result<GrayImage, CameraError> {
    let dev = Device::new(device_path)?;
    
    // Format kontrol et — Y800 (grayscale) bekliyoruz
    let fmt = dev.format()?;
    ensure!(
        fmt.fourcc == FourCC::new(b"Y800"),
        CameraError::WrongFormat(fmt.fourcc)
    );
    
    // Stream başlat
    let mut stream = MmapStream::new(&dev, Type::VideoCapture)?;
    
    // Frame al
    let (buf, _meta) = stream.next()?;
    
    // Raw bytes'ı GrayImage'a dönüştür
    let width = fmt.width;
    let height = fmt.height;
    GrayImage::from_raw(width, height, buf.to_vec())
        .ok_or(CameraError::FrameConversion)
}
```

**Dell Latitude 5550 IR kamera özellikleri:**
- Format: Y800 (8-bit grayscale)
- Çözünürlük: 640x360
- FPS: 15

### 8.2 Frame Ön İşleme

IR kamera görüntüsü doğrudan yüz tespitine verilmeden önce:

```rust
fn preprocess(image: GrayImage) -> GrayImage {
    // 1. Histogram equalization — kontrastı artır
    //    IR görüntüler düşük kontrastlı olabilir
    let equalized = histogram_equalize(image);
    
    // 2. Gaussian blur — gürültüyü azalt (3x3 kernel yeterli)
    let blurred = gaussian_blur(&equalized, 1.0);
    
    // 3. Normalize — 0-255 aralığına getir
    normalize(blurred)
}
```

---

## 9. Yüz Tanıma Pipeline

### 9.1 Algoritma Seçimi

**Kullanılacak: FaceNet benzeri embedding yaklaşımı**

```
Görüntü → Yüz Tespiti → Hizalama → Embedding → Karşılaştırma
```

1. **Yüz Tespiti:** Viola-Jones (Haar cascade) veya SSD tabanlı detector
2. **Hizalama:** Göz pozisyonlarına göre döndürme
3. **Embedding:** 128 boyutlu vektör çıkarma
4. **Karşılaştırma:** Cosine similarity

### 9.2 Cosine Similarity

```rust
pub fn cosine_similarity(a: &Embedding, b: &Embedding) -> f32 {
    let dot: f32 = a.0.iter().zip(b.0.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.0.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.0.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    
    dot / (norm_a * norm_b)
    // 1.0 = aynı kişi, 0.0 = tamamen farklı
}

pub fn is_match(a: &Embedding, b: &Embedding, threshold: f32) -> bool {
    cosine_similarity(a, b) >= threshold
}
```

### 9.3 Spoof Koruması (Anti-Spoofing)

IR kamera burada avantaj sağlar — fotoğraftan geçmeyi zorlaştırır:

```rust
// IR görüntüde yüz tespiti yapıldıktan sonra:
fn liveness_check(image: &GrayImage, face: &FaceRegion) -> bool {
    // IR görüntüde doku analizi
    // Gerçek cilt vs baskılı kağıt vs ekran
    // LBP (Local Binary Pattern) texture analizi
    // Basit versiyon: yeterli yoğunluk varyasyonu var mı?
    
    let variance = compute_region_variance(image, face);
    variance > LIVENESS_THRESHOLD  // deneme yanılma ile belirlenecek
}
```

---

## 10. Güvenlik Modeli

### 10.1 Tehdit Modeli

| Tehdit | Risk | Koruma |
|---|---|---|
| Fotoğraf ile bypass | Orta | IR kamera — kağıt/ekran farklı yansıtır |
| 3D maske | Düşük | IR doku analizi |
| Kayıtlı embedding çalınması | Düşük | /etc/rustface/ root-only |
| Race condition | Orta | Mutex ile kamera erişimi serialize et |
| Panic → lock bypass | Yok | `panic = "abort"` + `catch_unwind` |

### 10.2 Dosya İzinleri

```bash
# Kurulum sırasında
sudo chown -R root:root /etc/rustface/
sudo chmod 700 /etc/rustface/
sudo chmod 700 /etc/rustface/faces/
sudo chmod 600 /etc/rustface/faces/*.bin
sudo chmod 755 /usr/lib/security/pam_rustface.so
```

### 10.3 Panic Güvenliği

```rust
#[no_mangle]
pub extern "C" fn pam_sm_authenticate(
    pamh: *mut PamHandle,
    flags: c_int,
    argc: c_int,
    argv: *const *const c_char,
) -> c_int {
    // Tüm kod catch_unwind içinde — panic olursa PAM_IGNORE dön
    // Sistem ASLA kilitlenmez
    let result = std::panic::catch_unwind(|| {
        authenticate_inner(pamh, flags, argc, argv)
    });
    
    match result {
        Ok(pam_result) => pam_result,
        Err(_) => {
            log::error!("rustface-pam: panic occurred, falling back to password");
            PAM_IGNORE  // şifre ekranına düş, sistemi kilitleme
        }
    }
}
```

---

## 11. Hata Yönetimi

```rust
#[derive(Debug, thiserror::Error)]
pub enum RustfaceError {
    #[error("Kamera açılamadı: {path} — {source}")]
    CameraOpen { path: String, source: v4l::Error },
    
    #[error("Frame yakalanamadı: {0}")]
    CameraCapture(#[from] v4l::Error),
    
    #[error("Yanlış kamera formatı: {0:?}, Y800 bekleniyor")]
    WrongFormat(v4l::FourCC),
    
    #[error("Yüz tespit edilemedi")]
    NoFaceDetected,
    
    #[error("Kayıtlı yüz bulunamadı: {username}")]
    NoEnrolledFace { username: String },
    
    #[error("Embedding dosyası okunamadı: {0}")]
    StorageRead(#[from] std::io::Error),
    
    #[error("PAM handle hatası")]
    PamHandle,
}

// Hata → PAM kodu dönüşümü
impl From<RustfaceError> for c_int {
    fn from(err: RustfaceError) -> c_int {
        log::warn!("rustface-pam: {}", err);
        match err {
            RustfaceError::NoFaceDetected => PAM_IGNORE,
            RustfaceError::NoEnrolledFace { .. } => PAM_IGNORE,
            _ => PAM_IGNORE,  // Bilinmeyen hata → her zaman IGNORE
        }
    }
}
```

---

## 12. Geliştirme Sırası

Bu sırayı takip et. Her adımı test et, sonrakine geç.

### Adım 1 — Proje Kurulum (1-2 saat)
```bash
cargo new rustface-pam --lib
cd rustface-pam
# Cargo.toml'u düzenle — cdylib ekle
cargo build
```

### Adım 2 — Kamera Modülü (2-3 saat)
- `v4l` crate ile `/dev/video2` aç
- Y800 formatında tek frame yakala
- PNG olarak kaydet, gözle kontrol et
- Unit test yaz

### Adım 3 — Sahte PAM Modülü (1 saat)
- `pam_sm_authenticate` fonksiyonunu yaz
- Her zaman `PAM_SUCCESS` döndür (test için)
- `.so` olarak derle
- `/etc/pam.d/sudo`'ya ekle
- `sudo echo test` ile dene — şifresiz çalışmalı
- ⚠️ Bunu test ortamında yap, production'da kullanma

### Adım 4 — Yüz Tespiti (3-4 saat)
- `rustface` crate ile yüz tespit et
- Kamera frame'inde yüz bölgesini çiz
- Başarı oranını test et (farklı ışık koşulları)

### Adım 5 — Embedding (2-3 saat)
- Yüzden 128 boyutlu vektör çıkar
- İki fotoğraftan embedding al, cosine similarity hesapla
- Aynı kişi için >0.8, farklı kişi için <0.4 hedefle

### Adım 6 — Storage (1-2 saat)
- `/etc/rustface/faces/` dizini oluştur
- Binary format: embedding yaz/oku
- `rustface-enroll` CLI aracı yaz

### Adım 7 — Entegrasyon (2-3 saat)
- Tüm modülleri `pam_sm_authenticate`'de birleştir
- `catch_unwind` ile sarmala
- Config parsing ekle

### Adım 8 — Test & Kalibrasyon (2-4 saat)
- Threshold değerini ayarla
- Farklı ışık koşullarında test et
- Liveness check ekle

### Toplam tahmini süre: 15-25 saat

---

## 13. Test Stratejisi

### Unit Testler

```rust
#[cfg(test)]
mod tests {
    // Kamera olmadan test — örnek görüntü kullan
    #[test]
    fn test_embedding_same_person() {
        let img1 = load_test_image("test_data/face1a.png");
        let img2 = load_test_image("test_data/face1b.png");
        let recognizer = FaceRecognizer::new("models/face_model.bin").unwrap();
        
        let e1 = recognizer.embed_from_image(&img1).unwrap();
        let e2 = recognizer.embed_from_image(&img2).unwrap();
        
        assert!(cosine_similarity(&e1, &e2) > 0.7);
    }
    
    #[test]
    fn test_embedding_different_person() {
        // Farklı kişi için benzerlik düşük olmalı
        assert!(cosine_similarity(&e1, &e2) < 0.4);
    }
    
    #[test]
    fn test_storage_roundtrip() {
        // Yaz, oku, karşılaştır — aynı olmalı
    }
}
```

### Integration Test

```bash
# Gerçek sudo testi — ayrı bir terminal aç
# Önce mevcut sudo oturumunu sonlandır
sudo -k

# Test et
sudo echo "test"
# Kameraya bak → geçmeli
# Kameraya bakma → şifre sormalı
```

### Güvenlik Testi

```bash
# Fotoğraf testi — telefondan yüz fotoğrafı göster
# IR kamera fotoğrafı farklı yakalamalı, geçmemeli

# Işık testi — karanlıkta dene
# Timeout sonrası şifre ekranına düşmeli
```

---

## Referanslar

- [Linux PAM Modül Yazımı](http://www.linux-pam.org/Linux-PAM-html/mwg-expected-of-module-auth.html)
- [v4l crate docs](https://docs.rs/v4l)
- [rustface crate](https://github.com/atomashpolskiy/rustface)
- [Howdy kaynak kodu](https://github.com/boltgolt/howdy) — referans için incele
- [FaceNet paper](https://arxiv.org/abs/1503.03832) — embedding algoritması

---

*Bu doküman yaşayan bir belge — geliştirme sürecinde güncellenmeli.*
