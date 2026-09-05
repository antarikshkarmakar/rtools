#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
treeish="${1:-HEAD}"

cd -- "$repo_root"
tree_hash="$(git rev-parse --verify "${treeish}^{tree}")"

invalid_count=0

while IFS= read -r -d '' path; do
    IFS='/' read -r -a components <<< "$path"

    for component in "${components[@]}"; do
        reason=""

        if [[ "$component" == *'<'* || "$component" == *'>'* ||
              "$component" == *':'* || "$component" == *'"'* ||
              "$component" == *'\'* || "$component" == *'|'* ||
              "$component" == *'?'* || "$component" == *'*'* ]]; then
            reason="contains a character forbidden by Windows"
        elif [[ "$component" =~ [[:cntrl:]] ]]; then
            reason="contains an ASCII control character"
        elif [[ "$component" == *'.' || "$component" == *' ' ]]; then
            reason="ends with a dot or space"
        else
            base_name="${component%%.*}"
            case "${base_name^^}" in
                CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9]|COM[¹²³]|LPT[¹²³])
                    reason="uses a reserved Windows device name"
                    ;;
            esac
        fi

        if [[ -z "$reason" ]]; then
            if ! utf16_bytes="$(
                printf '%s' "$component" | iconv -f UTF-8 -t UTF-16LE | wc -c
            )"; then
                reason="is not valid UTF-8"
            elif (( utf16_bytes > 510 )); then
                reason="exceeds the portable 255-UTF-16-unit component limit"
            fi
        fi

        if [[ -n "$reason" ]]; then
            printf 'invalid tracked path %q: component %q %s\n' \
                "$path" "$component" "$reason" >&2
            ((invalid_count += 1))
            break
        fi
    done
done < <(git ls-tree -rz --name-only "$tree_hash")

if (( invalid_count > 0 )); then
    printf 'portable path verification failed: %d invalid tracked path(s)\n' \
        "$invalid_count" >&2
    exit 1
fi

printf 'verified tracked paths are portable to Windows: %s\n' "$treeish"
