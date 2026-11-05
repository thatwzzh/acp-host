#!/bin/bash
# 多渠道发布：GitHub Release → Homebrew tap → npm → PyPI，逐步验证。
#
#   ./publish.sh --check 0.2.0     # 只预检，不改任何东西
#   ./publish.sh 0.2.0             # 真发
#
# 需要：项目根目录 .release.json；环境变量 NPM_TOKEN、PYPI_TOKEN（只有对应渠道需要）。
# GitHub 走本机 keychain 凭据（git push / API）。
set -euo pipefail

DRY=0
[ "${1:-}" = "--check" ] && DRY=1 && shift
VERSION="${1:-}"
[ -n "$VERSION" ] || { echo "用法: $0 [--check] <version>"; exit 1; }

CONFIG=".release.json"
[ -f "$CONFIG" ] || { echo "!! 缺少 $CONFIG（模板见 skill 的 templates/release.json）"; exit 1; }

read_cfg() { python3 -c "import json;d=json.load(open('$CONFIG'));print($1)"; }
REPO=$(read_cfg "d['github']")
TAG="${TAG_PREFIX:-v}${VERSION}"
step() { printf "\n\033[1m== %s\033[0m\n" "$*"; }
run() { if [ "$DRY" = "1" ]; then echo "  [check] $*"; else eval "$@"; fi; }

# ── 1. 预检 ─────────────────────────────────────────────
step "预检 $REPO $TAG"
[ -z "$(git status --porcelain)" ] || { echo "!! 工作区有未提交改动"; exit 1; }
BRANCH=$(git rev-parse --abbrev-ref HEAD)
[ "$BRANCH" = "main" ] || { echo "!! 当前在 $BRANCH，发版请在 main"; exit 1; }
echo "  分支 main ✓ 工作区干净 ✓"

# 版本号必须出现在每个 versionFiles 里——漏一处就会「发出去装到旧二进制」
MISSING=0
while read -r f; do
  [ -z "$f" ] && continue
  [ -f "$f" ] || { echo "!! versionFiles 里的 $f 不存在"; MISSING=1; continue; }
  grep -q "$VERSION" "$f" || { echo "!! $f 里没有 ${VERSION}（忘改？）"; MISSING=1; }
done < <(python3 -c "import json;[print(x) for x in json.load(open('$CONFIG')).get('versionFiles',[])]")
[ "$MISSING" = "0" ] || exit 1
echo "  版本号在 $(python3 -c "import json;print(len(json.load(open('$CONFIG')).get('versionFiles',[])))") 个文件里一致 ✓"

if [ "$DRY" = "1" ]; then
  echo -e "\n预检通过（未改动任何东西）"
  exit 0
fi

# ── 2. 打 tag 并推送 ────────────────────────────────────
step "打 tag $TAG 并推送"
TARGETS=$(read_cfg "' '.join(d['binary']['targets'])")
git tag -a "$TAG" -m "$TAG" 2>/dev/null || echo "  tag 已存在，复用"
git push origin main
git push origin "$TAG"

# ── 3. 等 CI 产物 ──────────────────────────────────────
step "等 GitHub Actions 出产物"
EXPECTED=$(( $(echo "$TARGETS" | wc -w) * 2 ))
for i in $(seq 1 40); do
  COUNT=$(curl -sS --http1.1 --max-time 20 \
    "https://api.github.com/repos/$REPO/releases/tags/$TAG" |
    python3 -c "import json,sys;print(len(json.load(sys.stdin).get('assets',[])))" 2>/dev/null || echo 0)
  echo "  [$i] assets=$COUNT/$EXPECTED"
  [ "$COUNT" -ge "$EXPECTED" ] && break
  sleep 20
done
[ "${COUNT:-0}" -ge "$EXPECTED" ] || { echo "!! 产物没齐，收工"; exit 1; }

