#!/bin/bash
# rustface-pam — production kurulum scripti
# Kullanım: sudo bash install.sh [--npu|--gpu]
# Varsayılan: CPU backend

set -e

FEATURE_FLAG=""
BACKEND="cpu"
ENABLE_LOGIN=0

for arg in "$@"; do
    case $arg in
        --npu) FEATURE_FLAG="--features npu"; BACKEND="npu" ;;
        --gpu) FEATURE_FLAG="--features gpu"; BACKEND="gpu" ;;
        --enable-login) ENABLE_LOGIN=1 ;;
    esac
done

echo "[install] Backend: $BACKEND"

# Root kontrolü
if [ "$(id -u)" -ne 0 ]; then
    echo "[HATA] sudo bash install.sh"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Build
echo "[install] Derleniyor (release $FEATURE_FLAG)..."
cd "$SCRIPT_DIR"
sudo -u "$SUDO_USER" cargo build --release $FEATURE_FLAG

SO_SRC="$SCRIPT_DIR/target/release/libpam_rustface.so"
SO_DST="/usr/lib/security/pam_rustface.so"

# .so kur
echo "[install] .so kopyalanıyor (atomik rename)..."
# Önce geçici dosyaya kopyala, sonra atomik mv
# Bu sayede sudo aktif .so'yu değiştirirken segfault olmaz
cp "$SO_SRC" "${SO_DST}.new"
chown root:root "${SO_DST}.new"
chmod 755 "${SO_DST}.new"
mv "${SO_DST}.new" "$SO_DST"

# /etc/rustface dizini
echo "[install] /etc/rustface/ dizinleri oluşturuluyor..."
mkdir -p /etc/rustface/faces /etc/rustface/models
chown -R root:root /etc/rustface/
chmod 700 /etc/rustface/
# 711: unprivileged PAM clients (screen locker runs as the user, not root)
# must be able to open their own known-path .bin file. No listing (no read bit).
# Each .bin is chowned to its own user + mode 600, so it's unreadable by other
# local users — only the owning user (or root) can open it.
chmod 711 /etc/rustface/faces/
chmod 755 /etc/rustface/models/
for f in /etc/rustface/faces/*.bin; do
    [ -e "$f" ] || continue
    user="$(basename "$f" .bin | sed 's/_backup$//')"
    chown "$user:$user" "$f" 2>/dev/null || true
    chmod 600 "$f"
done

# /etc/pam.d/sudo (eğer pam-test satırı varsa kaldır)
PAM_FILE="/etc/pam.d/sudo"
PAM_BACKUP="/etc/pam.d/sudo.install.bak"
cp "$PAM_FILE" "$PAM_BACKUP"

# pam-test satırını kaldır (varsa)
sed -i '/pam_rustface.*debug=true/d' "$PAM_FILE"

# Production satırını ekle (yoksa)
PAM_PROD_LINE="auth  sufficient  /usr/lib/security/pam_rustface.so  threshold=0.6  timeout=3"
if ! grep -q "pam_rustface.so" "$PAM_FILE"; then
    sed -i "1s|^|${PAM_PROD_LINE}\n|" "$PAM_FILE"
    echo "[install] PAM satırı eklendi."
else
    echo "[install] PAM satırı zaten mevcut."
fi

# /etc/pam.d/system-login — login ekranı için yüz tanıma (opsiyonel)
if [ "$ENABLE_LOGIN" = "1" ]; then
    SL="/etc/pam.d/system-login"
    cp "$SL" "${SL}.install.bak"
    LINE="auth  sufficient  /usr/lib/security/pam_rustface.so  threshold=0.6  timeout=3  device=auto  min_uptime=120"
    if ! grep -q "pam_rustface.so" "$SL"; then
        # 'auth include system-auth' satırından önce ekle
        sed -i "/^auth[[:space:]]\+include[[:space:]]\+system-auth/i ${LINE}" "$SL"
        echo "[install] system-login'e face-auth satırı eklendi (min_uptime=120s)."
        echo "[install] Backup: ${SL}.install.bak"
    else
        echo "[install] system-login zaten pam_rustface içeriyor, atlanıyor."
    fi
fi

echo ""
echo "[install] Kurulum tamamlandı!"
echo ""
echo "Sonraki adım — yüz kayıt:"
echo "  sudo rustface-enroll $SUDO_USER"
echo ""
echo "Model dosyaları gerekli:"
echo "  /etc/rustface/models/seeta_fd_frontal_v1.0.bin"
echo "  /etc/rustface/models/arcface.onnx"
