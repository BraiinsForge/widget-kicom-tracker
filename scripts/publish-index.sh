#!/usr/bin/env bash
# Add a package index and a Nix binary cache beside the built NAR, so the directory,
# served at <base url>, is a package server a Deck registers with `bmc-nix-cli register-server`.
set -euo pipefail

if [ $# -ne 2 ]; then
    echo "usage: $0 <directory holding the .nar and .nar.json> <base url>" >&2
    exit 64
fi
dir=$1
base=${2%/}
root=$(cd -- "$(dirname -- "$0")/.." && pwd)
manifest=$root/manifest.json

metadata=$(echo "$dir"/*.nar.json)
nar=${metadata%.json}
package=$(jq -r .package "$metadata")

work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT

# The Deck adds an uploaded widget under this name too, so both routes give the same store path.
nix-store --restore "$work/bmc-$package" < "$nar"
store_path=$(nix-store --add "$work/bmc-$package")
nix copy --to "file://$work/cache?compression=none" "$store_path"
narinfo=$(basename "$store_path" | cut -d- -f1).narinfo

# The release's NAR doubles as the cache's: package.py writes exactly what Nix dumps.
nar_hash=$(nix-hash --type sha256 --flat --base32 "$nar")
if ! grep -qx "FileHash: sha256:$nar_hash" "$work/cache/$narinfo"; then
    echo "$nar differs from the NAR Nix dumps for $store_path" >&2
    exit 1
fi
sed "s|^URL: .*|URL: $(basename "$nar")|" "$work/cache/$narinfo" > "$dir/$narinfo"
# Above the default 50 of a cache without one, so the device asks its other caches first.
printf 'StoreDir: /nix/store\nPriority: 90\n' > "$dir/nix-cache-info"

cp "$root/$(jq -r .icon "$manifest")" "$dir/icon.svg"
jq -n \
    --slurpfile manifest "$manifest" \
    --arg package "$package" \
    --arg store_path "$store_path" \
    --arg icon "$base/icon.svg" \
    --arg commit "$(git -C "$root" rev-parse HEAD)" \
    '$manifest[0] as $m | {
        version: 1,
        provenance: {commit: $commit},
        indexes: [],
        caches: [],
        packages: [{
            name: $package,
            version: $m.version,
            store_path: $store_path,
            category: "widget",
            description: $m.description,
            upgrade_strategy: null,
            install_strategy: null,
            metadata: {
                widget: ({
                    uid: $m.uid,
                    display_name: $m.name,
                    category: ($m.category // "misc"),
                    supported_viewports: ($m.supported_viewports // [])
                } + if $m.subname then {subname: $m.subname} else {} end),
                assets: {icon: $icon}
            }
        }]
    }' > "$dir/nix-package-index.v1.json"