# ── 4. 更新 Homebrew formula ────────────────────────────
step "更新 Homebrew formula"
FORMULA=$(read_cfg "d['brew']['formula']")
TAPDIR=$(read_cfg "d['brew']['repoDir']")
ASSET=$(read_cfg "d['binary']['assetPattern']")
PATTERN="$ASSET"
python3 - "$REPO" "$TAG" "$FORMULA" "$TAPDIR" "$PATTERN" "$TARGETS" <<'PY'
import re, subprocess, sys, urllib.request
repo, tag, formula, tapdir, pattern, targets = sys.argv[1:7]
def fetch(url):
    return urllib.request.urlopen(url, timeout=30).read().decode()
sums = {}
for target in targets.split():
    asset = pattern.replace("{target}", target)
    sums[target] = fetch(f"https://github.com/{repo}/releases/download/{tag}/{asset}.sha256").split()[0]
path = f"{tapdir}/{formula}"
src = open(path).read()
src = re.sub(r'version "\d+\.\d+\.\d+"', f'version "{tag.lstrip("v")}"', src, count=1)
old = re.findall(r'sha256 "([0-9a-f]{64})"', src)
assert len(old) == len(sums), f"formula 里有 {len(old)} 个 sha256，目标是 {len(sums)} 个"
for previous, new in zip(old, sums.values()):
    src = src.replace(previous, new, 1)
open(path, "w").write(src)
print(f"  已更新 {len(sums)} 段 sha256")
PY
git -C "$TAPDIR" commit -qam "update to ${VERSION}" && git -C "$TAPDIR" push origin HEAD:main
echo "  tap 已推送"

# ── 5. 发布 npm / PyPI ─────────────────────────────────
step "发布 npm"
NPM_DIR=$(read_cfg "d.get('npm',{}).get('dir','')" 2>/dev/null || true)
if [ -n "${NPM_DIR:-}" ] && [ -n "${NPM_TOKEN:-}" ]; then
  TMPRC=$(mktemp)
  umask 077
  printf 'registry=https://registry.npmjs.org/\n//registry.npmjs.org/:_authToken=%s\n' "$NPM_TOKEN" > "$TMPRC"
  ( cd "$NPM_DIR" && npm publish --userconfig "$TMPRC" --access public )
  rm -f "$TMPRC"
else
  echo "  跳过（没有 npm.dir 或 NPM_TOKEN）"
fi

step "发布 PyPI"
PYPI_DIR=$(read_cfg "d.get('pypi',{}).get('dir','')" 2>/dev/null || true)
if [ -n "${PYPI_DIR:-}" ] && [ -n "${PYPI_TOKEN:-}" ]; then
  ( cd "$PYPI_DIR" && rm -rf dist && python3 -m build >/dev/null &&
    python3 -m twine upload --username __token__ --password "$PYPI_TOKEN" --non-interactive dist/* )
else
  echo "  跳过（没有 pypi.dir 或 PYPI_TOKEN）"
fi

# ── 6. 验证 ────────────────────────────────────────────
step "验证注册表可见"
NPM_PKG=$(read_cfg "d.get('npm',{}).get('package','')" 2>/dev/null || true)
PYPI_PKG=$(read_cfg "d.get('pypi',{}).get('package','')" 2>/dev/null || true)
for i in $(seq 1 20); do
  N="skip"; P="skip"
  [ -n "${NPM_PKG:-}" ] && N=$(curl -sS "https://registry.npmjs.org/$NPM_PKG?t=$(date +%s)" |
    python3 -c "import json,sys;print('ok' if '$VERSION' in json.load(sys.stdin).get('versions',{}) else 'wait')" 2>/dev/null || echo wait)
  [ -n "${PYPI_PKG:-}" ] && P=$(curl -sS "https://pypi.org/pypi/$PYPI_PKG/json?t=$(date +%s)" |
    python3 -c "import json,sys;print('ok' if '$VERSION' in json.load(sys.stdin)['releases'] else 'wait')" 2>/dev/null || echo wait)
  echo "  [$i] npm=$N pypi=$P"
  [ "$N" != "wait" ] && [ "$P" != "wait" ] && break
  sleep 20
done

echo -e "\n发布完成：$TAG"
echo "  建议再用真实安装验证一次：npm i -g $NPM_PKG / pip install $PYPI_PKG==$VERSION，跑 --version 比对"
