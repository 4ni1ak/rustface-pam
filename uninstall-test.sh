#!/bin/bash
# PAM test geri alma — test bittikten sonra çalıştır
set -e

PAM_BACKUP="/etc/pam.d/sudo.bak"
PAM_FILE="/etc/pam.d/sudo"
SO_DST="/usr/lib/security/pam_rustface.so"

if [ -f "$PAM_BACKUP" ]; then
    cp "$PAM_BACKUP" "$PAM_FILE"
    echo "[uninstall-test] /etc/pam.d/sudo geri yüklendi."
else
    echo "[UYARI] Backup bulunamadı: $PAM_BACKUP"
    echo "Manuel olarak pam_rustface satırını kaldır:"
    echo "  sudo nano $PAM_FILE"
fi

if [ -f "$SO_DST" ]; then
    rm "$SO_DST"
    echo "[uninstall-test] $SO_DST silindi."
fi

echo "[uninstall-test] Tamamlandı. sudo tekrar şifre isteyecek."
