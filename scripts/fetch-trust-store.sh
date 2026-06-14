#!/usr/bin/env bash
# fetch-trust-store.sh — извлекает PEM-сертификаты из живых TLS-соединений
# и раскладывает их по trust-store/{roots,intermediates}.
#
# Источники по умолчанию — официальные домены, использующие УЦ Минцифры:
#   - gosuslugi.ru
#   - esia.gosuslugi.ru
#   - lk.gosuslugi.ru
#   - nalog.gov.ru
#
# Сертификаты получаются командой `openssl s_client -showcerts`, что не
# требует доверия к системному CA. Скрипт фильтрует:
#   * leaf-сертификат (CN = домен) — НЕ кладётся в trust-store;
#   * самоподписанные (Subject == Issuer) → roots/;
#   * остальные                          → intermediates/.
#
# Дополнительно: всегда можно вручную скачать актуальные корни Минцифры
# с https://www.gosuslugi.ru/crt и положить их в roots/.
#
# Usage:
#   ./scripts/fetch-trust-store.sh [host1 host2 ...]
#
# Требования: bash, openssl, awk, sha256sum (или shasum -a 256 на macOS).

set -euo pipefail

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
ROOT_DIR="$( cd "${SCRIPT_DIR}/.." && pwd )"
TRUST_DIR="${ROOT_DIR}/trust-store"
ROOTS_DIR="${TRUST_DIR}/roots"
INTERMEDIATES_DIR="${TRUST_DIR}/intermediates"

mkdir -p "${ROOTS_DIR}" "${INTERMEDIATES_DIR}"

DEFAULT_HOSTS=(
  gosuslugi.ru
  esia.gosuslugi.ru
  lk.gosuslugi.ru
  nalog.gov.ru
)

HOSTS=( "$@" )
if [ "${#HOSTS[@]}" -eq 0 ]; then
  HOSTS=( "${DEFAULT_HOSTS[@]}" )
fi

if ! command -v openssl >/dev/null; then
  echo "ERROR: openssl is required" >&2; exit 1
fi

SHA256() {
  if command -v sha256sum >/dev/null; then sha256sum "$1" | awk '{print $1}';
  else shasum -a 256 "$1" | awk '{print $1}'; fi
}

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

extract_chain() {
  local host="$1"
  local out="${TMP_DIR}/${host}.bundle"
  echo "  → fetching chain from ${host}:443"
  # Таймаут через timeout (Linux) или perl (macOS fallback)
  if command -v timeout >/dev/null; then
    timeout 15 openssl s_client -showcerts -connect "${host}:443" \
      -servername "${host}" </dev/null 2>/dev/null > "${out}" || true
  else
    openssl s_client -showcerts -connect "${host}:443" \
      -servername "${host}" </dev/null 2>/dev/null > "${out}" || true
  fi
  if ! grep -q "BEGIN CERTIFICATE" "${out}"; then
    echo "    ✘ no certificates received from ${host}"
    return 1
  fi
  return 0
}

split_chain() {
  local host="$1"
  local bundle="${TMP_DIR}/${host}.bundle"
  local n=0
  awk '
    /-----BEGIN CERTIFICATE-----/ { capture=1; idx++; out="'"${TMP_DIR}/${host}"'-" idx ".pem" }
    capture { print > out }
    /-----END CERTIFICATE-----/   { capture=0 }
  ' "${bundle}"
}

classify_and_save() {
  local host="$1"
  local i=1
  while [ -f "${TMP_DIR}/${host}-${i}.pem" ]; do
    local cert="${TMP_DIR}/${host}-${i}.pem"
    local subject issuer cn
    subject="$(openssl x509 -in "${cert}" -noout -subject 2>/dev/null | sed 's/^subject= *//')"
    issuer="$(openssl x509 -in "${cert}" -noout -issuer  2>/dev/null | sed 's/^issuer= *//')"
    cn="$(openssl x509 -in "${cert}" -noout -subject 2>/dev/null \
          | sed -n 's/.*CN *= *\([^,\/]*\).*/\1/p')"

    if [ "${i}" = "1" ]; then
      # leaf — пропускаем
      echo "    · skip leaf  CN=${cn}"
      i=$((i+1)); continue
    fi

    local kind="intermediates"
    [ "${subject}" = "${issuer}" ] && kind="roots"
    local fp; fp="$(SHA256 "${cert}" | cut -c1-12)"
    local slug
    slug="$(printf '%s' "${cn:-cert}" | tr 'A-Z' 'a-z' | tr -cs 'a-z0-9' '-' | sed 's/-*$//; s/^-*//')"
    local dst="${TRUST_DIR}/${kind}/${slug:-cert}-${fp}.pem"
    if [ -f "${dst}" ]; then
      echo "    · dup ${kind}/${slug:-cert}-${fp}.pem (already present)"
    else
      cp "${cert}" "${dst}"
      echo "    ✔ ${kind}/${slug:-cert}-${fp}.pem  (${subject})"
    fi
    i=$((i+1))
  done
}

echo "Trust-store target: ${TRUST_DIR}"
for host in "${HOSTS[@]}"; do
  echo "── ${host} ──"
  if extract_chain "${host}"; then
    split_chain "${host}"
    classify_and_save "${host}"
  fi
done

echo
echo "Готово. Сводка:"
echo "  roots:         $(ls -1 "${ROOTS_DIR}"          2>/dev/null | grep -E '\.(pem|crt|cer)$' | wc -l | tr -d ' ')"
echo "  intermediates: $(ls -1 "${INTERMEDIATES_DIR}"  2>/dev/null | grep -E '\.(pem|crt|cer)$' | wc -l | tr -d ' ')"
echo
echo "Проверить отдельный файл можно так:"
echo "  openssl x509 -in trust-store/roots/<file>.pem -noout -text"
echo
echo "Если нужны именно корни УЦ Минцифры, скачайте их с официальной страницы:"
echo "  https://www.gosuslugi.ru/crt"
echo "и положите .cer/.pem в trust-store/roots/"
