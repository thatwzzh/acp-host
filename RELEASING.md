# 发版清单

版本号出现在四处，改的时候一起改：

| 文件 | 字段 |
|---|---|
| `Cargo.toml` | `version` |
| `clients/typescript/package.json` | `version` |
| `clients/python/pyproject.toml` | `version` |
| `clients/*/install.js`、`clients/python/acp_host/binary.py` | 下载用的 `VERSION` 默认值 |

## 1. 二进制

```bash
git tag v0.1.2 && git push origin v0.1.2
```

CI 会编四个平台并挂到 Release（`release.yml`）。

## 2. Homebrew

改 tap 仓库 `Formula/acp-host.rb` 的 `version` 和四个 `sha256`（从 Release 的 `.sha256` 资产里取）：

```bash
cd ../homebrew-acp-host && $EDITOR Formula/acp-host.rb && git commit -am "acp-host 0.1.2" && git push
```

## 3. npm

```bash
cd clients/typescript
npm publish --registry https://registry.npmjs.org --access public --otp=<6位码>
```

两个坑：

- 本机 registry 默认是 `registry.npmmirror.com`（淘宝镜像，只读），**必须显式 `--registry`**
- 账号开了 2FA 时，不给 `--otp` 会把版本**暂存**而不是发布，报 `Cannot publish over previously staged version`；
  想要之后免验证码，建 token 时勾上 `Bypass two-factor authentication`

## 4. PyPI

```bash
cd clients/python && python3 -m build
python3 -m twine upload --username __token__ --password "$PYPI_TOKEN" dist/*
```

首次发布必须是「Entire account」范围的 token（项目还没创建，选不到）；发成功后可以换成只限该项目的 token。

## 5. 验一遍

```bash
brew upgrade acp-host || brew install thatwzzh/acp-host/acp-host
npm view acp-host-client version
pip index versions acp-host-client
```
