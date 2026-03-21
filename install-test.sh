#!/bin/bash
# PAM test kurulum scripti — sadece test ortamında kullan
# Çalıştır: bash install-test.sh

set -e

SO_SRC="target/release/libpam_rustface.so"
SO_DST="/usr/lib/security/pam_rustface.so"
PAM_FILE="/etc/pam.d/sudo"
PAM_BACKUP="/etc/pam.d/sudo.bak"
PAM_LINE="auth  sufficient  /usr/lib/security/pam_rustface.so  debug=true"

echo "[install-test] .so kontrol ediliyor..."
if [ ! -f "$SO_SRC" ]; then
    echo "[HATA] $SO_SRC bulunamadı — önce cargo build --release --features pam-test"
    exit 1
fi

echo "[install-test] /etc/pam.d/sudo yedekleniyor → $PAM_BACKUP"
cp "$PAM_FILE" "$PAM_BACKUP"

echo "[install-test] .so kopyalanıyor → $SO_DST"
cp "$SO_SRC" "$SO_DST"
chmod 755 "$SO_DST"

echo "[install-test] PAM satırı ekleniyor (en üste)..."
# Zaten varsa ekleme
if grep -q "pam_rustface" "$PAM_FILE"; then
    echo "[install-test] Zaten mevcut, atlanıyor."
else
    # En üste ekle (ilk #%PAM... satırından sonra)
    sed -i "1s|^|${PAM_LINE}\n|" "$PAM_FILE"
fi

echo "[install-test] /etc/pam.d/sudo şu an:"
cat "$PAM_FILE"
echo ""
echo "[install-test] Kurulum tamam. Test:"
echo "  sudo -k && sudo echo 'rustface pam-test OK'"
echo ""
echo "[install-test] Geri almak için:"
echo "  sudo cp $PAM_BACKUP $PAM_FILE && sudo rm $SO_DST"
