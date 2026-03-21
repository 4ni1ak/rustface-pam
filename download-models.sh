#!/bin/bash
# Model dosyalarını indir
# /etc/rustface/models/ konumuna kur
set -e

MODEL_DIR="/etc/rustface/models"

echo "[models] Hedef: $MODEL_DIR"

# seeta_fd_frontal_v1.0.bin — rustface yüz tespiti modeli
SEETA_URL="https://github.com/atomashpolskiy/rustface/raw/master/model/seeta_fd_frontal_v1.0.bin"
SEETA_DST="$MODEL_DIR/seeta_fd_frontal_v1.0.bin"

if [ ! -f "$SEETA_DST" ]; then
    echo "[models] SeetaFace modeli indiriliyor..."
    curl -L "$SEETA_URL" -o "$SEETA_DST"
    echo "[models] seeta_fd_frontal_v1.0.bin indirildi."
else
    echo "[models] seeta_fd_frontal_v1.0.bin zaten mevcut."
fi

# ArcFace ONNX modeli
# Kaynak: https://github.com/onnx/models (buffalo_l veya w600k_r50)
ARCFACE_DST="$MODEL_DIR/arcface.onnx"

if [ ! -f "$ARCFACE_DST" ]; then
    echo "[models] ArcFace ONNX modeli indiriliyor..."
    # InsightFace'den w600k_r50 (ResNet50, ~166MB)
    ARCFACE_URL="https://github.com/deepinsight/insightface/releases/download/v0.7/buffalo_l.zip"
    TMPZIP="/tmp/buffalo_l.zip"
    curl -L "$ARCFACE_URL" -o "$TMPZIP"
    cd /tmp && unzip -o "$TMPZIP" "w600k_r50.onnx" 2>/dev/null || \
    unzip -o "$TMPZIP" "*.onnx" 2>/dev/null
    # İlk bulunan .onnx'i arcface.onnx olarak kur
    ONNX_FILE=$(find /tmp -name "*.onnx" -newer "$TMPZIP" | head -1)
    if [ -n "$ONNX_FILE" ]; then
        cp "$ONNX_FILE" "$ARCFACE_DST"
        echo "[models] arcface.onnx kuruldu: $ONNX_FILE"
    else
        echo "[UYARI] .onnx dosyası bulunamadı — manuel indirme gerekli"
        echo "  URL: https://github.com/deepinsight/insightface"
        echo "  Koy: $ARCFACE_DST"
    fi
else
    echo "[models] arcface.onnx zaten mevcut."
fi

echo ""
echo "[models] Kurulu modeller:"
ls -lh "$MODEL_DIR/"
